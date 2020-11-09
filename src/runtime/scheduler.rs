use std::cell::{Ref, RefMut, Cell};
use std::cmp::Reverse;
use std::fmt::{Debug, Pointer};
use std::ops::Deref;
use std::rc::Rc;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;
use crate::runtime::ports::{Port, InputPort, OutputPort};
use std::hash::{Hash, Hasher};
use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{channel, Sender, Receiver};
use crate::reactors::Named;
use std::fmt::Formatter;
use std::fmt::Display;
use super::time::*;
use super::{ReactionInvoker, Dependencies, Action};

#[derive(Eq, PartialEq, Hash)]
enum Event {
    ReactionExecute { tag: LogicalTime, at: LogicalTime, reaction: Arc<ReactionInvoker> },
    ReactionSchedule { tag: LogicalTime, min_at: LogicalTime, reaction: Arc<ReactionInvoker> },
}

impl Event {
    fn tag(&self) -> LogicalTime {
        match self {
            Event::ReactionExecute { tag, .. } => tag.clone(),
            Event::ReactionSchedule { tag, .. } => tag.clone()
        }
    }
}

#[derive(Clone)]
pub struct SchedulerRef {
    state: Arc<Mutex<SchedulerState>>,
    sender: Sender<Event>,
}

pub struct SyncScheduler {
    state: SchedulerRef,
    receiver: Receiver<Event>,
}

impl SyncScheduler {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Event>();
        let sched = SchedulerState {
            cur_logical_time: <_>::default(),
            micro_step: 0,
            queue: PriorityQueue::new(),
        };
        let state = SchedulerRef {
            state: Arc::new(Mutex::new(sched)),
            sender,
        };
        Self {
            state,
            receiver,
        }
    }

    pub fn launch_async(self) {
        use std::thread;
        thread::spawn(move || {
            loop {
                if let Ok(evt) = self.receiver.recv() {
                    let tag = evt.tag();
                    self.state.step(evt, tag)
                } else {
                    // all senders have hung up
                    println!("We're done here");
                    break;
                }
            }
        }).join();
    }

    pub fn new_ctx(&self) -> Ctx {
        self.state.new_ctx()
    }
}

/// Directs execution of the whole reactor graph.
pub struct SchedulerState {
    cur_logical_time: LogicalTime,
    micro_step: MicroStep,
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
}

impl SchedulerRef {
    fn critical<T>(&self, f: impl FnOnce(MutexGuard<SchedulerState>) -> T) -> T {
        let guard = self.state.lock().unwrap();
        f(guard)
    }


    pub fn new_ctx(&self) -> Ctx {
        let sched = self.clone();
        self.critical(|state| {
            Ctx {
                scheduler: sched,
                cur_logical_time: state.cur_logical_time,
            }
        })
    }

    fn step(&self, event: Event, eta: LogicalTime) {
        let reaction = match event {
            Event::ReactionExecute { reaction, .. } => reaction,
            Event::ReactionSchedule { reaction, .. } => reaction
        };

        SchedulerRef::catch_up_physical_time(eta);
        self.critical(|mut s| s.cur_logical_time = LogicalTime::default());

        let mut ctx = self.new_ctx();
        reaction.fire(&mut ctx)
    }

    fn catch_up_physical_time(up_to_time: LogicalTime) {
        let now = Instant::now();
        if now < up_to_time.instant {
            std::thread::sleep(up_to_time.instant - now);
        }
    }

    fn enqueue_port(&self, downstream: Ref<Dependencies>, now: LogicalTime) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in downstream.reactions.iter() {
            self.critical(|mut scheduler| {
                let time = scheduler.cur_logical_time;
                let evt = Event::ReactionExecute { tag: now, at: time, reaction: reaction.clone() };
                self.sender.send(evt);
            });
        }
    }

    fn enqueue_action(&self, action: &Action, additional_delay: Duration) {
        let min_delay = action.delay + additional_delay;

        let mut scheduler = self.state.lock().unwrap();
        let now = scheduler.cur_logical_time.clone();

        let mut instant = now.instant + min_delay;
        if !action.logical {
            // physical actions are adjusted to physical time if needed
            instant = Instant::max(instant, Instant::now());
        }


        // note that the microstep is global, doesn't really matter though
        scheduler.micro_step += 1;
        let eta = LogicalTime {
            instant,
            microstep: scheduler.micro_step,
        };

        for reaction in action.downstream.reactions.iter() {
            let evt = Event::ReactionSchedule { tag: now, min_at: eta, reaction: reaction.clone() };
            self.sender.send(evt);
        }
    }
}


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct Ctx {
    scheduler: SchedulerRef,
    cur_logical_time: LogicalTime,
}

impl Ctx {
    /// Get the value of a port at this time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_uses](super::Assembler::reaction_uses)).
    ///
    pub fn get<T: Copy>(&self, port: &InputPort<T>) -> Option<T> {
        port.get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_affects](super::Assembler::reaction_affects)).
    ///
    pub fn set<T>(&mut self, port: &mut OutputPort<T>, value: T) {
        let downstream = port.set(value);
        self.scheduler.enqueue_port(downstream, self.cur_logical_time);
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given action ([reaction_schedules](super::Assembler::reaction_schedules)).
    pub fn schedule(&mut self, action: &Action) {
        self.schedule_delayed(action, Duration::from_secs(0))
    }

    pub fn schedule_delayed(&mut self, action: &Action, offset: Duration) {
        self.scheduler.enqueue_action(action, offset)
    }

    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
    }

    /// note: this doesn't work for
    pub fn get_logical_time(&self) -> LogicalTime {
        self.cur_logical_time
    }
}


/// Wrapper around the user struct for safe dispatch.
///
/// Fields are
/// 1. the user struct, and
/// 2. every action and port declared by the reactor.
///
pub trait ReactorDispatcher {
    /// The type of reaction IDs
    type ReactionId: Copy + Named;
    /// Type of the user struct
    type Wrapped;
    /// Type of the construction parameters
    type Params;

    /// Assemble the user reactor, ie produce components with
    /// uninitialized dependencies & make state variables assume
    /// their default values, or else, a value taken from the params.
    fn assemble(args: Self::Params) -> Self;

    /// Execute a single user-written reaction.
    /// Dispatches on the reaction id, and unpacks parameters,
    /// which are the reactor components declared as fields of
    /// this struct.
    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId);
}

/// Declares dependencies of every reactor component.
///
/// Fields are
/// 1. a ReactorDispatcher
/// 2. a Rc<ReactionInvoker> for every reaction declared by the reactor
///
pub trait ReactorAssembler {
    /// Type of the [ReactorDispatcher]
    type RState: ReactorDispatcher;

    /// Execute the startup reaction of the reactor
    fn start(&mut self, ctx: Ctx);

    /// Create a new instance. The rid is a counter used to
    /// give unique IDs to reactions. The args are passed down
    /// to [ReactorDispatcher::assemble].
    ///
    /// The components of the ReactorDispatcher must be filled
    /// in with their respective dependencies (precomputed before
    /// codegen)
    fn assemble(rid: &mut i32,
                args: <Self::RState as ReactorDispatcher>::Params) -> Self;
}
