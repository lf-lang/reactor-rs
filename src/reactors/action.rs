use std::time::Duration;
use crate::reactors::assembler::{Assembler, Linked, GraphElement, NodeKind};
use crate::reactors::reactor::Reactor;


/// An action produces events with a specific delay.
///
/// It may be scheduled by a reaction, and trigger other reactions.
///
/// Special cases:
/// - timers
/// - startup actions
///
pub struct Action {
    name: &'static str,
    delay: Duration,
    logical: bool,
}

impl GraphElement for Action {
    fn kind(&self) -> NodeKind {
        NodeKind::Action
    }
}

impl Action {
    pub fn new_physical<R: Reactor>(assembler: &mut Assembler<R>, name: &'static str, delay: Duration) -> Linked<Self> {
        Self::new(assembler, name, delay, false)
    }

    pub fn new_logical<R: Reactor>(assembler: &mut Assembler<R>, name: &'static str, delay: Duration) -> Linked<Self> {
        Self::new(assembler, name, delay, true)
    }

    fn new<R: Reactor>(assembler: &mut Assembler<R>, name: &'static str, delay: Duration, logical: bool) -> Linked<Self> {
        assembler.create_node(Action { name, delay, logical })
    }

    fn is_logical(&self) -> bool {
        self.logical
    }
}


