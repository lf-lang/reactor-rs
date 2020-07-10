
use crate::reactors::assembler::{Assembler, GraphElement, Stamped, NodeKind};

use super::port::{InPort, OutPort};
use super::reaction::Reaction;

/// Trait for a reactor.
pub trait Reactor<'a> {
    // Translation strategy:
    // Ports are struct fields
    // Reactions are implemented with regular methods, described by a Reaction

}


impl<'a, T> GraphElement<'a> for T
    where T: Reactor<'a> + Sized {
    fn kind(&self) -> NodeKind {
        NodeKind::Reactor
    }
}


// Dummy reactor implementations

#[derive(Debug)]
pub struct ProduceReactor<'a> {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub value: Stamped<'a, OutPort<i32>>,

    pub react_incr: Stamped<'a, Reaction<'a, Self>>
}

impl<'a> ProduceReactor<'a> {
    pub fn new(assembler: &mut Assembler<'a>) -> Stamped<'a, Self> {
        let r = assembler.create_node(ProduceReactor {
            value: OutPort::new(assembler, "value", 0),
            react_incr: Reaction::new(
                assembler,
                "incr",
                |r| *r.value.get_mut() += 2
            )
        });

        link_reaction! (
            (assembler)(r.react_incr)
            (deps r.value)
            (antideps)
        );

        r
    }
}

impl<'a> Reactor<'a> for ProduceReactor<'a> {

}

#[derive(Debug)]
pub struct ConsumeReactor<'a> {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub input: Stamped<'a, InPort<i32>>,
    pub react_print: Stamped<'a, Reaction<'a, Self>>,
}

impl<'a> ConsumeReactor<'a> {
    pub fn new(assembler: &mut Assembler<'a>) -> Stamped<'a, Self> {
        let input = InPort::new(assembler, "value");
        assembler.create_node(ConsumeReactor {
            input,
            react_print: Reaction::new(
                assembler,
                "print_input",
                |r| println!("Value is {}", *r.input.borrow_or_panic().get()),
            ),
        })
    }
}


impl<'a> Reactor<'a> for ConsumeReactor<'a> {

}
