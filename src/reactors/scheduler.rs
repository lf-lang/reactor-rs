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
enum Event {
    ReactionExecute { at: LogicalTime, reaction: Rc<ClosedReaction> },
    ReactionSchedule { min_at: LogicalTime, reaction: Rc<ClosedReaction> },
}

/// Schedules actions during the execution of a reaction.
///
/// A scheduler must know which reaction is currently executing,
/// and to which reactor it belongs, in order to validate its
/// input.
pub struct Scheduler {
    schedulable: Schedulable,

    cur_logical_time: LogicalTime,
    micro_step: MicroStep,
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
}

impl Scheduler {
    pub(in super) fn new(schedulable: Schedulable) -> Scheduler {
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

            let mut ctx = self.new_ctx(reaction.clone());
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

    fn new_ctx(&mut self, reaction: Rc<ClosedReaction>) -> ReactionCtx {
        ReactionCtx {
            scheduler: self,
            reaction_id: ReactionId(reaction.global_id().clone()),
            reaction,
        }
    }
}


/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
///
pub struct ReactionCtx<'a> {
    scheduler: &'a mut Scheduler,
    reaction: Rc<ClosedReaction>,
    reaction_id: ReactionId,
}

impl<'a> ReactionCtx<'a> {
    /// Get the value of a port.
    ///
    /// Panics if the reaction being executed hasn't declared
    /// a dependency on the given port.
    pub fn get_port<T>(&self, port: &Port<T>) -> T where Self: Sized, T: Copy {
        assert!(self.scheduler.schedulable.get_allowed_reads(&self.reaction_id).contains(port.port_id()),
                "Forbidden read on port {} by reaction {}. Declare the dependency explicitly during assembly",
                port.global_id(), self.reaction_id.global_id()
        );

        port.get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    ///
    /// Panics if the reaction being executed hasn't declared
    /// a dependency on the given port.
    ///
    pub fn set_port<T>(&mut self, port: &Port<T>, value: T) where Self: Sized, T: Copy {
        assert!(self.scheduler.schedulable.get_allowed_writes(&self.reaction_id).contains(port.port_id()),
                "Forbidden read on port {} by reaction {}. Declare the dependency explicitly during assembly",
                port.global_id(), self.reaction_id.global_id()
        );

        port.set(value);

        self.scheduler.enqueue_port(port.port_id());
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    pub fn schedule_action(&mut self, action: &ActionId, additional_delay: Option<Duration>) {
        assert!(self.scheduler.schedulable.get_allowed_schedules(&self.reaction_id).contains(action),
                "Forbidden schedule call on action {} by reaction {}. Declare the dependency explicitly during assembly",
                action.global_id(), self.reaction_id.global_id()
        );

        self.scheduler.enqueue_action(action, additional_delay)
    }
}
