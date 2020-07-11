
use super::assembler::{Assembler, GraphElement, NodeKind, Linked};
use super::port::{InPort, OutPort};
use super::reaction::Reaction;

/// Trait for a reactor.
pub trait Reactor {
    // Translation strategy:
    // Ports are struct fields
    // Reactions are implemented with regular methods, described by a Reaction

    fn new(assembler: &mut Assembler<Self>) -> Self
        where Self: Sized;
}
