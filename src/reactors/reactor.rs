use super::assembler::{Assembler, GraphElement, NodeKind, Linked};
use super::port::{InPort, OutPort};
use super::reaction::Reaction;

/// Trait for a reactor.
pub trait Reactor {
    type State;

    // Translation strategy:
    // Ports are struct fields
    // Reactions are implemented with regular methods, described by a Reaction

    fn new<'a>(assembler: &mut Assembler<'a, Self>) -> (Self, Self::State) where Self: Sized + 'a;
}
