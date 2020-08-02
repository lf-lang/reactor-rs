//! A simple example:
//!
//! Producer -> Relay -> Consumer
//!
//! The producer schedules an "increment and send" reaction
//! every second. The consumer handles the new value and prints
//! it to the screen.
//!
//! ```shell
//! $ cargo run --bin example-forwarding
//! Received 1
//! Received 2
//! ...
//! ```


#[macro_use]
extern crate rust_reactors;

use std::borrow::Borrow;
use std::rc::Rc;
use std::time::Duration;

use rust_reactors::reactors::*;
use std::num::Wrapping;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

pub fn main() {
    let (app, mut scheduler) = make_world::<AppReactor>().unwrap();

    scheduler.launch(&app.producer.emit_action);
}

// toplevel reactor containing the others
struct AppReactor<'g> {
    producer: Rc<RunnableReactor<'g, OwnerReactor<'g>>>,
    consumer: Rc<RunnableReactor<'g, ConsumeReactor<'g>>>,
}

impl<'g> WorldReactor<'g> for AppReactor<'g> {
    fn assemble_world<'a>(assembler: &mut impl AssemblerBase<'a, 'g, Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let producer: Rc<RunnableReactor<'g, OwnerReactor>> = assembler.new_subreactor::<OwnerReactor>("producer")?;
        let consumer: Rc<RunnableReactor<'g, ConsumeReactor>> = assembler.new_subreactor::<ConsumeReactor>("consumer")?;

        assembler.bind_ports(&producer.output_port, &consumer.input_port)?;

        Ok(AppReactor { consumer, producer })
    }
}


type PV<'r> = &'r [u8];

struct OwnerReactor<'r> {
    output_port: Port<PV<'r>>,
    emit_action: ActionId,
    phantom: PhantomData<&'r ()>,
}


reaction_ids!(enum ProduceReactions { Emit, });

struct MyState {
    arr: [u8; 256],
    len: usize,
}

impl<'r> Reactor for OwnerReactor<'r> {
    type ReactionId = ProduceReactions;

    type State = MyState;

    fn initial_state() -> Self::State {
        MyState { arr: [0; 256], len: 0 }
    }

    fn assemble<'g>(assembler: &mut Assembler<'_, 'g, Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let emit_action = assembler.new_action("emit", Some(Duration::from_secs(1)), true)?;
        let output_port = assembler.new_output_port::<PV>("output")?;

        assembler.action_triggers(&emit_action, ProduceReactions::Emit)?;
        assembler.reaction_schedules(ProduceReactions::Emit, &emit_action)?;
        assembler.reaction_affects(ProduceReactions::Emit, &output_port)?;

        Ok(OwnerReactor { output_port, emit_action, phantom: PhantomData })
    }

    fn react<'g>(reactor: &RunnableReactor<'g, Self>, state: &mut Self::State, reaction_id: Self::ReactionId, ctx: &mut ReactionCtx<'_, 'g>) where Self: Sized + 'g {
        match reaction_id {
            ProduceReactions::Emit => {
                // println!("Emitting {}", *state);
                ctx.set_port(&reactor.output_port, &state.arr[0..state.len]);
                ctx.schedule_action(&reactor.emit_action, None)
            }
        }
    }
}


struct ConsumeReactor<'r> {
    input_port: Port<PV<'r>>,
}

reaction_ids!(enum ConsumeReactions { Print });

impl<'r> Reactor for ConsumeReactor<'r> {
    type ReactionId = ConsumeReactions;
    type State = ();

    fn initial_state() -> Self::State where Self: Sized {
        ()
    }

    fn assemble<'g>(assembler: &mut Assembler<'_, 'g, Self>) -> Result<Self, AssemblyError> where Self: Sized {
        let input_port = assembler.new_input_port::<PV>("input")?;

        assembler.reaction_uses(Self::ReactionId::Print, &input_port)?;

        Ok(ConsumeReactor { input_port })
    }

    fn react<'g>(reactor: &RunnableReactor<'g, Self>, _: &mut Self::State, reaction_id: Self::ReactionId, ctx: &mut ReactionCtx<'_, 'g>) where Self: Sized + 'g {
        match reaction_id {
            ConsumeReactions::Print => {
                println!("Received slice of len {}", ctx.get_port(&reactor.input_port).len())
            }
        }
    }
}
