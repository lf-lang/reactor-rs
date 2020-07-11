use reactors::assembler::Assembler;

use reactors::assembler::Stamped;
use reactors::port::{InPort, OutPort};
use reactors::reaction::Reaction;
use reactors::reactor::Reactor;

mod reactors;

fn main() {
    let mut assembler = Assembler::<WorldReactor>::root();
    let producer = assembler.assemble_subreactor::<ProduceReactor>();
    let consumer = assembler.assemble_subreactor::<ConsumeReactor>();

    assembler.connect(&producer.state.value, &consumer.state.input);

    consumer.state.react_print.fire(&consumer.state);
    producer.state.react_incr.fire(&producer.state);
    consumer.state.react_print.fire(&consumer.state);
}

// toplevel reactor containing the others
pub struct WorldReactor;

impl Reactor for WorldReactor {
    fn reactions(&self) -> Vec<&Stamped<Reaction<Self>>> where Self: Sized {
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

    pub value: Stamped<OutPort<i32>>,

    pub react_incr: Stamped<Reaction<Self>>,
}

impl Reactor for ProduceReactor {
    fn reactions(&self) -> Vec<&Stamped<Reaction<Self>>> {
        vec![&self.react_incr]
    }
    fn new(assembler: &mut Assembler<ProduceReactor>) -> Self {
        let value = OutPort::new(assembler, "value", 0);
        let react_incr = Reaction::new(
            assembler,
            "incr",
            |r: &ProduceReactor| *r.value.get_mut() += 2,
        );

        link_reaction!(
            (assembler)(&react_incr)
            (deps)
            (antideps &value)
        );
        ProduceReactor { value, react_incr }
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

impl Reactor for ConsumeReactor {
    fn reactions(&self) -> Vec<&Stamped<Reaction<Self>>> {
        vec![&self.react_print]
    }

    fn new(assembler: &mut Assembler<ConsumeReactor>) -> Self {
        let input = InPort::new(assembler, "value");
        let react_print = Reaction::new(
            assembler,
            "print_input",
            |r: &ConsumeReactor| println!("Value is {}", *r.input.borrow_or_panic().get()),
        );


        link_reaction!(
            (assembler)(&react_print)
            (deps &input)
            (antideps)
        );

        ConsumeReactor { input, react_print }
    }
}
