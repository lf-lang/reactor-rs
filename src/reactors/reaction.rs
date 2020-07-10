use crate::reactors::reactor::Reactor;

/// A reaction may be triggered by
/// - An event occurring on an input port of the reactor
/// - An action of the reactor, that was scheduled or executed
///   by another reaction
/// - todo a clock? I think that's conceptually like an event
pub struct Reaction<'a, Container>
    where Container: Reactor<'a> + Sized + 'a {

    name: &'static str,

    /// Body to execute
    /// Arguments:
    /// - the reactor instance that contains the reaction todo should this be mut?
    /// - todo the scheduler, to send events like schedule, etc
    body: fn(&'a Container),

    /// These will be resolved against the input port names
    /// of the containing reactor
    port_dependencies: Vec<&'static str>,
}

#[macro_export]
macro_rules! reaction {
    { $name:literal ($( $trigger:literal ),*) -> ($( $var:ident ),*) $body:tt } => {
        Reaction::new(
            $name,
            vec![
                $( $trigger, )*
            ],
            |reactor| {
                $( let $var = *reactor.$var.data.borrow_or_panic().get(); );*

                $body
            }
        )
    };
}

impl<'a, Container> Reaction<'a, Container>
    where Container: Reactor<'a> {

    pub fn fire(&self, c: &'a Container) {
        (self.body)(c)
    }

    pub fn new(
        name: &'static str,
        port_deps: Vec<&'static str>,
        body: fn(&'a Container),
    ) -> Reaction<'a, Container> {
        Reaction {
            name,
            port_dependencies: port_deps,
            body,
        }
    }
}
