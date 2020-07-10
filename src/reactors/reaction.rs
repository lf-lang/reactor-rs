use crate::reactors::reactor::Reactor;
use crate::reactors::assembler::{Assembler, Stamped, GraphElement, NodeKind};

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
#[derive(Debug)]
pub struct Reaction<'a, Container>
    where Container: Reactor<'a> + Sized + 'a {
    name: &'static str,

    /// Body to execute
    /// Arguments:
    /// - the reactor instance that contains the reaction todo should this be mut?
    /// - todo the scheduler, to send events like schedule, etc
    body: fn(&'a Container),

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

impl<'a, Container> GraphElement<'a>
for Reaction<'a, Container>
    where Container: Reactor<'a> {
    fn kind(&self) -> NodeKind {
        NodeKind::Reaction
    }
}


impl<'a, Container> Reaction<'a, Container>
    where Container: Reactor<'a> {

    pub fn fire(&self, c: &'a Container) {
        (self.body)(c)
    }

    pub fn new(
        assembler: &mut Assembler<'a>,
        name: &'static str,
        body: fn(&'a Container),
    ) -> Stamped<'a, Reaction<'a, Container>> {
        assembler.create_node(Reaction { body, name })
    }
}
