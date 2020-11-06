use std::pin::Pin;
use std::cell::Cell;
use std::time::{Duration, Instant};
use std::io::stdin;
use futures::io::Error;
use petgraph::stable_graph::edge_index;
use std::rc::Rc;

use crate::runtime::*;
use crate::runtime::ports::{OutputPort, InputPort, Port};
use crate::reactors::Assembler;
use std::cell::{RefCell, RefMut};

fn main() {}
// this is a manual translation of https://github.com/icyphy/lingua-franca/blob/master/example/ReflexGame/ReflexGameMinimal.lf

// translation strategy:
// - reactor: struct
//   - state variables: struct fields
//   - reaction(t1, .., tn) u_1, .., u_n -> d_1, .. d_n
//      fn (&mut self, &mut Ctx<'_>, ...)

// one special startup reaction, for which physical action parameters
// are passed as owned parameters


/*

main reactor ReflexGame {
    p = new RandomSource();
    g = new GetUserInput();
    p.out -> g.prompt;
    g.another -> p.another;
}
 */
fn assemble() {
    let mut rid = 0;

    // --- p = new RandomSource();

    let mut pcell = Rc::new(RefCell::new(RandomSourceReactionState::assemble(())));

    let p_schedule = Rc::new(ReactionInvoker::new(rid, pcell.clone(), RandomSourceReactions::Schedule));
    rid += 1;
    let p_emit = Rc::new(ReactionInvoker::new(rid, pcell.clone(), RandomSourceReactions::Emit));
    rid += 1;

    {
        let mut pmut = pcell.borrow_mut();
        pmut.another.set_downstream(vec![p_schedule.clone()])
    }

    // --- end

    // --- g = new GetUserInput();
    let mut gcell = GetUserInputAssembler::assemble(&mut rid, ());

    {
        let mut p = pcell.borrow_mut();
        let mut g = gcell._rstate.borrow_mut();

        // --- p.out -> g.prompt;
        ports::bind(&mut p.out, &mut g.prompt);

        // --- g.another -> p.another;
        ports::bind(&mut g.another, &mut p.another);
    }
}


struct RandomSource;

impl RandomSource { // reaction block

    fn random_delay() -> Duration {
        Duration::from_secs(1)
    }

    /// reaction(startup) -> prompt {=
    ///      // Random number functions are part of stdlib.h, which is included by reactor.h.
    ///      // Set a seed for random number generation based on the current time.
    ///      srand(time(0));
    ///      schedule(prompt, random_time());
    ///  =}
    fn react_startup(&mut self, ctx: &mut Ctx<'_>, prompt: &Action) {
        // seed random gen
        ctx.schedule(prompt, Some(RandomSource::random_delay()));
    }

    /// reaction(prompt) -> out, prompt {=
    ///     printf("Hit Return!");
    ///     fflush(stdout);
    ///     SET(out, true);
    /// =}
    fn react_emit(&mut self, ctx: &mut Ctx<'_>, out: &mut OutputPort<bool>) {
        println!("Hit Return!");
        ctx.set(out, true);
    }

    /// reaction(another) -> prompt {=
    ///     schedule(prompt, random_time());
    /// =}
    fn react_schedule(&mut self, ctx: &mut Ctx<'_>, out: &mut OutputPort<bool>) {
        println!("Hit Return!");
        ctx.set(out, true);
    }
}

/*
    input another:bool;
    output out:bool;
    logical action prompt(2 secs);
 */
struct RandomSourceReactionState {
    _impl: RandomSource,
    prompt: Action,
    another: InputPort<bool>,
    out: OutputPort<bool>,
}

#[derive(Copy, Clone)]
enum RandomSourceReactions {
    Schedule,
    Emit,
}

impl ReactionState for RandomSourceReactionState {
    type ReactionId = RandomSourceReactions;
    type Wrapped = RandomSource;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        RandomSourceReactionState {
            _impl: RandomSource,
            prompt: Action::new(None, true),
            another: InputPort::<bool>::new(),
            out: OutputPort::<bool>::new(),
        }
    }

    fn start(&mut self, ctx: &mut Ctx) {
        self._impl.react_startup(ctx, &self.prompt)
    }

    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId) {
        match rid {
            RandomSourceReactions::Schedule => {
                self._impl.react_schedule(ctx, &mut self.out)
            }
            RandomSourceReactions::Emit => {
                self._impl.react_emit(ctx, &mut self.out)
            }
        }
    }
}


struct GetUserInput {
    prompt_time: Option<Instant>
}


// user impl
impl GetUserInput {
    fn read_input_loop(ctx: &mut Ctx<'_>, response: &Action) {
        let mut buf = String::new();
        loop {
            match stdin().read_line(&mut buf) {
                Ok(_) => {
                    ctx.schedule(response, None)
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
    fn react_startup(&mut self, ctx: &mut Ctx<'_>, response: &Action) {
        unimplemented!()
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
    fn react_handle_line(&mut self, ctx: &mut Ctx<'_>, another: &mut OutputPort<bool>) {
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

    // reaction(prompt) {=
    // self->prompt_time = get_physical_time();
    // =}
    fn react_prompt(&mut self, ctx: &mut Ctx<'_>, prompt: &InputPort<bool>) {
        self.prompt_time = Some(ctx.get_physical_time())
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
    response: Action,
    prompt: InputPort<bool>,
    another: OutputPort<bool>,
}

#[derive(Copy, Clone)]
enum GetUserInputReactions {
    HandleLine,
    Prompt,
}

impl ReactionState for GetUserInputReactionState {
    type ReactionId = GetUserInputReactions;
    type Wrapped = GetUserInput;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        GetUserInputReactionState {
            _impl: GetUserInput { prompt_time: None },
            response: Action::new(None, false),
            prompt: InputPort::<bool>::new(),
            another: OutputPort::<bool>::new(),
        }
    }

    fn start(&mut self, ctx: &mut Ctx) {
        self._impl.react_startup(ctx, &self.response)
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

macro_rules! new_reaction {
    ($rid:ident, $_rstate:ident, $name:ident) => {{
        let r = Rc::new(ReactionInvoker::new(*$rid, $_rstate.clone(), <Self::RState as ReactionState>::ReactionId::$name));
        *$rid += 1;
        r
    }};
}

struct GetUserInputAssembler {
    _rstate: Rc<RefCell</*{{*/GetUserInputReactionState/*}}*/>>,
    /*{{*/react_handle_line/*}}*/: Rc<ReactionInvoker>,
    /*{{*/react_prompt/*}}*/: Rc<ReactionInvoker>,
}

impl AssemblyWrapper for /*{{*/GetUserInputAssembler/*}}*/ {
    type RState = /*{{*/GetUserInputReactionState/*}}*/;

    fn assemble(rid: &mut i32, args: <Self::RState as ReactionState>::Params) -> Self {
        let mut _rstate = Rc::new(RefCell::new(Self::RState::assemble(args)));

        let /*{{*/react_handle_line /*}}*/ = new_reaction!(rid, _rstate, /*{{*/HandleLine/*}}*/);
        let /*{{*/react_prompt /*}}*/ = new_reaction!(rid, _rstate, /*{{*/Prompt/*}}*/);

        { // declare local dependencies
            let mut statemut = _rstate.borrow_mut();

            statemut./*{{*/prompt/*}}*/.set_downstream(vec![/*{{*/react_prompt/*}}*/.clone()]);
            // todo gmut.response.set_downstream(vec![handle_line.clone()]);
        }

        Self {
            _rstate,
            /*{{*/react_handle_line/*}}*/,
            /*{{*/react_prompt/*}}*/,
        }
    }
}
