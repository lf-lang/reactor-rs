use reactors::assembler::Assembler;

use reactors::assembler::Linked;
use reactors::port::{InPort, OutPort};
use reactors::reaction::Reaction;
use reactors::reactor::Reactor;
use std::fmt::format;

mod reactors;

fn main() {
    let mut assembler = Assembler::<WorldReactor>::root();
    let producer = assembler.assemble_subreactor::<ProduceReactor>();
    let consumer = assembler.assemble_subreactor::<ConsumeReactor>();

    assembler.connect(&producer.state.out_value, &consumer.state.in_value);

    consumer.state.react_print.fire(&consumer.state);
    producer.state.react_incr.fire(&producer.state);
    consumer.state.react_print.fire(&consumer.state);
}

// toplevel reactor containing the others
pub struct WorldReactor;

impl Reactor for WorldReactor {
    fn reactions(&self) -> Vec<&Linked<Reaction<Self>>> where Self: Sized {
        vec![]
    }

    fn new(_: &mut Assembler<Self>) -> Self where Self: Sized {
        WorldReactor
    }
}


#[derive(Debug)]
pub struct ProduceReactor {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub out_value: Linked<OutPort<i32>>,

    pub react_incr: Linked<Reaction<Self>>,
}

impl Reactor for ProduceReactor {
    fn reactions(&self) -> Vec<&Linked<Reaction<Self>>> {
        vec![&self.react_incr]
    }
    fn new(assembler: &mut Assembler<ProduceReactor>) -> Self {
        let out_value = OutPort::new(assembler, "value", 0);
        let react_incr = Reaction::new(
            assembler,
            "incr",
            |r: &ProduceReactor| *r.out_value.get_mut() += 2,
        );

        link_reaction!((&react_incr) with (assembler)
            (uses)
            (affects &out_value)
        );
        ProduceReactor { out_value, react_incr }
    }
}

#[derive(Debug)]
pub struct ConsumeReactor {
    pub in_value: Linked<InPort<i32>>,
    pub react_print: Linked<Reaction<Self>>,
}

impl Reactor for ConsumeReactor {
    fn reactions(&self) -> Vec<&Linked<Reaction<Self>>> {
        vec![&self.react_print]
    }

    fn new(assembler: &mut Assembler<ConsumeReactor>) -> Self {
        let in_value = InPort::new(assembler, "value");
        let react_print = Reaction::new(
            assembler,
            "print_input",
            |r: &ConsumeReactor| println!("Value is {}", port_value!(*r.in_value)),
        );


        link_reaction!((&react_print) with (assembler)
            (uses &in_value)
            (affects)
        );

        ConsumeReactor { in_value, react_print }
    }
}
