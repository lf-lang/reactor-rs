#![allow(unused)]
#[macro_use]
extern crate reactor_rt;


use std::io::stdin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::Rng;

use reactor_rt::reaction_ids;
use reactor_rt::reaction_ids_helper;
use reactor_rt::*;
use reactor_rt::Offset::{After, Asap};

// this is a manual translation of https://github.com/icyphy/lingua-franca/blob/f5868bec199e02f784393f32b594be5df935e2ee/example/C/ReflexGame/ReflexGameMinimal.lf#

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
// fn (&mut self, ctx: &mut LogicalCtx<'_>, params*)
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

/*

main reactor ReflexGame {
    p = new RandomSource();
    g = new GetUserInput();
    p.out -> g.prompt;
    g.another -> p.another;
}
 */
fn main() {
    let mut reactor_id = ReactorId::first();

    // --- p = new RandomSource();
    let mut pcell = RandomSourceAssembler::assemble(&mut reactor_id, ());

    // --- g = new GetUserInput();
    let mut gcell = GetUserInputAssembler::assemble(&mut reactor_id, ());

    {
        let mut p = pcell._rstate.lock().unwrap();
        let mut g = gcell._rstate.lock().unwrap();

        // --- p.out -> g.prompt;
        bind_ports(&mut p.out, &mut g.prompt);

        // --- g.another -> p.another;
        bind_ports(&mut g.another, &mut p.another);
    }

    let mut scheduler = SyncScheduler::new();

    scheduler.startup(|mut starter| {
        starter.start(&mut gcell);
        starter.start(&mut pcell);
    });
    scheduler.launch_async(Duration::from_secs(10)).join().unwrap();
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
    fn react_startup(link: SchedulerLink, ctx: &mut LogicalCtx, prompt: &LogicalAction) {
        // seed random gen
        ctx.schedule(prompt, After(RandomSource::random_delay()));
    }

    /// reaction(prompt) -> out, prompt {=
    ///     printf("Hit Return!");
    ///     fflush(stdout);
    ///     SET(out, true);
    /// =}
    fn react_emit(&mut self, ctx: &mut LogicalCtx, out: &mut OutputPort<bool>) {
        println!("Hit Return!");
        ctx.set(out, true);
    }

    /// reaction(another) -> prompt {=
    ///     schedule(prompt, random_time());
    /// =}
    fn react_schedule(&mut self, ctx: &mut LogicalCtx, prompt: &LogicalAction) {
        ctx.schedule(prompt, After(RandomSource::random_delay()));
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
            another: Default::default(),
            out: Default::default(),
        }
    }

    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId) {
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
    _rstate: Arc<Mutex</*{{*/RandomSourceDispatcher/*}}*/>>,
    /*{{*/react_schedule/*}}*/: Arc<ReactionInvoker>,
    /*{{*/react_emit/*}}*/: Arc<ReactionInvoker>,
}

impl ReactorAssembler for /*{{*/RandomSourceAssembler/*}}*/ {
    type RState = /*{{*/RandomSourceDispatcher/*}}*/;

    fn start(&mut self, link: SchedulerLink, ctx: &mut LogicalCtx) {
        RandomSource::react_startup(link, ctx, &self._rstate.lock().unwrap().prompt);
    }


    fn assemble(reactor_id: &mut ReactorId, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = reactor_id.get_and_increment();
        let mut reaction_id = 0;

        let /*{{*/react_schedule /*}}*/ = new_reaction!(this_reactor, reaction_id, _rstate, /*{{*/Schedule/*}}*/);
        let /*{{*/react_emit /*}}*/ = new_reaction!(this_reactor, reaction_id, _rstate, /*{{*/Emit/*}}*/);

        { // declare local dependencies
            let mut statemut = _rstate.lock().unwrap();

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
    fn read_input_loop(mut ctx: SchedulerLink, response: PhysicalAction) {
        let mut buf = String::new();
        loop {
            match stdin().read_line(&mut buf) {
                Ok(_) => {
                    ctx.schedule_physical(&response, Asap)
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
    fn react_startup(link: SchedulerLink, ctx: &mut LogicalCtx, response: PhysicalAction) {
        std::thread::spawn(move || GetUserInput::read_input_loop(link, response));
    }

    // reaction(prompt) {=
    // self->prompt_time = get_physical_time();
    // =}
    fn react_prompt(&mut self, ctx: &mut LogicalCtx, prompt: &InputPort<bool>) {
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
    fn react_handle_line(&mut self, ctx: &mut LogicalCtx, another: &mut OutputPort<bool>) {
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
            prompt: Default::default(),
            another: Default::default(),
        }
    }

    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId) {
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
    _rstate: Arc<Mutex</*{{*/GetUserInputReactionState/*}}*/>>,
    /*{{*/react_handle_line/*}}*/: Arc<ReactionInvoker>,
    /*{{*/react_prompt/*}}*/: Arc<ReactionInvoker>,
}

impl ReactorAssembler for /*{{*/GetUserInputAssembler/*}}*/ {
    type RState = /*{{*/GetUserInputReactionState/*}}*/;


    fn start(&mut self, link: SchedulerLink, ctx: &mut LogicalCtx) {
        let mut response = /* response */PhysicalAction::new(None, "response");
        /*{{*/response/*}}*/.set_downstream(vec![/*{{*/self.react_handle_line/*}}*/.clone()].into());

        GetUserInput::react_startup(link, ctx, response);
    }

    fn assemble(reactor_id: &mut ReactorId, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = reactor_id.get_and_increment();
        let mut reaction_id = 0;

        let /*{{*/react_handle_line /*}}*/ = new_reaction!(this_reactor, reaction_id, _rstate, /*{{*/HandleLine/*}}*/);
        let /*{{*/react_prompt /*}}*/ = new_reaction!(this_reactor, reaction_id, _rstate, /*{{*/Prompt/*}}*/);

        { // declare local dependencies
            let mut statemut = _rstate.lock().unwrap();

            statemut./*{{*/prompt/*}}*/.set_downstream(vec![/*{{*/react_prompt/*}}*/.clone()].into());
        }

        Self {
            _rstate,
            /*{{*/react_handle_line/*}}*/,
            /*{{*/react_prompt/*}}*/,
        }
    }
}
