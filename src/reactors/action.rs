use std::time::Duration;
use crate::reactors::assembler::{Assembler, Stamped, GraphElement, NodeKind};


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
    pub fn new_physical(assembler: &mut Assembler, name: &'static str, delay: Duration) -> Stamped<Self> {
        Self::new(assembler, name, delay, false)
    }

    pub fn new_logical(assembler: &mut Assembler, name: &'static str, delay: Duration) -> Stamped<Self> {
        Self::new(assembler, name, delay, true)
    }

    fn new(assembler: &mut Assembler, name: &'static str, delay: Duration, logical: bool) -> Stamped<Self> {
        assembler.create_node(Action { name, delay, logical })
    }

    fn is_logical(&self) -> bool {
        self.logical
    }
}


