use std::borrow::Borrow;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use priority_queue::PriorityQueue;

use crate::reactors::{ActionId, GlobalAssembler, Port, Reactor, WorldReactor};
use crate::reactors::flowgraph::Schedulable;
use crate::reactors::id::{GlobalId, Identified, PortId, ReactionId};
use crate::reactors::reaction::ClosedReaction;
use std::ops::{DerefMut, Deref};
use std::marker::PhantomData;

type MicroStep = u32;

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
enum Event<'g> {
    ReactionExecute { at: LogicalTime, reaction: Rc<ClosedReaction<'g>> },
    ReactionSchedule { min_at: LogicalTime, reaction: Rc<ClosedReaction<'g>> },
}

/// Directs execution of the whole reactor graph.
pub struct Scheduler<'g> {
    schedulable: Schedulable<'g>,

    cur_logical_time: LogicalTime,
    micro_step: MicroStep,
    queue: PriorityQueue<Event<'g>, Reverse<LogicalTime>>,
}

impl<'g> Scheduler<'g> {
    // todo logging

    pub(in super) fn new(schedulable: Schedulable<'g>) -> Self {
        Scheduler {
            schedulable,
            cur_logical_time: <_>::default(),
            micro_step: 0,
            queue: PriorityQueue::new(),
        }
    }

    pub fn launch(&mut self, startup_action: &ActionId) {
        self.enqueue_action(startup_action, None);
        while !self.queue.is_empty() {
            self.step()
        }
    }

    fn step(&mut self) {
        if let Some((event, Reverse(time))) = self.queue.pop() {
            let (reaction) = match event {
                Event::ReactionExecute { reaction, .. } => reaction,
                Event::ReactionSchedule { reaction, .. } => reaction
            };

            self.catch_up_physical_time(time);
            self.cur_logical_time = time;

            let mut ctx = ReactionCtx {
                scheduler: self,
                reaction_id: ReactionId((*reaction).global_id().clone()),
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

    fn enqueue_port(&mut self, port_id: &PortId) {
        // todo possibly, reactions must be scheduled at most once per logical time step?
        for reaction in self.schedulable.get_downstream_reactions(port_id) {
            let evt = Event::ReactionExecute { at: self.cur_logical_time, reaction: Rc::clone(reaction) };
            self.queue.push(evt, Reverse(self.cur_logical_time));
        }
    }

    fn enqueue_action(&mut self, action_id: &ActionId, additional_delay: Option<Duration>) {
        let min_delay = action_id.min_delay() + additional_delay.unwrap_or(Duration::from_secs(0));

        self.micro_step += 1;
        let eta = LogicalTime {
            instant: self.cur_logical_time.instant + min_delay,
            microstep: self.micro_step,
        };

        for reaction in self.schedulable.get_triggered_reactions(action_id) {
            let evt = Event::ReactionSchedule { min_at: eta, reaction: Rc::clone(reaction) };
            self.queue.push(evt, Reverse(eta));
        }
    }
}


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
///
pub struct ReactionCtx<'a, 'g> {
    scheduler: &'a mut Scheduler<'g>,
    reaction_id: ReactionId,
}

impl<'a, 'g> ReactionCtx<'a, 'g> {
    /// Get the value of a port at this time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_uses](super::Assembler::reaction_uses)).
    ///
    pub fn get_port<T>(&self, port: &Port<T>) -> T where Self: Sized, T: Copy {
        assert!(self.scheduler.schedulable.get_allowed_reads(&self.reaction_id).contains(port.port_id()),
                "Forbidden read on port {} by reaction {}. Declare the dependency explicitly during assembly",
                port.global_id(), self.reaction_id.global_id()
        );

        port.copy_get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    ///
    /// TODO would be possible to get a mutable ref to the port internal value instead
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given port ([reaction_affects](super::Assembler::reaction_affects)).
    ///
    pub fn set_port<T>(&mut self, port: &Port<T>, value: T) where Self: Sized, T: Copy {
        self.assert_has_write_access(port);

        port.set(value);

        self.scheduler.enqueue_port(port.port_id());
    }

    pub fn get_port_mut<'p, T>(&mut self, port: &'p Port<T>) -> impl DerefMut<Target=T> + 'p
        where Self: Sized {
        self.assert_has_write_access(port);
        self.scheduler.enqueue_port(port.port_id()); // FIXME we don't know if this will actually be set
        port.get_mut()
    }


    fn assert_has_write_access<T>(&mut self, port: &Port<T>) {
        assert!(self.scheduler.schedulable.get_allowed_writes(&self.reaction_id).contains(port.port_id()),
                "Forbidden read on port {} by reaction {}. Declare the dependency explicitly during assembly",
                port.global_id(), self.reaction_id.global_id()
        );
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// # Panics
    ///
    /// If the reaction being executed has not declared its
    /// dependency on the given action ([reaction_schedules](super::Assembler::reaction_schedules)).
    pub fn schedule_action(&mut self, action: &ActionId, additional_delay: Option<Duration>) {
        assert!(self.scheduler.schedulable.get_allowed_schedules(&self.reaction_id).contains(action),
                "Forbidden schedule call on action {} by reaction {}. Declare the dependency explicitly during assembly",
                action.global_id(), self.reaction_id.global_id()
        );

        self.scheduler.enqueue_action(action, additional_delay)
    }
}
