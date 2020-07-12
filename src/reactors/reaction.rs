use std::fmt::{Debug, Formatter};

use super::assembler::{Assembler, GraphElement, Linked, NodeKind};
use super::reactor::Reactor;
use std::marker::PhantomData;

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
pub struct Reaction<'a, R> where R: Reactor + Sized + 'a {
    /// Has no importance except for debug
    name: &'static str,

    /// Body to execute
    /// Arguments:
    /// - the reactor instance that contains the reaction todo should this be mut?
    /// - todo the scheduler, to send events like schedule, etc
    body: fn(&R, &mut R::State) -> (),

    _phantom_a: PhantomData<&'a ()>,

}

impl<'a, C> Debug for Reaction<'a, C> where C: Reactor + Sized + 'a {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "reaction {}()", self.name)
    }
}

impl<'a, C> GraphElement for Reaction<'a, C> where C: Reactor + 'a {
    fn kind(&self) -> NodeKind {
        NodeKind::Reaction
    }

    fn name(&self) -> &'static str {
        self.name
    }
}


impl<'a, R> Reaction<'a, R> where R: Reactor + 'a {
    pub fn fire(&self, reactor: &R, state: &mut R::State) {
        (self.body)(reactor, state)
    }

    pub fn new<'b>(assembler: &mut Assembler<'b, R>,
                   name: &'static str,
                   body: fn(&R, &mut R::State)) -> Linked<Reaction<'a, R>> where R: 'a, 'a : 'b {
        assembler.declare_reaction(Reaction { body, name, _phantom_a: PhantomData })
    }
}
