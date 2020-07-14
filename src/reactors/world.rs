use crate::reactors::assembler::{Assembler, AssemblyError, RunnableReactor};
use crate::reactors::framework::{Reactor, Scheduler};
use crate::reactors::util::Nothing;

/// A top-level reactor. Such a reactor may not declare input
/// or output ports, only sub-reactors and connections between
/// them. TODO this is not checked anywhere
pub trait WorldReactor {
    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized;
}

impl<T> Reactor for T where T: WorldReactor {
    type ReactionId = Nothing;
    type State = ();

    fn initial_state() -> Self::State where Self: Sized {
        ()
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        Self::assemble(assembler)
    }

    fn react(_: &RunnableReactor<Self>, _: &mut Self::State, _: Self::ReactionId, _: &mut Scheduler) where Self: Sized {
        unreachable!("Reactor declares no reaction")
    }
}
