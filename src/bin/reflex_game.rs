#[macro_use]
extern crate rust_reactors;


use std::cell::{RefCell, RefMut};
use std::cell::Cell;
use std::io::stdin;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use futures::io::Error;
use petgraph::stable_graph::edge_index;
use rand::Rng;

use rust_reactors::reaction_ids;
use rust_reactors::reaction_ids_helper;
use rust_reactors::reactors::{Enumerated, Named};
use rust_reactors::runtime::*;

// this is a manual translation of https://github.com/icyphy/lingua-franca/blob/master/example/ReflexGame/ReflexGameMinimal.lf

// translation strategy:
// - reactor: struct
//   - state variables: struct fields

// Reactor r -> 3 structs + 1 enum
// - user struct (r)
// - dispatch struct (r_dispatch)   : ReactionState
// - assembly struct (r_assembly)   : AssemblyWrapper
// - reaction id enum (r_reactions) : Copy

// # User struct
//
// Contains user-written code. The form of declarations is
// high-level. All the framework goo is hidden into the other
// structs.
//
// The fields of this struct are the state variables of the
// reactor (if there are none, the type is zero-size and can
// be optimized out).
//
// ## Reactions
//
// reaction(t1, .., tn) u_1, .., u_n -> d_1, .. d_n
//
// is translated *in the user struct* to
//
// fn (&mut self, ctx: &mut Ctx<'_>, params*)
//
// where params is:
//   - for each $p\in\{t_i\}\union\{u_i\}$, where $p$ is an input port of type $T$
//       p: &InputPort<T>
//   - for each $o\in\{d_i\}$, where $o$ is an output port of type $T$
//       o: &mut OutputPort<T>
//   - for each $a\in\{d_i\}$, where $a$ is a logical action
//       a: &Action

// ## Startup reaction

// The startup reaction translated the same way as reactions,
// except the additional params contain registered physical actions too.

// # Glue code
//
// Each reaction gets an enum constant in a new enum.
// See ReactorDispatcher and ReactorAssembler for the rest.


// # Information needed by the code generator
//
// - Transitive closure of dependency graph, where ports are
// considered opaque (they can be bound dynamically).
// - All structural checks need to be done before codegen


macro_rules! new_reaction {
    ($rid:ident, $_rstate:ident, $name:ident) => {{
        let r = Arc::new(ReactionInvoker::new(*$rid, $_rstate.clone(), <Self::RState as ReactorDispatcher>::ReactionId::$name));
        *$rid += 1;
        r
    }};
}

/*

main reactor ReflexGame {
    p = new RandomSource();
    g = new GetUserInput();
    p.out -> g.prompt;
    g.another -> p.another;
}
 */
fn main() {
    let mut rid = 0;

    // --- p = new RandomSource();
    let mut pcell = RandomSourceAssembler::assemble(&mut rid, ());

    // --- g = new GetUserInput();
    let mut gcell = GetUserInputAssembler::assemble(&mut rid, ());

    {
        let mut p = pcell._rstate.borrow_mut();
        let mut g = gcell._rstate.borrow_mut();

        // --- p.out -> g.prompt;
        bind_ports(&mut p.out, &mut g.prompt);

        // --- g.another -> p.another;
        bind_ports(&mut g.another, &mut p.another);
    }

    let mut scheduler = SyncScheduler::new();

    scheduler.start(&mut gcell);
    scheduler.start(&mut pcell);
    scheduler.launch_async().join();
}


struct RandomSource;

impl RandomSource {
    fn random_delay() -> Duration {
        let mut rng = rand::thread_rng();
        use rand::prelude::*;
        Duration::from_millis(rng.gen_range(200, 2500))
    }

    /// reaction(startup) -> prompt {=
    ///      // Random number functions are part of stdlib.h, which is included by reactor.h.
    ///      // Set a seed for random number generation based on the current time.
    ///      srand(time(0));
    ///      schedule(prompt, random_time());
    ///  =}
    fn react_startup(mut ctx: PhysicalCtx, prompt: &LogicalAction) {
        // seed random gen
        ctx.schedule_delayed(prompt, RandomSource::random_delay());
    }

