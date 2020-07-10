use reactors::assembler::Assembler;

use reactors::assembler::Stamped;
use reactors::port::{InPort, OutPort};
use reactors::reaction::Reaction;
use reactors::reactor::Reactor;

mod reactors;

fn main() {
    let mut assembler = Assembler::new();
    let producer = ProduceReactor::new(&mut assembler);
    let consumer = ConsumeReactor::new(&mut assembler);


    assembler.connect(&producer.value, &consumer.input);

    consumer.react_print.fire(&consumer);
    producer.react_incr.fire(&producer);
    consumer.react_print.fire(&consumer);
}


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
