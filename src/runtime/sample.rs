use std::pin::Pin;
use std::cell::Cell;
use std::time::{Duration, Instant};
use std::io::stdin;
use futures::io::Error;
use petgraph::stable_graph::edge_index;
use std::rc::Rc;

use crate::runtime::*;
use crate::runtime::ports::{OutputPort, InputPort};

fn main() {}
// this is a manual translation of https://github.com/icyphy/lingua-franca/blob/master/example/ReflexGame/ReflexGameMinimal.lf

// translation strategy:
// - reactor: struct
//   - state variables: struct fields
//   - reaction(t1, .., tn) u_1, .., u_n -> d_1, .. d_n
//      fn (&mut self, &mut Ctx, ...)

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
    let mut p = RandomSource {};
    let mut qq = GetUserInputWrapper {
        _impl: GetUserInput { prompt_time: Instant::now() },
        response: Action::new(None, false),
        prompt: InputPort::<bool>::new(),
        another: OutputPort::<bool>::new()
    };
    let mut chan1 = bool::default();
    let mut chan2 = bool::default();
}


struct RandomSource {}

impl RandomSource { // reaction block

    fn random_delay() -> Duration {
        Instant::from_secs(0)
    }

    /// reaction(startup) -> prompt {=
    ///      // Random number functions are part of stdlib.h, which is included by reactor.h.
    ///      // Set a seed for random number generation based on the current time.
    ///      srand(time(0));
    ///      schedule(prompt, random_time());
    ///  =}
    fn react_startup(ctx: &mut Ctx<'_>, prompt: &Action) {
        // seed random gen
        ctx.schedule(prompt, Some(RandomSource::random_delay()));
    }

    /// reaction(prompt) -> out, prompt {=
    ///     printf("Hit Return!");
    ///     fflush(stdout);
    ///     SET(out, true);
    /// =}
    fn react_emit(ctx: &mut Ctx<'_>, out: OutputPort<bool>) {
        println!("Hit Return!");
        ctx.set_port(out, true);
    }

    /// reaction(another) -> prompt {=
    ///     schedule(prompt, random_time());
    /// =}
    fn react_schedule(ctx: &mut Ctx<'_>, out: OutputPort<bool>) {
        println!("Hit Return!");
        ctx.set_port(out, true);
    }
}


struct GetUserInput {
    prompt_time: Instant
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
    fn react_handle_line(&mut self, ctx: &mut Ctx<'_>, another: &OutputPort<bool>) {
        if self.prompt_time == Instant::from_secs(0) {
            println!("You cheated!");
        } else {
            let time = ctx.get_logical_time() - self.prompt_time;
            println!("Response time: {} ms", time.as_millis());
            self.prompt_time = Instant::zero();
        }
        ctx.set_port(another, true)
    }

    // reaction(prompt) {=
    // self->prompt_time = get_physical_time();
    // =}
    fn react_prompt(&mut self, ctx: &mut Ctx<'_>, prompt: &InputPort<bool>) {
        self.prompt_time = ctx.get_physical_time()
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

    fn start(&mut self, ctx: &mut Ctx) {
        self._impl.react_startup(ctx, &self.response)
    }

    fn react(&mut self, ctx: &mut Ctx, rid: Self::ReactionId) {
        match rid {
            GetUserInputReactions::HandleLine => {
                self._impl.react_handle_line(ctx, &self.another)
            }
            GetUserInputReactions::Prompt => {
                self._impl.react_prompt(ctx, &self.prompt)
            }
        }
    }
}
