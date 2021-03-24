#![allow(unused)]
#[macro_use]
extern crate rust_reactors;


use std::cell::{RefCell, RefMut};
use std::cell::Cell;
use std::io::stdin;
use std::pin::Pin;
use std::process::exit;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use rand::Rng;

use rust_reactors::reaction_ids;
use rust_reactors::reaction_ids_helper;
use rust_reactors::runtime::*;

// this is a manual translation of
// https://github.com/icyphy/lingua-franca/blob/master/test/Cpp/StructPrint.lf

/*


main reactor StructAsType {
    s = new Source();
    p = new Print();
    s.out -> p.in;
}
 */
fn main() {
    let mut rid = 0;

    // --- s = new Source();
    let mut s_cell = SourceAssembler::assemble(&mut rid, ());

    // --- p = new Print();
    // note: default parameters are hoisted here
    let mut p_cell = PrintAssembler::assemble(&mut rid, (42, "Earth"));

    {
        let mut p = s_cell._rstate.lock().unwrap();
        let mut g = p_cell._rstate.lock().unwrap();

        // --- p.out -> g.prompt;
        bind_ports(&mut p.out, &mut g.input);
    }

    let mut scheduler = SyncScheduler::new();

    scheduler.startup(|mut starter| {
        starter.start(&mut s_cell);
        starter.start(&mut p_cell);
    });
    scheduler.launch_async(Duration::from_millis(10)).join().unwrap();
}


#[derive(Debug, Copy, Clone)]
struct Hello {
    name: &'static str,
    value: i32,
}

struct Source;

impl Source {
    /// reaction(startup) -> out {=
    //        // create a dynamically allocated mutable Hello object
    //        auto hello = reactor::make_mutable_value<Hello>();
    //        hello->name = "Earth";
    //        hello->value = 42;
    //        // this implicitly converts the mutable value to an immutable value
    //        out.set(std::move(hello));
    //  =}
    fn react_startup(mut link: SchedulerLink,
                     ctx: &mut LogicalCtx,
                     out: &mut OutputPort<Hello>) {
        // Create our Hello struct
        let mut hello = Hello { name: "Venus", value: 42 };
        hello.name = "Earth";
        // implicitly moved
        ctx.set(out, hello);
        // hello.name = "Mars"; // error!
    }
}

/*
    input another:bool;
    output out:bool;
    logical action prompt(2 secs);
 */
struct SourceDispatcher {
    _impl: Source,
    out: OutputPort<Hello>,
}

impl ReactorDispatcher for SourceDispatcher {
    type ReactionId = Nothing;
    type Wrapped = Source;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        SourceDispatcher {
            _impl: Source,
            out: Default::default(),
        }
    }

    fn react(&mut self, _: &mut LogicalCtx, _: Self::ReactionId) {}
}


struct SourceAssembler {
    _rstate: Arc<Mutex</*{{*/SourceDispatcher/*}}*/>>,
}

impl ReactorAssembler for /*{{*/SourceAssembler/*}}*/ {
    type RState = /*{{*/SourceDispatcher/*}}*/;

    fn start(&mut self, link: SchedulerLink, ctx: &mut LogicalCtx) {
        Source::react_startup(link, ctx, &mut self._rstate.lock().unwrap().out);
    }


    fn assemble(reactor_id: &mut u32, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = *reactor_id;
        *reactor_id += 1;
        let mut reaction_id = 0;

        Self {
            _rstate,
        }
    }
}

struct Print {
    expected_value: i32,
    expected_name: &'static str,
}

impl Print {
    //  reaction(in) {=
    //         // get a reference to the received struct for convenience
    //         auto& s = *in.get();
    //         std::cout << "Received: name = " << s.name << ", value = " << s.value << '\n';
    //         if (s.value != expected_value || s.name != expected_name) {
    //             std::cerr << "ERROR: Expected name = " << expected_name << ", value = " << expected_value << '\n';
    //             exit(1);
    //         }
    //  =}
    fn react_print(&mut self, ctx: &mut LogicalCtx, _input: &InputPort<Hello>) {
        let h = ctx.get(_input).unwrap();
        println!("Receive {:?}", h);
        if h.value != self.expected_value || h.name != self.expected_name {
            eprintln!("ERROR: Expected name = {}, value = {}", self.expected_name, self.expected_value);
            exit(1)
        }
    }
}


/*

    physical action response;
    state prompt_time:time(0);
    input prompt:bool;
    output another:bool;
 */

struct PrintReactionState {
    _impl: Print,
    input: InputPort<Hello>,
}

reaction_ids!(enum PrintReactions { Print });

impl ReactorDispatcher for PrintReactionState {
    type ReactionId = PrintReactions;
    type Wrapped = Print;
    type Params = (i32, &'static str);

    fn assemble(p: Self::Params) -> Self {
        PrintReactionState {
            _impl: Print {
                expected_value: p.0,
                expected_name: p.1,
            },
            input: Default::default(),
        }
    }

    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId) {
        match rid {
            PrintReactions::Print => {
                self._impl.react_print(ctx, &self.input)
            }
        }
    }
}

struct PrintAssembler {
    _rstate: Arc<Mutex</*{{*/PrintReactionState/*}}*/>>,
    /*{{*/react_print/*}}*/: Arc<ReactionInvoker>,
}

impl ReactorAssembler for /*{{*/PrintAssembler/*}}*/ {
    type RState = /*{{*/PrintReactionState/*}}*/;


    fn start(&mut self, _: SchedulerLink, _: &mut LogicalCtx) {
        // nothing to do
    }

    fn assemble(reactor_id: &mut u32, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = *reactor_id;
        *reactor_id += 1;
        let mut reaction_id = 0;

        let /*{{*/react_print /*}}*/ = new_reaction!(this_reactor, reaction_id, _rstate, /*{{*/Print/*}}*/);

        { // declare local dependencies
            let mut statemut = _rstate.lock().unwrap();

            statemut./*{{*/input/*}}*/.set_downstream(vec![/*{{*/react_print/*}}*/.clone()].into());
        }

        Self {
            _rstate,
            /*{{*/react_print/*}}*/,
        }
    }
}
