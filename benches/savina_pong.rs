/*
 * Copyright (c) 2021, TU Dresden.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
 * THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
 * THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

#![allow(unused, non_snake_case, non_camel_case_types)]
#[macro_use]
extern crate reactor_rt;
extern crate env_logger;


use std::sync::{Arc, Mutex};

use ::reactor_rt::{LogicalInstant, PhysicalInstant, Duration};
use ::reactor_rt::Offset::{After, Asap};


use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main, black_box};

use reactor_rt::reaction_ids;
use reactor_rt::reaction_ids_helper;
use reactor_rt::*;

/*
The ping/pong game from Savina benchmarks. This can be compared
to the C implementation (see results.md).

See original at https://github.com/icyphy/lingua-franca/blob/f5868bec199e02f784393f32b594be5df935e2ee/benchmark/C/Savina/PingPong.lf


 */

criterion_group!(benches, reactor_main);
criterion_main!(benches);

fn reactor_main(c: &mut Criterion) {
    env_logger::init();
    let mut group = c.benchmark_group("savina_pong");
    for num_pongs in [1000, 10_000, 50_000, 1_000_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_pongs),
            num_pongs,
            |b, &size| {
                b.iter(|| {
                    let timeout = Some(Duration::from_secs(5));
                    launch(1, size, timeout);
                });
            }
        );
    }
    group.finish();
}

fn launch(numIterations: u32, count: u32, timeout: Option<Duration>)  {


    // todo CLI parsing
    let options = SchedulerOptions {
        timeout: None,
        keep_alive: false
    };
    let main_args = savina_pong::SavinaPongParams {
        count,
        expected: count
    };

    SyncScheduler::run_main::<SavinaPongDispatcher>(options, main_args);
}


pub use savina_pong::SavinaPongParams;
pub use savina_pong::SavinaPongDispatcher;
pub use ping::PingParams;
pub use ping::PingDispatcher;
pub use pong::PongParams;
pub use pong::PongDispatcher;

mod savina_pong {

    // todo link to source
    pub struct SavinaPong;

    /// Parameters for the construction of a [SavinaPong]
    #[derive(Clone)]
    pub struct SavinaPongParams {
        pub count:u32,
        pub expected:u32
    }


//------------------------//


    pub struct SavinaPongDispatcher {
        _id: ::reactor_rt::ReactorId,
        _impl: SavinaPong,
        _params: SavinaPongParams,
        _startup_reactions: ::reactor_rt::ReactionSet,
        _shutdown_reactions: ::reactor_rt::ReactionSet,

    }


    reaction_ids!(pub enum SavinaPongReactions {});

    impl SavinaPongDispatcher {
        #[inline]
        fn user_assemble(_id: ::reactor_rt::ReactorId, args: SavinaPongParams) -> Self {
            let SavinaPongParams {  count, expected } = args.clone();
            Self {
                _id,
                _params: args,
                _startup_reactions: Default::default(),
                _shutdown_reactions: Default::default(),
                _impl: SavinaPong {

                },

            }
        }
    }

    use ::reactor_rt::*; // after this point there's no user-written code
    use std::sync::{Arc, Mutex};

    impl ::reactor_rt::ReactorDispatcher for SavinaPongDispatcher {
        type ReactionId = SavinaPongReactions;
        type Wrapped = SavinaPong;
        type Params = SavinaPongParams;

        fn assemble(args: Self::Params, assembler: &mut AssemblyCtx) -> Self {
            // children reactors
            // --- ping = new Ping();
            let mut ping: super::PingDispatcher = assembler.assemble_sub(super::PingParams { count: args.count, });
            // --- pong = new Pong();
            let mut pong: super::PongDispatcher = assembler.assemble_sub(super::PongParams { expected: args.expected, });

            // assemble self
            let this_reactor = assembler.get_next_id();
            let mut _self = Self::user_assemble(this_reactor, args);



            {
                _self._startup_reactions = vec![];
                _self._shutdown_reactions = vec![];
            }
            {
                // Declare connections
                // --- ping.send -> pong.receive;
                bind_ports(&mut ping.send, &mut pong.receive);
                // --- pong.send -> ping.receive;
                bind_ports(&mut pong.send, &mut ping.receive);
            }
            assembler.register_reactor(ping);
            assembler.register_reactor(pong);

            _self
        }

        #[inline]
        fn react(&mut self, ctx: &mut ::reactor_rt::LogicalCtx, rid: Self::ReactionId) {
            match rid {

            }
        }
    }


    impl ::reactor_rt::ErasedReactorDispatcher for SavinaPongDispatcher {

        fn id(&self) -> ReactorId {
            self._id
        }

