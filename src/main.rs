use reactors::assembler::Assembler;

use reactors::assembler::Linked;
use reactors::port::{InPort, OutPort};
use reactors::reaction::Reaction;
use reactors::framework::{Reactor, StatelessReactor};
use std::fmt::format;
use crate::reactors::assembler::RunnableReactor;
use std::marker::PhantomData;
use std::borrow::BorrowMut;
use crate::reactors::framework::{ReactionId, Enumerated, Scheduler, OutputPortId};

mod reactors;

fn main() {
    // let mut world =
    //     Assembler::<WorldReactor>::root().assemble_subreactor::<AppReactor>();
    //
    // let state_box= world.description;
    // let AppReactor { consumer, producer, .. } = state_box;
    //
    // consumer.description.react_print.fire(&consumer.description, &mut consumer.state);
    // producer.description.react_incr.fire(&producer.description, &mut producer.state);
    // consumer.description.react_print.fire(&consumer.description, &mut consumer.state);
}


pub struct ProduceReactor {
    output_port: OutputPortId<i32>
}


#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
enum ProduceReactions {
    Emit
}

impl Enumerated for ProduceReactions {
    fn list() -> Vec<Self> {
        vec![Self::Emit]
    }
}

impl ReactionId<ProduceReactor> for ProduceReactions {}


impl Reactor for ProduceReactor {
    type ReactionId = ProduceReactions;
    type State = ();


    fn initial_state() -> Self::State {
        ()
    }

    fn react(_: &mut Self::State, reaction_id: Self::ReactionId, scheduler: &dyn Scheduler) {
        match reaction_id {
            Self::ReactionId::Emit => {}
        }
    }
}


//
// // toplevel reactor containing the others todo hide as implementation detail
// pub struct WorldReactor;
//
// impl Reactor for WorldReactor {
//     type State = ();
//
//     fn new<'a>(_: &mut Assembler<'a, Self>) -> (Self, ()) where Self: 'a {
//         (WorldReactor, ())
//     }
// }

//
// // Links the other two reactors
// pub struct AppReactor<'a> {
//     producer: Linked<RunnableReactor<'a, ProduceReactor<'a>>>,
//     consumer: Linked<RunnableReactor<'a, ConsumeReactor<'a>>>,
//     _phantom_a: PhantomData<&'a ()>,
// }
//
// impl<'b> Reactor for AppReactor<'b> {
//     type State = ();
//
//     fn new<'a>(assembler: &mut Assembler<'a, Self>) -> (Self, ()) where 'b : 'a {
//         let producer = assembler.assemble_subreactor::<ProduceReactor>();
//         let consumer = assembler.assemble_subreactor::<ConsumeReactor>();
//
//         assembler.connect(&producer.description.out_value, &consumer.description.in_value);
//
//         (AppReactor { producer, consumer, _phantom_a: PhantomData }, ())
//     }
// }

//
// #[derive(Debug)]
// pub struct ProduceReactor<'a> {
//     /// This is the ouput port, that should be borrowed
//     // output: RefCell<i32>,
//     // output_borrower: Rc<RefCell<i32>>,
//
//     pub out_value: Linked<OutPort<i32>>,
//
//     pub react_incr: Linked<Reaction<'a, Self>>,
//     _phantom_a: PhantomData<&'a ()>,
//
// }
//
// impl<'b> Reactor for ProduceReactor<'b> {
//     type State = ();
//
//     fn new<'a>(assembler: &mut Assembler<'a, Self>) -> (Self, ()) where 'b : 'a {
//         let out_value = OutPort::new(assembler, "value", 0);
//         let react_incr = Reaction::new(
//             assembler,
//             "incr",
//             |r: &ProduceReactor, _| *r.out_value.get_mut() += 2,
//         );
//
//         link_reaction!((&react_incr) with (assembler)
//             (uses)
//             (affects &out_value)
//         );
//         (ProduceReactor { out_value, react_incr, _phantom_a: PhantomData }, ())
//     }
// }
//
// #[derive(Debug)]
// pub struct ConsumeReactor<'a> {
//     pub in_value: Linked<InPort<i32>>,
//     pub react_print: Linked<Reaction<'a, Self>>,
//     _phantom_a: PhantomData<&'a ()>,
// }
//
// impl<'b> Reactor for ConsumeReactor<'b> {
//     type State = ();
//
//     fn new<'a>(assembler: &mut Assembler<'a, Self>) -> (Self, ()) where 'b : 'a {
//         let in_value = InPort::new(assembler, "value");
//         let react_print = Reaction::new(
//             assembler,
//             "print_input",
//             |r: &ConsumeReactor, _| println!("Value is {}", port_value!(*r.in_value)),
//         );
//
//
//         link_reaction!((&react_print) with (assembler)
//             (uses &in_value)
//             (affects)
//         );
//
//         (ConsumeReactor { in_value, react_print, _phantom_a: PhantomData }, ())
//     }
// }
