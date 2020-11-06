use std::cell::{Ref, RefMut, Cell};
use std::cmp::Reverse;
use std::fmt::Debug;
use std::ops::Deref;
use std::rc::Rc;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;
use crate::runtime::ports::{Port, InputPort, OutputPort};

type MicroStep = u128;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
struct LogicalTime {
    instant: Instant,
    microstep: MicroStep,
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self { instant: Instant::now(), microstep: 0 }
    }
}

#[derive(Eq, PartialEq, Hash)]
enum Event {
    ReactionExecute { at: LogicalTime, reaction: ReactionInvoker },
    ReactionSchedule { min_at: LogicalTime, reaction: ReactionInvoker },
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

    fn enqueue_port<T>(&mut self, port_id: &Rc<PortCell<T>>) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in port_id.downstream {
            let evt = Event::ReactionExecute { at: self.cur_logical_time, reaction: Rc::clone(&reaction) };
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

        for reaction in action.downstream {
            let evt = Event::ReactionSchedule { min_at: eta, reaction: Rc::clone(&reaction) };
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
}

impl<'a> Ctx<'a> {
    /// Get the value of a port at this time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_uses](super::Assembler::reaction_uses)).
    ///
    pub fn get<T>(&self, port: &InputPort<T>) -> Option<T> {
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
        port.set(value);
        self.scheduler.enqueue_port(&port.cell);
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
}


pub struct Action {
    delay: Duration,
    logical: bool,
    downstream: Vec<ReactionInvoker>,
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


pub trait ReactorWrapper {
    type ReactionId: Copy;
    type Wrapped;
    type Params;

    fn assemble(args: Self::Params) -> Self;
    fn start(&mut self, ctx: &mut Ctx);
    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId);
}

pub(in super) struct ReactionInvoker {
    body: Box<dyn FnMut(&mut Ctx)>,
    id: i32,
}

impl ReactionInvoker {
    fn fire(&self, ctx: &mut Ctx) {
        (self.body)(ctx)
    }

    fn new<T: ReactorWrapper + Sized>(id: i32, mut reactor: Rc<T>, reaction: T::ReactionId) -> ReactionInvoker {
        let body = Box::new(move |&mut ctx| {
            reactor.react(ctx, reaction)
        });
        ReactionInvoker { body, id }
    }
}
