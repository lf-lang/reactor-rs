use crate::reactors::assembler::{Assembler, RunnableReactor, AssemblyError};
use crate::reactors::framework::{Reactor, Scheduler};
use crate::reactors::ports::PortId;
use crate::reactors::util::{Enumerated, Named, Nothing};
use crate::reactors::world::WorldReactor;
use std::convert::TryInto;

mod reactors;

fn main() {
    let mut app = Assembler::<AppReactor>::make_world().unwrap();

    fn test_set(v: i32, app: &RunnableReactor<AppReactor>) {
        app.consumer.input_port.set(v);

        assert_eq!(v, app.relay.input_port.get());
        assert_eq!(v, app.relay.output_port.get());
        assert_eq!(v, app.consumer.input_port.get());

    }

    test_set(32, &app);
    test_set(4, &app);
    println!("Ok!")

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
    relay: RunnableReactor<PortRelay>,
    consumer: RunnableReactor<ConsumeReactor>,
}

impl WorldReactor for AppReactor {
    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let consumer = assembler.new_subreactor::<ConsumeReactor>("consumer")?;
        let relay = assembler.new_subreactor::<PortRelay>("relay")?;
        let producer = assembler.new_subreactor::<ProduceReactor>("producer")?;

        assembler.bind_ports(&producer.output_port, &relay.input_port)?;
        assembler.bind_ports(&relay.output_port, &consumer.input_port)?;

        Ok(AppReactor { consumer, producer, relay })
    }
}


pub struct ProduceReactor {
    output_port: PortId<i32>
}


reaction_ids!(pub enum ProduceReactions { Emit });


impl Reactor for ProduceReactor {
    type ReactionId = ProduceReactions;
    type State = i32;


    fn initial_state() -> Self::State {
        0
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let output_port = assembler.new_output_port::<i32>("output");

        assembler.reaction_affects(ProduceReactions::Emit, &output_port);

        Ok(ProduceReactor { output_port })
    }

    fn react(reactor: &RunnableReactor<Self>, state: &mut Self::State, reaction_id: Self::ReactionId, scheduler: &mut Scheduler) where Self: Sized {
        match reaction_id {
            ProduceReactions::Emit => {
                *state += 1;
                scheduler.set_port(&reactor.output_port, *state)
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

    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let input_port = assembler.new_input_port::<i32>("input");

        Ok(ConsumeReactor { input_port })
    }

    fn react(reactor: &RunnableReactor<Self>, _: &mut Self::State, reaction_id: Self::ReactionId, scheduler: &mut Scheduler) where Self: Sized {
        match reaction_id {
            ConsumeReactions::Print => {
                print!("{}", scheduler.get_port(&reactor.input_port))
            }
        }
    }
}

// Just binds its input to its output
// This is useless, but it tests the binding logic
pub struct PortRelay {
    input_port: PortId<i32>,
    output_port: PortId<i32>,
}

impl Reactor for PortRelay {
    type ReactionId = Nothing;
    type State = ();

    fn initial_state() -> Self::State where Self: Sized {
        ()
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let input_port = assembler.new_input_port::<i32>("input");
        let output_port = assembler.new_output_port::<i32>("output");

        assembler.bind_ports(&input_port, &output_port)?;
        Ok(PortRelay { input_port, output_port })
    }

    fn react(_: &RunnableReactor<Self>, _: &mut Self::State, _: Self::ReactionId, _: &mut Scheduler) where Self: Sized {
        unreachable!("Reactor declares no reaction")
    }
}
