
use super::assembler::{Assembler, GraphElement, NodeKind, Stamped};
use super::port::{InPort, OutPort};
use super::reaction::Reaction;

/// Trait for a reactor.
pub trait Reactor {
    // Translation strategy:
    // Ports are struct fields
    // Reactions are implemented with regular methods, described by a Reaction


    /// Returns the reactions defined on this instance.
    /// Order is important, as it determines the relative priority
    /// of reactions that should execute at the same timestamp
    fn reactions(&self) -> Vec<&Stamped<Reaction<Self>>>
        where Self: Sized;

    fn new(assembler: &mut Assembler<Self>) -> Self
        where Self: Sized;
}


impl<T> GraphElement for T
    where T: Reactor + Sized {
    fn kind(&self) -> NodeKind {
        NodeKind::Reactor
    }
}

