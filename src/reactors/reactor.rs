use super::assembler::{Assembler, GraphElement, Stamped, NodeKind};

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
}


impl<T> GraphElement for T
    where T: Reactor + Sized {
    fn kind(&self) -> NodeKind {
        NodeKind::Reactor
    }
}


// Dummy reactor implementations

#[derive(Debug)]
pub struct ProduceReactor {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub value: Stamped<OutPort<i32>>,

    pub react_incr: Stamped<Reaction<Self>>,
}

impl ProduceReactor {
    pub fn new(assembler: &mut Assembler) -> Stamped<Self> {
        let value = OutPort::new(assembler, "value", 0);
        let react_incr = Reaction::new(
            assembler,
            "incr",
            |r: &ProduceReactor| *r.value.get_mut() += 2,
        );
        let r = assembler.create_node(ProduceReactor { value, react_incr });

        link_reaction!(
            (assembler)(&r.react_incr)
            (deps &r.value)
            (antideps)
        );

        r
    }
}

impl Reactor for ProduceReactor {
    fn reactions(&self) -> Vec<&Stamped<Reaction<Self>>> {
        vec![&self.react_incr]
    }
}

#[derive(Debug)]
pub struct ConsumeReactor {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub input: Stamped<InPort<i32>>,
    pub react_print: Stamped<Reaction<Self>>,
}

impl ConsumeReactor {
    pub fn new(assembler: &mut Assembler) -> Stamped<Self> {
        let input = InPort::new(assembler, "value");
        let react_print = Reaction::new(
            assembler,
            "print_input",
            |r: &ConsumeReactor| println!("Value is {}", *r.input.borrow_or_panic().get()),
        );
        assembler.create_node(ConsumeReactor {
            input,
            react_print,
        })
    }
}

impl Reactor for ConsumeReactor {
    fn reactions(&self) -> Vec<&Stamped<Reaction<Self>>> {
        vec![&self.react_print]
    }
}
