use std::fmt::{Debug, Formatter};

use super::assembler::{Assembler, GraphElement, Linked, NodeKind};
use super::reactor::Reactor;

/// A reaction is some managed executable code, owned by a reactor.
///
/// Components of a reaction n of a reactor C:
///
/// - dependencies, a subset of
///   - the input ports of C
///   - the output ports of the reactors contained in C
///
/// - triggers, a subset of
///   - the dependencies of n
///   - the actions of C
///
/// - an executable body
///
/// - antidependencies, a subset of
///   - the output ports of C
///   - the input ports of the reactors contained in C
///
/// - a set of schedulable actions, which is the subset of the actions of C
/// for which n can generate events
///
/// Note that all of this information (except the body) is
/// encoded into the graph at assembly time, it's not part of this struct.
///
pub struct Reaction<R>
    where R: Reactor + Sized {
    /// Has no importance except for debug
    name: &'static str,

    /// Body to execute
    /// Arguments:
    /// - the reactor instance that contains the reaction todo should this be mut?
    /// - todo the scheduler, to send events like schedule, etc
    body: fn(&R),

}

impl<C> Debug for Reaction<C>
    where C: Reactor + Sized {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "reaction {}()", self.name)
    }
}

impl<R> GraphElement for Reaction<R>
    where R: Reactor {
    fn kind(&self) -> NodeKind {
        NodeKind::Reaction
    }
}


impl<R> Reaction<R>
    where R: Reactor {
    pub fn fire(&self, c: &R) {
        (self.body)(c)
    }

    pub fn new(assembler: &mut Assembler<R>, name: &'static str, body: fn(&R)) -> Linked<Reaction<R>>
        where R: 'static {
        assembler.declare_reaction(Reaction { body, name })
    }
}
