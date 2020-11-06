use std::pin::Pin;
use std::cell::Cell;
use std::time::{Duration, Instant};
use std::io::stdin;
use futures::io::Error;
use petgraph::stable_graph::edge_index;
use std::rc::Rc;

use crate::runtime::Ctx;

fn main() {}
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
    let mut g = GetUserInput { prompt_time: LfTime::zero() };
    let mut chan1 = bool::default();
    let mut chan2 = bool::default();
}


/*
    physical action response;
    input prompt:bool;
    output another:bool;
 */



///  output out : i32;
///
///  reaction(t) -> out {=
///         printf("Hello World.\n");
///  =}
/*


// Send a periodic image out

 */

struct RandomSource {}

impl RandomSource { // preamble block

    fn random_delay() -> LfTime {
        LfTime::from_secs(0)
    }
}

impl RandomSource { // reaction block

    /// reaction(startup) -> prompt {=
    ///      // Random number functions are part of stdlib.h, which is included by reactor.h.
    ///      // Set a seed for random number generation based on the current time.
    ///      srand(time(0));
    ///      schedule(prompt, random_time());
    ///  =}
    fn react_startup(ctx: &mut impl Ctx, prompt: &Action) {
        // seed random gen
        ctx.schedule(prompt, Some(random_delay()));
    }

    /// reaction(prompt) -> out, prompt {=
    ///     printf("Hit Return!");
    ///     fflush(stdout);
    ///     SET(out, true);
    /// =}
    fn react_emit(ctx: &mut impl Ctx, out: OutPort<bool>) {
        println!("Hit Return!");
        ctx.set_port(out, true);
    }

    /// reaction(another) -> prompt {=
    ///     schedule(prompt, random_time());
    /// =}
    fn react_schedule(ctx: &mut impl Ctx, out: OutPort<bool>) {
        println!("Hit Return!");
        ctx.set_port(out, true);
    }
}


struct GetUserInput {
    prompt_time: LfTime
}

impl GetUserInput {
    fn read_input_loop(ctx: &mut impl Ctx, response: &Action) {
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
    fn react_startup(&mut self, ctx: &mut impl Ctx, response: &Action) {}

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
    fn react_handle_line<'a>(&mut self, ctx: &mut impl Ctx, another: OutPort<bool>) {
        if self.prompt_time == LfTime::from_secs(0) {
            println!("You cheated!");
        } else {
            let time = (ctx.get_logical_time() - self.prompt_time);
            println!("Response time: {} ms", time.as_millis());
            self.prompt_time = LfTime::default();
        }
        ctx.set_port(another, true)
    }

    // reaction(prompt) {=
    // self->prompt_time = get_physical_time();
    // =}
    fn react_prompt(&mut self, ctx: &mut impl Ctx) {
        self.prompt_time = ctx.get_physical_time()
    }
}