        fn react_erased(&mut self, ctx: &mut ::reactor_rt::LogicalCtx, rid: u32) {
            let rid = <SavinaPongReactions as int_enum::IntEnum>::from_int(rid).unwrap();
            self.react(ctx, rid)
        }

        fn cleanup_tag(&mut self, ctx: ::reactor_rt::LogicalCtx) {
            // todo
        }

        fn enqueue_startup(&self, ctx: &mut StartupCtx) {


            ctx.enqueue(&self._startup_reactions);
        }

        fn enqueue_shutdown(&self, ctx: &mut StartupCtx) {
            ctx.enqueue(&self._shutdown_reactions);
        }

    }
}

mod ping {

    // todo link to source
    pub struct Ping {
        pingsLeft: u32,
    }

    #[warn(unused)]
    impl Ping {

        // --- reaction(startup, serve) -> send {= ... =}
        fn react_0(&mut self,
                   #[allow(unused)] ctx: &mut ::reactor_rt::LogicalCtx,
                   #[allow(unused)] params: &PingParams,
                   #[allow(unused)] serve: & ::reactor_rt::LogicalAction::<()>,
                   send: &mut ::reactor_rt::OutputPort<u32>) {
            ctx.set(send, self.pingsLeft);
            self.pingsLeft -= 1;
        }

        // --- reaction (receive) -> serve {= ... =}
        fn react_1(&mut self,
                   #[allow(unused)] ctx: &mut ::reactor_rt::LogicalCtx,
                   #[allow(unused)] params: &PingParams,
                   _receive: & ::reactor_rt::InputPort<u32>,
                   #[allow(unused)] serve: & ::reactor_rt::LogicalAction::<()>) {
            if self.pingsLeft > 0 {
                ctx.schedule(serve, Offset::Asap);
            } else {
                ctx.request_stop();
            }
        }

    }

    /// Parameters for the construction of a [Ping]
    #[derive(Clone)]
    pub struct PingParams {
        pub count: u32,
    }


//------------------------//


    pub struct PingDispatcher {
        _id: ::reactor_rt::ReactorId,
        _impl: Ping,
        _params: PingParams,
        _startup_reactions: ::reactor_rt::ReactionSet,
        _shutdown_reactions: ::reactor_rt::ReactionSet,
        pub send: ::reactor_rt::OutputPort<u32>,
        pub receive: ::reactor_rt::InputPort<u32>,
        serve: ::reactor_rt::LogicalAction::<()>,
    }


    reaction_ids!(pub enum PingReactions {R0 = 0,R1 = 1,});
    use std::sync::{Arc, Mutex};

    impl PingDispatcher {
        #[inline]
        fn user_assemble(_id: ::reactor_rt::ReactorId, args: PingParams) -> Self {
            let PingParams { count } = args.clone();
            Self {
                _id,
                _params: args,
                _startup_reactions: Default::default(),
                _shutdown_reactions: Default::default(),
                _impl: Ping {
                    pingsLeft: count,
                },
                send: Default::default(),
                receive: Default::default(),
                serve: ::reactor_rt::LogicalAction::<()>::new("serve", None),
            }
        }
    }

    use ::reactor_rt::*; // after this point there's no user-written code

    impl ::reactor_rt::ReactorDispatcher for PingDispatcher {
        type ReactionId = PingReactions;
        type Wrapped = Ping;
        type Params = PingParams;

        fn assemble(args: Self::Params, assembler: &mut AssemblyCtx) -> Self {
            // children reactors


            // assemble self
            let this_reactor = assembler.get_next_id();
            let mut _self = (Self::user_assemble(this_reactor, args));

            let react_0 = new_reaction!(this_reactor, _self, R0);
            let react_1 = new_reaction!(this_reactor, _self, R1);

            {

                _self._startup_reactions = vec![react_0.clone()];
                _self._shutdown_reactions = vec![];

                _self.send.set_downstream(vec![].into());
                _self.receive.set_downstream(vec![react_1.clone()].into());
                _self.serve.set_downstream(vec![react_0.clone()].into());
            }
            {
                // Declare connections
            }


            _self
        }

        #[inline]
        fn react(&mut self, ctx: &mut ::reactor_rt::LogicalCtx, rid: Self::ReactionId) {
            match rid {
                PingReactions::R0 => {
                    self._impl.react_0(ctx, &self._params, &self.serve, &mut self.send)
                }
                ,
                PingReactions::R1 => {
                    self._impl.react_1(ctx, &self._params, &self.receive, &self.serve)
                }
            }
        }
    }


    impl ::reactor_rt::ErasedReactorDispatcher for PingDispatcher {

        fn id(&self) -> ReactorId {
            self._id
        }

        fn react_erased(&mut self, ctx: &mut ::reactor_rt::LogicalCtx, rid: u32) {
            let rid = <PingReactions as int_enum::IntEnum>::from_int(rid).unwrap();
            self.react(ctx, rid)
        }

