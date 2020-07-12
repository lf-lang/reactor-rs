use reactors::assembler::Assembler;

use reactors::assembler::Linked;
use reactors::port::{InPort, OutPort};
use reactors::reaction::Reaction;
use reactors::reactor::Reactor;
use std::fmt::format;
use crate::reactors::assembler::RunnableReactor;
use std::marker::PhantomData;
use std::borrow::BorrowMut;

mod reactors;

fn main() {
    let mut world =
        Assembler::<WorldReactor>::root().assemble_subreactor::<AppReactor>();

    let state_box= world.state.borrow_mut();
    let AppReactor { consumer, producer, .. } = state_box;

    consumer.state.react_print.fire(&mut consumer.state);
    producer.state.react_incr.fire(&mut producer.state);
    consumer.state.react_print.fire(&mut consumer.state);
}

// toplevel reactor containing the others todo hide as implementation detail
pub struct WorldReactor;

impl Reactor for WorldReactor {
    fn new<'a>(_: &mut Assembler<'a, Self>) -> Self where Self: 'a {
        WorldReactor
    }
}


// Links the other two reactors
pub struct AppReactor<'a> {
    producer: Linked<RunnableReactor<'a, ProduceReactor<'a>>>,
    consumer: Linked<RunnableReactor<'a, ConsumeReactor<'a>>>,
    _phantom_a: PhantomData<&'a ()>,
}

impl<'b> Reactor for AppReactor<'b> {
    fn new<'a>(assembler: &mut Assembler<'a, Self>) -> Self where 'b : 'a {
        let producer = assembler.assemble_subreactor::<ProduceReactor>();
        let consumer = assembler.assemble_subreactor::<ConsumeReactor>();

        assembler.connect(&producer.state.out_value, &consumer.state.in_value);

        AppReactor { producer, consumer, _phantom_a: PhantomData }
    }
}


#[derive(Debug)]
pub struct ProduceReactor<'a> {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub out_value: Linked<OutPort<i32>>,

    pub react_incr: Linked<Reaction<'a, Self>>,
    _phantom_a: PhantomData<&'a ()>,

}

impl<'b> Reactor for ProduceReactor<'b> {
    fn new<'a>(assembler: &mut Assembler<'a, Self>) -> Self where 'b : 'a {
        let out_value = OutPort::new(assembler, "value", 0);
        let react_incr = Reaction::new(
            assembler,
            "incr",
            |r: &mut ProduceReactor| *r.out_value.get_mut() += 2,
        );

        link_reaction!((&react_incr) with (assembler)
            (uses)
            (affects &out_value)
        );
        ProduceReactor { out_value, react_incr, _phantom_a: PhantomData }
    }
}

#[derive(Debug)]
pub struct ConsumeReactor<'a> {
    pub in_value: Linked<InPort<i32>>,
    pub react_print: Linked<Reaction<'a, Self>>,
    _phantom_a: PhantomData<&'a ()>,
}

impl<'b> Reactor for ConsumeReactor<'b> {
    fn new<'a>(assembler: &mut Assembler<'a, Self>) -> Self where 'b : 'a {
        let in_value = InPort::new(assembler, "value");
        let react_print = Reaction::new(
            assembler,
            "print_input",
            |r: &mut ConsumeReactor| println!("Value is {}", port_value!(*r.in_value)),
        );


        link_reaction!((&react_print) with (assembler)
            (uses &in_value)
            (affects)
        );

        ConsumeReactor { in_value, react_print, _phantom_a: PhantomData }
    }
}
