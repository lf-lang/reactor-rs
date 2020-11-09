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

type MicroStep = u128;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct LogicalTime {
    instant: Instant,
    microstep: MicroStep,
}

impl Display for LogicalTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("LogicalTime(")?;
        self.instant.fmt(f)?;
        f.write_str(" + ")?;
        <_ as Display>::fmt(&self.microstep, f)?;
        f.write_str(")")?;
        Ok(())
    }
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self { instant: Instant::now(), microstep: 0 }
    }
}

impl LogicalTime {
    pub fn to_instant(&self) -> Instant {
        self.instant
    }
}

#[derive(Eq, PartialEq, Hash)]
enum Event {
    ReactionExecute { at: LogicalTime, reaction: Arc<ReactionInvoker> },
    ReactionSchedule { min_at: LogicalTime, reaction: Arc<ReactionInvoker> },
}

#[derive(Clone)]
pub struct SyncScheduler {
    state: Arc<Mutex<Scheduler>>
}


/// Directs execution of the whole reactor graph.
pub struct Scheduler {
    cur_logical_time: LogicalTime,
    micro_step: MicroStep,
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
}

impl SyncScheduler {
    fn with_state<T>(&self, f: impl FnOnce(MutexGuard<Scheduler>) -> T) -> T {
        let guard = self.state.lock().unwrap();
        f(guard)
    }

    pub fn new() -> Self {
        let sched = Scheduler {
            cur_logical_time: <_>::default(),
            micro_step: 0,
            queue: PriorityQueue::new(),
        };
        Self {
            state: Arc::new(Mutex::new(sched))
        }
    }

    pub fn new_ctx(&self) -> Ctx {
        let inst = self.state.lock().unwrap().cur_logical_time.clone();
        Ctx {
            scheduler: self.clone(),
            cur_logical_time: inst,
        }
    }

    pub fn launch(self) {
        loop {
            let next = self.with_state(|mut locked| {
                if let Some((event, Reverse(eta))) = locked.queue.pop() {
                    Some((event, eta))
                } else {
                    None
                }
            });
            if let Some((event, eta)) = next {
                self.step(event, eta);
            } else {
                // break;
            }
        }
    }

    fn step(&self, event: Event, eta: LogicalTime) {
        let reaction = match event {
            Event::ReactionExecute { reaction, .. } => reaction,
            Event::ReactionSchedule { reaction, .. } => reaction
        };

        SyncScheduler::catch_up_physical_time(eta);
        self.with_state(|mut s| s.cur_logical_time = eta);

        let mut ctx = Ctx {
            scheduler: self.clone(),
            cur_logical_time: eta,
        };
        reaction.fire(&mut ctx)
    }

    fn catch_up_physical_time(up_to_time: LogicalTime) {
        let now = Instant::now();
        if now < up_to_time.instant {
            std::thread::sleep(up_to_time.instant - now);
        }
    }

    fn enqueue_port(&self, downstream: Ref<Dependencies>) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in downstream.reactions.iter() {
            self.with_state(|mut scheduler| {
                let time = scheduler.cur_logical_time;
                let evt = Event::ReactionExecute { at: time, reaction: reaction.clone() };
                scheduler.queue.push(evt, Reverse(time));
            });
        }
    }

    fn enqueue_action(&self, action: &Action, additional_delay: Duration) {
        let min_delay = action.delay + additional_delay;
        let mut scheduler = self.state.lock().unwrap();

        let mut instant = scheduler.cur_logical_time.instant + min_delay;
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
            let evt = Event::ReactionSchedule { min_at: eta, reaction: reaction.clone() };
            scheduler.queue.push(evt, Reverse(eta));
        }
    }
}


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct Ctx {
    scheduler: SyncScheduler,
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
        self.scheduler.enqueue_port(downstream);
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
        self.scheduler.enqueue_action(action, Duration::from_secs(0))
    }

    pub fn schedule_delayed(&mut self, action: &Action, offset: Duration) {
        self.scheduler.enqueue_action(action, offset)
    }

    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
    }

    pub fn get_logical_time(&self) -> LogicalTime {
        self.cur_logical_time
    }
}


pub struct Action {
    delay: Duration,
    logical: bool,
    downstream: Dependencies,
    name: &'static str,
}

impl Action {
    pub fn set_downstream(&mut self, r: Dependencies) {
        self.downstream = r
    }

    pub fn new(
        min_delay: Option<Duration>,
        is_logical: bool,
        name: &'static str) -> Self {
        Action {
            delay: min_delay.unwrap_or(Duration::new(0, 0)),
            logical: is_logical,
            downstream: Default::default(),
            name,
        }
    }
}

impl Named for Action {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
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

pub struct Dependencies {
    reactions: Vec<Arc<ReactionInvoker>>
}

impl Default for Dependencies {
    fn default() -> Self {
        Self { reactions: Vec::new() }
    }
}

impl Dependencies {
    pub fn append(&mut self, other: &mut Dependencies) {
        self.reactions.append(&mut other.reactions)
    }
}

impl From<Vec<Arc<ReactionInvoker>>> for Dependencies {
    fn from(reactions: Vec<Arc<ReactionInvoker>>) -> Self {
        Self { reactions }
    }
}

pub struct ReactionInvoker {
    body: Box<dyn Fn(&mut Ctx)>,
    id: i32,
    name: &'static str,
}

unsafe impl Sync for ReactionInvoker {}

unsafe impl Send for ReactionInvoker {}

impl Named for ReactionInvoker {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl Display for ReactionInvoker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
    }
}

impl ReactionInvoker {
    fn fire(&self, ctx: &mut Ctx) {
        (self.body)(ctx)
    }

    pub fn new<T: ReactorDispatcher + 'static>(id: i32,
                                               reactor: Rc<RefCell<T>>,
                                               rid: T::ReactionId) -> ReactionInvoker {
        let body = move |ctx: &mut Ctx| {
            let mut ref_mut = reactor.deref().borrow_mut();
            let r1: &mut T = &mut *ref_mut;
            T::react(r1, ctx, rid)
        };
        ReactionInvoker {
            body: Box::new(body),
            id,
            name: rid.name(),
        }
    }
}


impl PartialEq for ReactionInvoker {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ReactionInvoker {}

impl Hash for ReactionInvoker {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}
