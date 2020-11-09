use std::cell::{Cell, Ref, RefMut};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::fmt::{Debug, Pointer};
use std::fmt::Display;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;

use crate::reactors::Named;
use crate::runtime::{LogicalAction, PhysicalAction, ReactorAssembler};
use crate::runtime::ports::{InputPort, OutputPort, Port};

use super::{Action, Dependencies, ReactionInvoker};
use super::time::*;

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

    fn eta(&self) -> LogicalTime {
        match self {
            Event::ReactionExecute { at, .. } => at.clone(),
            Event::ReactionSchedule { min_at, .. } => min_at.clone()
        }
    }

    fn reaction(&self) -> Arc<ReactionInvoker> {
        match self {
            Event::ReactionExecute { reaction, .. } => reaction.clone(),
            Event::ReactionSchedule { reaction, .. } => reaction.clone()
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

    pub fn launch_async(self) -> JoinHandle<()> {
        use std::thread;
        thread::spawn(move || {
            loop {
                if let Ok(evt) = self.receiver.recv() {
                    let tag = evt.eta();
                    self.state.step(evt, tag)
                } else {
                    // all senders have hung up
                    println!("We're done here");
                    break;
                }
            }
        })
    }

    pub fn start(&self, r: &mut impl ReactorAssembler) {
        let sched = self.state.clone();
        let ctx = self.state.critical(|state| {
            PhysicalCtx {
                scheduler: sched,
                cur_logical_time: state.cur_logical_time,
            }
        });
        r.start(ctx)
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
        let reaction = event.reaction();

        SchedulerRef::catch_up_physical_time(eta);
        self.critical(|mut s| s.cur_logical_time = LogicalTime::default());

        let mut ctx = self.new_ctx();
        reaction.fire(&mut ctx)
    }

    fn catch_up_physical_time(up_to_time: LogicalTime) {
        let now = Instant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
            std::thread::sleep(t);
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

    fn enqueue_action<T>(&self, action: &Action<T>, additional_delay: Duration) {
        let (now, eta) = self.critical(|mut scheduler| {
            let now = scheduler.cur_logical_time.clone();

            // note that the microstep is global, doesn't really matter though
            scheduler.micro_step += 1;
            (now, action.make_eta(now, scheduler.micro_step, additional_delay))
        });

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
    pub fn schedule(&mut self, action: &LogicalAction) {
        self.schedule_delayed(action, Duration::from_secs(0))
    }

    pub fn schedule_delayed(&mut self, action: &LogicalAction, offset: Duration) {
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

/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct PhysicalCtx {
    scheduler: SchedulerRef,
    cur_logical_time: LogicalTime,
}

impl PhysicalCtx {
    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given action ([reaction_schedules](super::Assembler::reaction_schedules)).
    pub fn schedule<T>(&mut self, action: &Action<T>) {
        self.schedule_delayed(action, Duration::from_secs(0))
    }

    pub fn schedule_delayed<T>(&mut self, action: &Action<T>, offset: Duration) {
        self.scheduler.enqueue_action(action, offset)
    }

    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
    }
}
