use std::rc::Rc;

use crate::reactors::*;
use crate::reactors::ReactionCtx;



#[test]
fn test_port_flow() {
    let mut app = crate::reactors::make_world::<AppReactor>().unwrap();

    fn test_set(v: i32, app: &RunnableWorld<AppReactor>) {
        app.producer.output_port.set(v);

        assert_eq!(v, app.relay.input_port.get());
        assert_eq!(v, app.relay.output_port.get());
        assert_eq!(v, app.consumer.input_port.get());

    }

    test_set(32, &app);
    test_set(4, &app);
}

// toplevel reactor containing the others
struct AppReactor {
    producer: Rc<RunnableReactor<ProduceReactor>>,
    relay:    Rc<RunnableReactor<PortRelay>>,
    relay2:   Rc<RunnableReactor<PortRelay>>,
    consumer: Rc<RunnableReactor<ConsumeReactor>>,
}

impl WorldReactor for AppReactor {
    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let consumer = assembler.new_subreactor::<ConsumeReactor>("consumer")?;
        let relay = assembler.new_subreactor::<PortRelay>("relay1")?;
        let relay2 = assembler.new_subreactor::<PortRelay>("relay2")?;
        let producer = assembler.new_subreactor::<ProduceReactor>("producer")?;

        assembler.bind_ports(&producer.output_port, &relay.input_port)?;
        assembler.bind_ports(&relay.output_port, &relay2.input_port)?;
        assembler.bind_ports(&relay2.output_port, &consumer.input_port)?;

        Ok(AppReactor { consumer, producer, relay, relay2 })
    }
}


struct ProduceReactor {
    output_port: PortId<i32>
}


reaction_ids!(enum ProduceReactions { Emit });


impl Reactor for ProduceReactor {
    type ReactionId = ProduceReactions;
    type State = i32;

    fn initial_state() -> Self::State {
        0
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let output_port = assembler.new_output_port::<i32>("output")?;

        assembler.reaction_affects(ProduceReactions::Emit, &output_port)?;

        Ok(ProduceReactor { output_port })
    }

    fn react(reactor: &RunnableReactor<Self>, state: &mut Self::State, reaction_id: Self::ReactionId, ctx: &mut ReactionCtx) where Self: Sized {
        match reaction_id {
            ProduceReactions::Emit => {
                *state += 1;
                ctx.set_port(&reactor.output_port, *state)
            }
        }
    }
}


struct ConsumeReactor {
    input_port: PortId<i32>
}

reaction_ids!(enum ConsumeReactions { Print });

impl Reactor for ConsumeReactor {
    type ReactionId = ConsumeReactions;
    type State = ();

    fn initial_state() -> Self::State where Self: Sized {
        ()
    }

    fn assemble(assembler: &mut Assembler<Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let input_port = assembler.new_input_port::<i32>("input")?;

        Ok(ConsumeReactor { input_port })
    }

    fn react(reactor: &RunnableReactor<Self>, _: &mut Self::State, reaction_id: Self::ReactionId, ctx: &mut ReactionCtx) where Self: Sized {
        match reaction_id {
            ConsumeReactions::Print => {
                print!("{}", ctx.get_port(&reactor.input_port))
            }
        }
    }
}

// Just binds its input to its output
// This is useless, but it tests the binding logic
struct PortRelay {
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
        let input_port = assembler.new_input_port::<i32>("input")?;
        let output_port = assembler.new_output_port::<i32>("output")?;

        assembler.bind_ports(&input_port, &output_port)?;
        Ok(PortRelay { input_port, output_port })
    }

    fn react(_: &RunnableReactor<Self>, _: &mut Self::State, _: Self::ReactionId, _: &mut ReactionCtx) where Self: Sized {
        unreachable!("Reactor declares no reaction")
    }
}
