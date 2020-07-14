use std::time::Duration;

use crate::reactors::action::ActionId;
use crate::reactors::assembler::{Assembler, RunnableReactor, AssemblyError};
use crate::reactors::util::{Enumerated, Named};
use crate::reactors::ports::PortId;
use std::hash::Hash;

/// Describes the structure of a reactor.
///
/// Instances are created by cooperating with an [Assembler].
/// Sub-components of the reactor (ports, sub-reactors, etc)
/// are created by the assembler and should be stored in
/// instances. They're immutable.
///
/// Mutable state variables are split into a [State] associated
/// type, that is managed by the framework.
pub trait Reactor {
    /// Enumerates the reactions available for this reactor.
    /// This is used as input to the [react] function.
    type ReactionId: Ord + Eq + Hash + Enumerated + Named + Sized + Copy;

    /// The type for the internal state of the reactor.
    ///
    /// The self instance should not contain the internal state variables.
    /// It couldn't use it anyway, since the [react] method take a `self`
    /// argument as an immutable reference.
    ///
    /// Use `()` for a stateless reactor.
    type State: Sized;

    /// Produce the initial state. This is passed by reference
    /// to the [react] function.
    fn initial_state() -> Self::State where Self: Sized;

    /// Initializes the structure of this reactor.
    /// This will create subcomponents and link them using the [Assembler].
    ///
    /// The returned instance is wrapped into a [RunnableReactor] for execution.
    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized;

    /// Execute a reaction of this reactor.
    fn react(
        // This is the assembled reactor. It's immutable in this method
        reactor: &RunnableReactor<Self>,
        // A mutable reference to the internal reactor state
        state: &mut Self::State,
        // ID of the reaction to execute
        reaction_id: Self::ReactionId,
        // Scheduler instance, that can make the reaction affect the event queue
        scheduler: &mut Scheduler,
    ) where Self: Sized; // todo this could return a Result
}


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
    pub fn get_port<T>(& self, port: &PortId<T>) -> T where Self: Sized {
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
