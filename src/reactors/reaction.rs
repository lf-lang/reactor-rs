use super::reactor::Reactor;
use super::assembler::{Assembler, Stamped, GraphElement, NodeKind};
use std::fmt::{Debug, Formatter};

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
pub struct Reaction<Container>
    where Container: Reactor + Sized {
    name: &'static str,

    /// Body to execute
    /// Arguments:
    /// - the reactor instance that contains the reaction todo should this be mut?
    /// - todo the scheduler, to send events like schedule, etc
    body: fn(&Container),

}

impl<C> Debug for Reaction<C>
    where C: Reactor + Sized {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "reaction {}()", self.name)
    }
}

#[macro_export]
macro_rules! link_reaction {
    {($assembler:expr)($reaction:expr) (deps $( $dep:expr )*) (antideps $( $anti:expr )*)} => {

        {
            $(
                $assembler.reaction_link($reaction, $dep, true);
            )*
            $(
                $assembler.reaction_link($reaction, $anti, false);
            )*
        }
    };
}

impl<Container> GraphElement
for Reaction<Container>
    where Container: Reactor {
    fn kind(&self) -> NodeKind {
        NodeKind::Reaction
    }
}


impl<Container> Reaction<Container>
    where Container: Reactor {
    pub fn fire(&self, c: &Container) {
        (self.body)(c)
    }

    pub fn new(
        assembler: &mut Assembler<Container>,
        name: &'static str,
        body: fn(&Container),
    ) -> Stamped<Reaction<Container>>
        where Container: 'static {
        assembler.create_node(Reaction { body, name })
    }
}