    /// reaction(prompt) -> out, prompt {=
    ///     printf("Hit Return!");
    ///     fflush(stdout);
    ///     SET(out, true);
    /// =}
    fn react_emit(&mut self, ctx: &mut Ctx, out: &mut OutputPort<bool>) {
        println!("Hit Return!");
        ctx.set(out, true);
    }

    /// reaction(another) -> prompt {=
    ///     schedule(prompt, random_time());
    /// =}
    fn react_schedule(&mut self, ctx: &mut Ctx, prompt: &LogicalAction) {
        ctx.schedule_delayed(prompt, RandomSource::random_delay());
    }
}

/*
    input another:bool;
    output out:bool;
    logical action prompt(2 secs);
 */
struct RandomSourceDispatcher {
    _impl: RandomSource,
    prompt: LogicalAction,
    another: InputPort<bool>,
    out: OutputPort<bool>,
}

reaction_ids!(enum RandomSourceReactions { Schedule, Emit, });

impl ReactorDispatcher for RandomSourceDispatcher {
    type ReactionId = RandomSourceReactions;
    type Wrapped = RandomSource;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        RandomSourceDispatcher {
            _impl: RandomSource,
            prompt: LogicalAction::new(None, "prompt"),
            another: InputPort::<bool>::new(),
            out: OutputPort::<bool>::new(),
        }
    }

    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId) {
        match rid {
            RandomSourceReactions::Schedule => {
                self._impl.react_schedule(ctx, &self.prompt)
            }
            RandomSourceReactions::Emit => {
                self._impl.react_emit(ctx, &mut self.out)
            }
        }
    }
}


struct RandomSourceAssembler {
    _rstate: Rc<RefCell</*{{*/RandomSourceDispatcher/*}}*/>>,
    /*{{*/react_schedule/*}}*/: Arc<ReactionInvoker>,
    /*{{*/react_emit/*}}*/: Arc<ReactionInvoker>,
}

impl ReactorAssembler for /*{{*/RandomSourceAssembler/*}}*/ {
    type RState = /*{{*/RandomSourceDispatcher/*}}*/;

    fn start(&mut self, ctx: PhysicalCtx) {
        RandomSource::react_startup(ctx, &self._rstate.borrow().prompt);
    }


    fn assemble(rid: &mut i32, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Rc::new(RefCell::new(Self::RState::assemble(args)));

        let /*{{*/react_schedule /*}}*/ = new_reaction!(rid, _rstate, /*{{*/Schedule/*}}*/);
        let /*{{*/react_emit /*}}*/ = new_reaction!(rid, _rstate, /*{{*/Emit/*}}*/);

        { // declare local dependencies
            let mut statemut = _rstate.borrow_mut();

            statemut./*{{*/another/*}}*/.set_downstream(vec![/*{{*/react_schedule/*}}*/.clone()].into());
            statemut./*{{*/prompt/*}}*/.set_downstream(vec![/*{{*/react_emit/*}}*/.clone()].into());
        }

        Self {
            _rstate,
            /*{{*/react_schedule/*}}*/,
            /*{{*/react_emit/*}}*/,
        }
    }
}

struct GetUserInput {
    prompt_time: Option<Instant>
}


// user impl
impl GetUserInput {
    fn read_input_loop(ctx: &mut PhysicalCtx, response: &PhysicalAction) {
        let mut buf = String::new();
        loop {
            match stdin().read_line(&mut buf) {
                Ok(_) => {
                    ctx.schedule(response)
                }
                Err(_) => {}
            }
        }
    }