        fn cleanup_tag(&mut self, ctx: ::reactor_rt::LogicalCtx) {
            // todo
        }

        fn enqueue_startup(&self, ctx: &mut StartupCtx) {


            ctx.enqueue(&self._startup_reactions);
        }

        fn enqueue_shutdown(&self, ctx: &mut StartupCtx) {
            ctx.enqueue(&self._shutdown_reactions);
        }

    }
}

mod pong {

    // todo link to source
    pub struct Pong {
        count: u32,
    }

    #[warn(unused)]
    impl Pong {

        // --- reaction(receive) -> send {= ... =}
        fn react_0(&mut self,
                   #[allow(unused)] ctx: &mut ::reactor_rt::LogicalCtx,
                   #[allow(unused)] params: &PongParams,
                   receive: & ::reactor_rt::InputPort<u32>,
                   send: &mut ::reactor_rt::OutputPort<u32>) {
            self.count += 1;
            ctx.set(send, ctx.get(receive).unwrap());
        }

        // --- reaction(shutdown) {= ... =}
        fn react_1(&mut self,
                   #[allow(unused)] ctx: &mut ::reactor_rt::LogicalCtx,
                   #[allow(unused)] params: &PongParams,
        ) {
            if self.count != params.expected {
                panic!("Pong expected to receive {} inputs, but it received {}.", params.expected, self.count);
            }
        }

    }

    /// Parameters for the construction of a [Pong]
    #[derive(Clone)]
    pub struct PongParams {
        pub expected: u32,
    }


//------------------------//


    pub struct PongDispatcher {
        _id: ::reactor_rt::ReactorId,
        _impl: Pong,
        _params: PongParams,
        _startup_reactions: ::reactor_rt::ReactionSet,
        _shutdown_reactions: ::reactor_rt::ReactionSet,
        pub send: ::reactor_rt::OutputPort<u32>,
        pub receive: ::reactor_rt::InputPort<u32>,
    }


    reaction_ids!(
  pub enum PongReactions {R0 = 0,R1 = 1,}
);

    impl PongDispatcher {
        #[inline]
        fn user_assemble(_id: ::reactor_rt::ReactorId, args: PongParams) -> Self {
            let PongParams { expected } = args.clone();
            Self {
                _id,
                _params: args,
                _startup_reactions: Default::default(),
                _shutdown_reactions: Default::default(),
                _impl: Pong {
                    count: 0,
                },
                send: Default::default(),
                receive: Default::default(),
            }
        }
    }

    use ::reactor_rt::*; // after this point there's no user-written code
    use std::sync::{Arc, Mutex};

    impl ::reactor_rt::ReactorDispatcher for PongDispatcher {
        type ReactionId = PongReactions;
        type Wrapped = Pong;
        type Params = PongParams;

        fn assemble(args: Self::Params, assembler: &mut AssemblyCtx) -> Self {
            // children reactors


            // assemble self
            let this_reactor = assembler.get_next_id();
            let mut _self = Self::user_assemble(this_reactor, args);

            let react_0 = new_reaction!(this_reactor, _self, R0);
            let react_1 = new_reaction!(this_reactor, _self, R1);

            {
                _self._startup_reactions = vec![];
                _self._shutdown_reactions = vec![react_1.clone()];

                _self.send.set_downstream(vec![].into());
                _self.receive.set_downstream(vec![react_0.clone()].into());
            }
            {
                // Declare connections
            }


            _self
        }

        #[inline]
        fn react(&mut self, ctx: &mut ::reactor_rt::LogicalCtx, rid: Self::ReactionId) {
            match rid {
                PongReactions::R0 => {
                    self._impl.react_0(ctx, &self._params, &self.receive, &mut self.send)
                }
                ,
                PongReactions::R1 => {
                    self._impl.react_1(ctx, &self._params)
                }
            }
        }
    }


    impl ::reactor_rt::ErasedReactorDispatcher for PongDispatcher {

        fn id(&self) -> ReactorId {
            self._id
        }

        fn react_erased(&mut self, ctx: &mut ::reactor_rt::LogicalCtx, rid: u32) {
            let rid = <PongReactions as int_enum::IntEnum>::from_int(rid).unwrap();
            self.react(ctx, rid)
        }

        fn cleanup_tag(&mut self, ctx: ::reactor_rt::LogicalCtx) {
            // todo
        }

        fn enqueue_startup(&self, ctx: &mut StartupCtx) {


            ctx.enqueue(&self._startup_reactions);
        }

        fn enqueue_shutdown(&self, ctx: &mut StartupCtx) {
            ctx.enqueue(&self._shutdown_reactions);
        }

    }
}
