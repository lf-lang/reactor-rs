use std::pin::Pin;
use std::cell::Cell;
use std::time::{Duration, Instant};
use std::io::stdin;
use futures::io::Error;
use petgraph::stable_graph::edge_index;
use std::rc::Rc;

use crate::runtime::*;
use crate::runtime::ports::{OutputPort, InputPort, bind};
use crate::reactors::Assembler;

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
    let mut q = GetUserInputWrapper::assemble(());
    let mut p = RandomSourceWrapper::assemble(());

    bind(&mut p.out, &mut q.prompt);
    bind(&mut q.another, &mut p.another);
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
struct RandomSourceWrapper {
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

impl ReactorWrapper for RandomSourceWrapper {
    type ReactionId = RandomSourceReactions;
    type Wrapped = RandomSource;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        RandomSourceWrapper {
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

struct GetUserInputWrapper {
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

impl ReactorWrapper for GetUserInputWrapper {
    type ReactionId = GetUserInputReactions;
    type Wrapped = GetUserInput;
    type Params = ();

    fn assemble(_: Self::Params) -> Self {
        let mut r = GetUserInputWrapper {
            _impl: GetUserInput { prompt_time: None },
            response: Action::new(None, false),
            prompt: InputPort::<bool>::new(),
            another: OutputPort::<bool>::new(),
        };

        r
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
