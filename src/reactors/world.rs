use std::collections::HashMap;
use std::rc::Rc;

use crate::reactors::{Assembler, AssemblerBase, AssemblyError, GlobalAssembler, ReactionCtx, Reactor, RunnableReactor};
use crate::reactors::id::GlobalId;
use crate::reactors::reaction::ClosedReaction;
use crate::reactors::util::Nothing;

/// A top-level reactor. Such a reactor may only declare
/// sub-reactors and connections between them. TODO this is not checked anywhere
pub trait WorldReactor<'g> {
    /// Assemble the structure of this reactor. The parameter
    /// does not allow all operations of
    fn assemble_world<'a>(assembler: &mut impl AssemblerBase<'a, 'g, Self>) -> Result<Self, AssemblyError> where Self: Sized;
}

impl<'g, T> Reactor for T where T: WorldReactor<'g> {
    type ReactionId = Nothing;
    type State = ();

    fn initial_state() -> Self::State where Self: Sized {
        ()
    }

    fn assemble<'gp>(_: &mut Assembler<'_, 'gp, Self>) -> Result<Self, AssemblyError> where Self: Sized {
        panic!("Cannot assemble world (todo)")
    }

    fn react<'gp>(_: &RunnableReactor<'gp, Self>, _: &mut Self::State, _: Self::ReactionId, _: &mut ReactionCtx<'_, 'gp>) where Self: Sized + 'gp {
        unreachable!("Reactor declares no reaction")
    }
}
