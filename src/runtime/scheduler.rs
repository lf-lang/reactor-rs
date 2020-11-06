use std::cell::{Ref, RefMut, Cell};
use std::cmp::Reverse;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;
use crate::runtime::ports::{Port, InputPort, OutputPort};
use std::hash::{Hash, Hasher};
use std::cell::RefCell;

type MicroStep = u128;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct LogicalTime {
    instant: Instant,
    microstep: MicroStep,
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
    ReactionExecute { at: LogicalTime, reaction: Rc<ReactionInvoker> },
    ReactionSchedule { min_at: LogicalTime, reaction: Rc<ReactionInvoker> },
}

/// Directs execution of the whole reactor graph.
pub struct Scheduler {
    cur_logical_time: LogicalTime,
    micro_step: MicroStep,
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
}

impl<'g> Scheduler {
    // todo logging

    pub(in super) fn new() -> Self {
        Scheduler {
            cur_logical_time: <_>::default(),
            micro_step: 0,
            queue: PriorityQueue::new(),
        }
    }

    pub fn launch(&mut self, startup_action: &Action) {
        self.enqueue_action(startup_action, None);
        while !self.queue.is_empty() {
            self.step()
        }
    }

    fn step(&mut self) {
        if let Some((event, Reverse(time))) = self.queue.pop() {
            let reaction = match event {
                Event::ReactionExecute { reaction, .. } => reaction,
                Event::ReactionSchedule { reaction, .. } => reaction
            };

            self.catch_up_physical_time(time);
            self.cur_logical_time = time;

            let mut ctx = Ctx {
                scheduler: self,
                cur_logical_time: time,
            };
            reaction.fire(&mut ctx)
        }
    }

    fn catch_up_physical_time(&mut self, up_to_time: LogicalTime) {
        let now = Instant::now();
        if now < up_to_time.instant {
            std::thread::sleep(up_to_time.instant - now);
        }
    }

    fn enqueue_port(&mut self, downstream: Ref<Vec<Rc<ReactionInvoker>>>) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in downstream.iter() {
            let evt = Event::ReactionExecute { at: self.cur_logical_time, reaction: reaction.clone() };
            self.queue.push(evt, Reverse(self.cur_logical_time));
        }
    }

    fn enqueue_action(&mut self, action: &Action, additional_delay: Option<Duration>) {
        let min_delay = action.delay + additional_delay.unwrap_or(Duration::from_secs(0));

        let mut instant = self.cur_logical_time.instant + min_delay;
        if !action.logical {
            // physical actions are adjusted to physical time if needed
            instant = Instant::max(instant, Instant::now());
        }

        // note that the microstep is global, doesn't really matter though
        self.micro_step += 1;
        let eta = LogicalTime {
            instant,
            microstep: self.micro_step,
        };

        for reaction in action.downstream.iter() {
            let evt = Event::ReactionSchedule { min_at: eta, reaction: reaction.clone() };
            self.queue.push(evt, Reverse(eta));
        }
    }
}


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct Ctx<'a> {
    scheduler: &'a mut Scheduler,
    cur_logical_time: LogicalTime,
}

impl<'a> Ctx<'a> {
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
    pub fn schedule(&mut self, action: &Action, offset: Option<Duration>) {
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
    downstream: Vec<Rc<ReactionInvoker>>,
}

impl Action {
    pub(in super) fn new(
        min_delay: Option<Duration>,
        is_logical: bool) -> Self {
        Action {
            delay: min_delay.unwrap_or(Duration::new(0, 0)),
            logical: is_logical,
            downstream: Vec::new(),
        }
    }
}


pub trait ReactionState {
    type ReactionId: Copy;
    type Wrapped;
    type Params;

    fn assemble(args: Self::Params) -> Self;
    fn start(&mut self, ctx: &mut Ctx);
    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId);
}

pub trait AssemblyWrapper {
    type RState: ReactionState;
    fn assemble(rid: &mut i32, args: <Self::RState as ReactionState>::Params) -> Self;
}

pub struct ReactionInvoker {
    body: Box<dyn Fn(&mut Ctx)>,
    id: i32,
}

impl ReactionInvoker {
    fn fire(&self, ctx: &mut Ctx) {
        (self.body)(ctx)
    }

    pub(in super) fn new<T: ReactionState + 'static>(id: i32,
                                                     reactor: Rc<RefCell<T>>,
                                                     rid: T::ReactionId) -> ReactionInvoker {
        let body = move |ctx: &mut Ctx<'_>| {
            let mut ref_mut = reactor.deref().borrow_mut();
            let r1: &mut T = &mut *ref_mut;
            T::react(r1, ctx, rid)
        };
        ReactionInvoker { body: Box::new(body), id }
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
