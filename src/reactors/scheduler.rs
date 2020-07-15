use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use priority_queue::PriorityQueue;

use crate::reactors::{ActionId, GlobalAssembler, PortId, Reactor, RunnableWorld, WorldReactor};
use crate::reactors::flowgraph::FlowGraph;
use crate::reactors::id::GlobalId;
use crate::reactors::reaction::ClosedReaction;
use std::cmp::Reverse;

pub struct Schedulable {
    /// Maps port ids to a list of reactions that must be scheduled
    /// each time the port is set in a reaction.
    reactions_by_port_id: HashMap<GlobalId, Vec<Rc<ClosedReaction>>>,

}


impl Schedulable {
    pub(in super) fn new(reactions_by_port_id: HashMap<GlobalId, Vec<Rc<ClosedReaction>>>) -> Schedulable {
        Schedulable { reactions_by_port_id }
    }
}


struct LogicalTime {
    instant: Duration,
    microstep: u32,
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self { instant: Duration::from_secs(0), microstep: 0 }
    }
}

#[derive(Eq, Hash)]
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
