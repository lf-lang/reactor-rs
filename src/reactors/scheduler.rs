use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use priority_queue::PriorityQueue;

use crate::reactors::{ActionId, GlobalAssembler, PortId, Reactor, RunnableWorld, WorldReactor};
use crate::reactors::flowgraph::Schedulable;
use crate::reactors::id::GlobalId;
use crate::reactors::reaction::ClosedReaction;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
struct LogicalTime {
    instant: Instant,
    microstep: u32,
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self { instant: Instant::now(), microstep: 0 }
    }
}

#[derive(Eq, PartialEq, Hash)]
enum Event {
    ReactionExecute { at: LogicalTime, reaction: ClosedReaction }
}

/// Schedules actions during the execution of a reaction.
///
/// A scheduler must know which reaction is currently executing,
/// and to which reactor it belongs, in order to validate its
/// input.
pub struct Scheduler {
    schedulable: Schedulable,

    cur_logical_time: LogicalTime,
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,
}

impl Scheduler {
    fn new(schedulable: Schedulable) -> Scheduler {
        Scheduler {
            schedulable,
            cur_logical_time: <_>::default(),
            queue: PriorityQueue::new(),
        }
    }
}

pub struct ReactionCtx<'a> {
    scheduler: &'a mut Scheduler,
    reaction: Rc<ClosedReaction>,
}

impl<'a> ReactionCtx<'a> {
    pub(in super) fn new(scheduler: &'a mut Scheduler, reaction: Rc<ClosedReaction>) -> Self {
        Self { scheduler, reaction }
    }

    /// Get the value of a port.
    ///
    /// Panics if the reaction being executed hasn't declared
    /// a dependency on the given port.
    pub fn get_port<T>(&self, port: &PortId<T>) -> T where Self: Sized {
        unimplemented!()
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
    pub fn set_port<T>(&mut self, port: &PortId<T>, value: T) where Self: Sized {
        unimplemented!()
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    pub fn schedule_action(&mut self, action: ActionId, additional_delay: Option<Duration>) {
        unimplemented!()
    }
}