    /// reaction(startup) -> response {=
    ///     pthread_t thread_id;
    ///     pthread_create(&thread_id, NULL, &read_input, response);
    /// =}
    ///
    fn react_startup(ctx: PhysicalCtx, response: PhysicalAction) {
        use std::thread;
        thread::spawn(move || {
            let response = response;
            let mut ctx = ctx;
            GetUserInput::read_input_loop(&mut ctx, &response)
        });
    }

    // reaction(prompt) {=
    // self->prompt_time = get_physical_time();
    // =}
    fn react_prompt(&mut self, ctx: &mut Ctx, prompt: &InputPort<bool>) {
        let instant = ctx.get_physical_time();
        self.prompt_time = Some(instant)
    }

    /// reaction(response) -> another {=
    ///        if (self->prompt_time == 0LL) {
    ///            printf("YOU CHEATED!\n");
    ///        } else {
    ///            int time_in_ms = (get_logical_time() - self->prompt_time) / MSEC(1);
    ///            printf("Response time ms: %d\n", time_in_ms);
    ///            self->prompt_time = 0LL;
    ///        }
    ///        SET(another, true);
    /// =}
    fn react_handle_line(&mut self, ctx: &mut Ctx, another: &mut OutputPort<bool>) {
        match self.prompt_time.take() {
            None => {
                println!("You cheated!");
            }
            Some(t) => {
                let time = ctx.get_logical_time().to_instant() - t;
                println!("Response time: {} ms", time.as_millis());
            }
        }
        ctx.set(another, true)
    }
}


/*

    physical action response;
    state prompt_time:time(0);
    input prompt:bool;
    output another:bool;
 */

struct GetUserInputReactionState {
    _impl: GetUserInput,
    prompt: InputPort<bool>,
    another: OutputPort<bool>,
}

reaction_ids!(enum GetUserInputReactions { HandleLine, Prompt, });

impl ReactorDispatcher for GetUserInputReactionState {
    type ReactionId = GetUserInputReactions;
    type Wrapped = GetUserInput;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        GetUserInputReactionState {
            _impl: GetUserInput { prompt_time: None },
            prompt: InputPort::<bool>::new(),
            another: OutputPort::<bool>::new(),
        }
    }

    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId) {
        match rid {
            GetUserInputReactions::HandleLine => {
                self._impl.react_handle_line(ctx, &mut self.another)
            }
            GetUserInputReactions::Prompt => {
                self._impl.react_prompt(ctx, &self.prompt)
            }
        }
    }
}

struct GetUserInputAssembler {
    _rstate: Rc<RefCell</*{{*/GetUserInputReactionState/*}}*/>>,
    /*{{*/react_handle_line/*}}*/: Arc<ReactionInvoker>,
    /*{{*/react_prompt/*}}*/: Arc<ReactionInvoker>,
}

impl ReactorAssembler for /*{{*/GetUserInputAssembler/*}}*/ {
    type RState = /*{{*/GetUserInputReactionState/*}}*/;


    fn start(&mut self, ctx: PhysicalCtx) {
        let mut response = (/* response */PhysicalAction::new(None, "response"));
        /*{{*/response/*}}*/.set_downstream(vec![/*{{*/self.react_handle_line/*}}*/.clone()].into());

        GetUserInput::react_startup(ctx, response);
    }

    fn assemble(rid: &mut i32, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Rc::new(RefCell::new(Self::RState::assemble(args)));

        let /*{{*/react_handle_line /*}}*/ = new_reaction!(rid, _rstate, /*{{*/HandleLine/*}}*/);
        let /*{{*/react_prompt /*}}*/ = new_reaction!(rid, _rstate, /*{{*/Prompt/*}}*/);

        { // declare local dependencies
            let mut statemut = _rstate.borrow_mut();

            statemut./*{{*/prompt/*}}*/.set_downstream(vec![/*{{*/react_prompt/*}}*/.clone()].into());
        }

        Self {
            _rstate,
            /*{{*/react_handle_line/*}}*/,
            /*{{*/react_prompt/*}}*/,
        }
    }
}
