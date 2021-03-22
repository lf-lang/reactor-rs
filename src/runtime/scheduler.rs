

use std::cmp::Reverse;



use std::hash::{Hash};
use std::marker::PhantomData;


use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;


use crate::runtime::{Logical, LogicalAction, Physical, PhysicalAction, ReactorAssembler};
use crate::runtime::ports::{InputPort, OutputPort};

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


/// Global state of the system.
struct SchedulerState {
    /// Current logical time in the system. Note that reactions
    /// executing have their own copy of the value they were scheduled on.
    cur_logical_time: LogicalTime,
    /// Micro-step of the next action to schedule from this
    /// logical time on. This way if several actions are scheduled
    /// at the same logical time they're scheduled at increasing
    /// micro-steps so they're still ordered.
    // fixme this is completely monotonic for now & isn't reset when logical time increases
    micro_step: MicroStep,
}

/// Main public API for the scheduler. Contains the priority queue
/// and public launch routine with event loop.
pub struct SyncScheduler {
    /// Reference to the shared state (note, every reaction
    /// ctx has the same kind of reference as this, ie, this
    /// one is not special)
    state: SchedulerRef,
    /// The receiver end of the communication channels. Reactions
    /// contexts each have their own [Sender]. The main event loop
    /// polls this to make progress.
    ///
    /// Note that the receiver is unique.
    receiver: Receiver<Event>,
    // todo the priority (Reverse<LogicalTime>) should take into account relative reaction priority
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
}

impl SyncScheduler {
    /// Creates a new scheduler. Its state is initialized to nothing.
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Event>();
        let sched = SchedulerState {
            cur_logical_time: <_>::default(),
            micro_step: 0,
        };
        let state = SchedulerRef {
            state: Arc::new(Mutex::new(sched)),
            sender,
        };
        Self {
            state,
            receiver,
            queue: PriorityQueue::new(),
        }
    }

    pub fn launch_async(mut self) -> JoinHandle<()> {
        use std::thread;
        thread::spawn(move || {
            loop {
                if let Ok(evt) = self.receiver.recv() {
                    let eta = evt.eta();
                    self.queue.push(evt, Reverse(eta));
                    let (evt, Reverse(eta)) = self.queue.pop().unwrap();
                    self.state.step(evt, eta)
                } else {
                    // all senders have hung up
                    println!("We're done here");
                    break;
                }
            }
        })
    }

    fn shutdown(self) {

    }

    pub fn start(&self, r: &mut impl ReactorAssembler) {
        let sched = self.state.clone();
        let ctx = self.state.critical(|state| {
            PhysicalCtx {
                scheduler: sched,
                cur_logical_time: state.cur_logical_time,
                _t: PhantomData
            }
        });
        r.start(ctx)
    }
}

/// Reference to the global state with a handle to send events.
#[derive(Clone)]
struct SchedulerRef {
    /// Reference to shared global state
    state: Arc<Mutex<SchedulerState>>,
    sender: Sender<Event>,
}

impl SchedulerRef {
    fn critical<T>(&self, f: impl FnOnce(MutexGuard<SchedulerState>) -> T) -> T {
        let guard = self.state.lock().unwrap();
        f(guard)
    }

    pub fn new_ctx(&self) -> LogicalCtx {
        let sched = self.clone();
        self.critical(|state| {
            LogicalCtx {
                scheduler: sched,
                cur_logical_time: state.cur_logical_time,
                _t: PhantomData
            }
        })
    }

    fn step(&self, event: Event, eta: LogicalTime) {
        let reaction = event.reaction();

        SchedulerRef::catch_up_physical_time(eta);
        self.critical(|mut s| s.cur_logical_time = LogicalTime::default());

        let mut ctx = self.new_ctx();
        reaction.fire(&mut ctx)
        // todo probably we should destroy the port values at this time
    }

    fn catch_up_physical_time(up_to_time: LogicalTime) {
        let now = Instant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
            std::thread::sleep(t);
        }
    }

    fn enqueue_port(&self, downstream: Dependencies, now: LogicalTime) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in downstream.reactions.iter() {
            self.critical(|scheduler| {
                let time = scheduler.cur_logical_time;
                let evt = Event::ReactionExecute { tag: now, at: time, reaction: reaction.clone() };
                self.sender.send(evt).unwrap();
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
            self.sender.send(evt).unwrap(); // send it into the event queue
        }
    }
}


pub type LogicalCtx = Ctx<Logical>;
pub type PhysicalCtx = Ctx<Physical>;


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct Ctx<T> {
    scheduler: SchedulerRef,
    cur_logical_time: LogicalTime,
    _t: PhantomData<T>
}

impl<A> Ctx<A> {
    /// Get the value of a port at this time.
    pub fn get<T: Copy>(&self, port: &InputPort<T>) -> Option<T> {
        port.get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    pub fn set<T>(&mut self, port: &mut OutputPort<T>, value: T) {
        let downstream = port.set(value);
        self.scheduler.enqueue_port(downstream, self.cur_logical_time);
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule(&mut self, action: &LogicalAction, offset: Offset) {
        self.schedule_impl(action, offset);
    }

    // private
    fn schedule_impl<T>(&mut self, action: &Action<T>, offset: Offset) {
        self.scheduler.enqueue_action(action, offset.to_duration())
    }

    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
    }

    /// Request a shutdown which will be acted upon at the end
    /// of this reaction.
    pub fn request_shutdown(self) {
        // self.scheduler.shutdown()
    }
}

impl LogicalCtx {
    /// note: this doesn't work for
    pub fn get_logical_time(&self) -> LogicalTime {
        self.cur_logical_time
    }
}

impl PhysicalCtx {
    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule_physical(&mut self, action: &PhysicalAction, offset: Offset) {
        self.schedule_impl(action, offset);
    }
}
