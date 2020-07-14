use crate::reactors::assembler::{Assembler, RunnableReactor};
use crate::reactors::framework::{Reactor, Scheduler};
use crate::reactors::ports::PortId;
use crate::reactors::util::{Enumerated, Named};
use crate::reactors::world::WorldReactor;
use std::convert::TryInto;

mod reactors;

fn main() {
    let mut app = Assembler::<AppReactor>::make_world();


    //
    // let state_box= world.description;
    // let AppReactor { consumer, producer, .. } = state_box;
    //
    // consumer.description.react_print.fire(&consumer.description, &mut consumer.state);
    // producer.description.react_incr.fire(&producer.description, &mut producer.state);
    // consumer.description.react_print.fire(&consumer.description, &mut consumer.state);
}

// toplevel reactor containing the others
pub struct AppReactor {
    producer: RunnableReactor<ProduceReactor>,
    consumer: RunnableReactor<ConsumeReactor>,
}

impl WorldReactor for AppReactor {
    fn assemble(assembler: &mut Assembler<Self>) -> Self where Self: Sized {
        let consumer = assembler.new_subreactor::<ConsumeReactor>("consumer");
        let producer = assembler.new_subreactor::<ProduceReactor>("producer");

        assembler.bind_ports(&producer.output_port, &consumer.input_port);

        AppReactor { consumer, producer }
    }
}


pub struct ProduceReactor {
    output_port: PortId<i32>
}


reaction_ids!(pub enum ProduceReactions { Emit });


impl Reactor for ProduceReactor {
    type ReactionId = ProduceReactions;
    type State = ();


    fn initial_state() -> Self::State {
        ()
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Self where Self: Sized {
        let output_port = assembler.new_output_port::<i32>("output", 0);
        assembler.reaction_affects(ProduceReactions::Emit, &output_port);
        ProduceReactor { output_port }
    }

    fn react(reactor: &RunnableReactor<Self>, _: &mut Self::State, reaction_id: Self::ReactionId, scheduler: &mut Scheduler) where Self: Sized {
        match reaction_id {
            ProduceReactions::Emit => {
                scheduler.set_port(&reactor.output_port, scheduler.get_port(&reactor.output_port) + 1)
            }
        }
    }
}


pub struct ConsumeReactor {
    input_port: PortId<i32>
}

reaction_ids!(pub enum ConsumeReactions { Print });

impl Reactor for ConsumeReactor {
    type ReactionId = ConsumeReactions;
    type State = ();

    fn initial_state() -> Self::State where Self: Sized {
        ()
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Self where Self: Sized {
        let input_port = assembler.new_input_port("input", 0);

        ConsumeReactor { input_port }
    }

    fn react(reactor: &RunnableReactor<Self>, _: &mut Self::State, reaction_id: Self::ReactionId, scheduler: &mut Scheduler) where Self: Sized {
        match reaction_id {
            ConsumeReactions::Print => {
                print!("{}", scheduler.get_port(&reactor.input_port))
            }
        }
    }
}
