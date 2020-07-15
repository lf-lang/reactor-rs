use std::time::Duration;

use crate::reactors::{ActionId, PortId};

/// Schedules actions during the execution of a reaction.
///
/// A scheduler must know which reaction is currently executing,
/// and to which reactor it belongs, in order to validate its
/// input.
pub struct Scheduler;

impl Scheduler {
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
