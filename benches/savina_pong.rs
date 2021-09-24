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


use std::sync::{Arc, Mutex};

use ::reactor_rt::{LogicalInstant, PhysicalInstant, Duration};
use ::reactor_rt::Offset::{After, Asap};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main, black_box};

use reactor_rt::*;

/*
The ping/pong game from Savina benchmarks. This can be compared
to the C implementation (see results.md).

See original at https://github.com/icyphy/lingua-franca/blob/f5868bec199e02f784393f32b594be5df935e2ee/benchmark/C/Savina/PingPong.lf


 */

criterion_group!(benches, reactor_main);
criterion_main!(benches);

fn reactor_main(c: &mut Criterion) {

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
    let main_args = reactors::SavinaPongParams {
        count,
    };

    SyncScheduler::run_main::<reactors::SavinaPongAdapter>(options, main_args);
}

//-------------------//
//---- REACTORS -----//
//-------------------//
mod reactors {
    pub use self::pong::PongAdapter;
    pub use self::pong::PongParams;
    pub use self::ping::PingAdapter;
    pub use self::ping::PingParams;
    pub use self::savina_pong::SavinaPongAdapter;
    pub use self::savina_pong::SavinaPongParams;
    //--------------------------------------------//
    //------------ Pong -------//
    //-------------------//
    mod pong {
        //-- Generated by LFC @ 2021/09/24 19:48:39 --//
        #![allow(unused)]

        use ::reactor_rt::{LogicalInstant, PhysicalInstant, Duration};
        use ::reactor_rt::Offset::{After, Asap};
        use std::sync::{Arc, Mutex};



        /// Generated from /home/clem/Documents/LF/reactor-rust/benches/SavinaPong.lf:25
        ///
        /// --- reactor Pong(expected: u32(1000000)) { ... }
        pub struct Pong {
            count: u32,
        }

        #[warn(unused)]
        impl Pong {

            // --- reaction(receive) -> send {= ... =}
            fn react_0(&mut self,
                       #[allow(unused)] ctx: &mut ::reactor_rt::ReactionCtx,
                       #[allow(unused)] params: &PongParams,
                       receive: ::reactor_rt::ReadablePort<u32>,
                       #[allow(unused_mut)] mut send: ::reactor_rt::WritablePort<u32>,) {
                self.count += 1;
                ctx.set(send, ctx.get(receive).unwrap());
            }

            // --- reaction(shutdown) {= ... =}
            fn react_1(&mut self,
                       #[allow(unused)] ctx: &mut ::reactor_rt::ReactionCtx,
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


        pub struct PongAdapter {
            _id: ::reactor_rt::ReactorId,
            _impl: Pong,
            _params: PongParams,
            _startup_reactions: ::reactor_rt::ReactionSet,
            _shutdown_reactions: ::reactor_rt::ReactionSet,
            pub port_send: ::reactor_rt::Port<u32>,
            pub port_receive: ::reactor_rt::Port<u32>,
        }

        impl PongAdapter {
            #[inline]
            fn user_assemble(_assembler: &mut ::reactor_rt::AssemblyCtx, _params: PongParams) -> Self {
                let PongParams { expected, } = _params.clone();
                Self {
                    _id: _assembler.get_id(),
                    _params,
                    _startup_reactions: Default::default(),
                    _shutdown_reactions: Default::default(),
                    _impl: Pong {
                        count: 0,
                    },
                    port_send: _assembler.new_port("send"),
                    port_receive: _assembler.new_port("receive"),
                }
            }
        }

        use ::reactor_rt::*; // after this point there's no user-written code

    impl ::reactor_rt::ReactorInitializer for PongAdapter {
        type Wrapped = Pong;
        type Params = PongParams;
        const MAX_REACTION_ID: LocalReactionId = LocalReactionId::new_const(2);

        fn assemble(args: Self::Params, _assembler: &mut AssemblyCtx) -> Result<Self, AssemblyError> {
            // children reactors
            let () = {
                let PongParams { expected, } = args.clone();

                ()
            };

            let self_id = _assembler.fix_cur_id();
            // declared before sub-components, so their local id is between zero and MAX_REACTION_ID
            let [react_0,
            react_1] = _assembler.new_reactions::<{Self::MAX_REACTION_ID.index()}>();

            // assemble self
            let mut _self: Self = Self::user_assemble(_assembler, args);


            {
                _self._startup_reactions = vec![];
                _self._shutdown_reactions = vec![react_1,];

                // --- reaction(receive) -> send {= ... =}
                _assembler.declare_triggers(_self.port_receive.get_id(), react_0)?;
                _assembler.effects_port(react_0, &_self.port_send)?;
                // --- reaction(shutdown) {= ... =}

                // Declare connections
            }


            Ok(_self)
        }
    }


        impl ReactorBehavior for PongAdapter {

            #[inline]
            fn id(&self) -> ReactorId {
                self._id
            }

            fn react_erased(&mut self, ctx: &mut ReactionCtx, rid: LocalReactionId) {
                match rid.raw() {
                    0 => self._impl.react_0(ctx, &self._params,::reactor_rt::ReadablePort::new(&self.port_receive), ::reactor_rt::WritablePort::new(&mut self.port_send),),
                    1 => self._impl.react_1(ctx, &self._params),

                    _ => panic!("Invalid reaction ID: {} should be < {}", rid, Self::MAX_REACTION_ID)
                }
            }

            fn cleanup_tag(&mut self, ctx: &CleanupCtx) {
                ctx.cleanup_port(&mut self.port_send);
                ctx.cleanup_port(&mut self.port_receive);
            }

            fn enqueue_startup(&self, ctx: &mut StartupCtx) {
                ctx.enqueue(&self._startup_reactions);

            }

            fn enqueue_shutdown(&self, ctx: &mut StartupCtx) {
                ctx.enqueue(&self._shutdown_reactions);
            }

        }
    }


    //--------------------------------------------//
    //------------ Ping -------//
    //-------------------//
    mod ping {
        //-- Generated by LFC @ 2021/09/24 19:48:39 --//
        #![allow(unused)]

        use ::reactor_rt::{LogicalInstant, PhysicalInstant, Duration};
        use ::reactor_rt::Offset::{After, Asap};
        use std::sync::{Arc, Mutex};



        /// Generated from /home/clem/Documents/LF/reactor-rust/benches/SavinaPong.lf:5
        ///
        /// --- reactor Ping(count: u32(1000000)) { ... }
        pub struct Ping {
            pingsLeft: u32,
        }

        #[warn(unused)]
        impl Ping {

            // --- reaction(startup, serve) -> send {= ... =}
            fn react_0(&mut self,
                       #[allow(unused)] ctx: &mut ::reactor_rt::ReactionCtx,
                       #[allow(unused)] params: &PingParams,
                       #[allow(unused)] serve: &mut ::reactor_rt::LogicalAction<()>,
                       #[allow(unused_mut)] mut send: ::reactor_rt::WritablePort<u32>,) {
                ctx.set(send, self.pingsLeft);
                self.pingsLeft -= 1;
            }

            // --- reaction (receive) -> serve {= ... =}
            fn react_1(&mut self,
                       #[allow(unused)] ctx: &mut ::reactor_rt::ReactionCtx,
                       #[allow(unused)] params: &PingParams,
                       receive: ::reactor_rt::ReadablePort<u32>,
                       #[allow(unused_mut)] mut serve: &mut ::reactor_rt::LogicalAction<()>,) {
                if self.pingsLeft > 0 {
                    ctx.schedule(serve, Asap);
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


        pub struct PingAdapter {
            _id: ::reactor_rt::ReactorId,
            _impl: Ping,
            _params: PingParams,
            _startup_reactions: ::reactor_rt::ReactionSet,
            _shutdown_reactions: ::reactor_rt::ReactionSet,
            pub port_send: ::reactor_rt::Port<u32>,
            pub port_receive: ::reactor_rt::Port<u32>,
            action_serve: ::reactor_rt::LogicalAction<()>,
        }

        impl PingAdapter {
            #[inline]
            fn user_assemble(_assembler: &mut ::reactor_rt::AssemblyCtx, _params: PingParams) -> Self {
                let PingParams { count, } = _params.clone();
                Self {
                    _id: _assembler.get_id(),
                    _params,
                    _startup_reactions: Default::default(),
                    _shutdown_reactions: Default::default(),
                    _impl: Ping {
                        pingsLeft: count,
                    },
                    port_send: _assembler.new_port("send"),
                    port_receive: _assembler.new_port("receive"),
                    action_serve: _assembler.new_logical_action("serve", None),
                }
            }
        }

        use ::reactor_rt::*; // after this point there's no user-written code

    impl ::reactor_rt::ReactorInitializer for PingAdapter {
        type Wrapped = Ping;
        type Params = PingParams;
        const MAX_REACTION_ID: LocalReactionId = LocalReactionId::new_const(2);

        fn assemble(args: Self::Params, _assembler: &mut AssemblyCtx) -> Result<Self, AssemblyError> {
            // children reactors
            let () = {
                let PingParams { count, } = args.clone();

                ()
            };

            let self_id = _assembler.fix_cur_id();
            // declared before sub-components, so their local id is between zero and MAX_REACTION_ID
            let [react_0,
            react_1] = _assembler.new_reactions::<{Self::MAX_REACTION_ID.index()}>();

            // assemble self
            let mut _self: Self = Self::user_assemble(_assembler, args);


            {
                _self._startup_reactions = vec![react_0,];
                _self._shutdown_reactions = vec![];

                // --- reaction(startup, serve) -> send {= ... =}
                _assembler.declare_triggers(_self.action_serve.get_id(), react_0)?;
                _assembler.effects_port(react_0, &_self.port_send)?;
                // --- reaction (receive) -> serve {= ... =}
                _assembler.declare_triggers(_self.port_receive.get_id(), react_1)?;

                // Declare connections
            }


            Ok(_self)
        }
    }


        impl ReactorBehavior for PingAdapter {

            #[inline]
            fn id(&self) -> ReactorId {
                self._id
            }

            fn react_erased(&mut self, ctx: &mut ReactionCtx, rid: LocalReactionId) {
                match rid.raw() {
                    0 => self._impl.react_0(ctx, &self._params,&mut self.action_serve, ::reactor_rt::WritablePort::new(&mut self.port_send),),
                    1 => self._impl.react_1(ctx, &self._params,::reactor_rt::ReadablePort::new(&self.port_receive), &mut self.action_serve,),

                    _ => panic!("Invalid reaction ID: {} should be < {}", rid, Self::MAX_REACTION_ID)
                }
            }

            fn cleanup_tag(&mut self, ctx: &CleanupCtx) {
                ctx.cleanup_port(&mut self.port_send);
                ctx.cleanup_port(&mut self.port_receive);
                ctx.cleanup_action(&mut self.action_serve);
            }

            fn enqueue_startup(&self, ctx: &mut StartupCtx) {
                ctx.enqueue(&self._startup_reactions);

            }

            fn enqueue_shutdown(&self, ctx: &mut StartupCtx) {
                ctx.enqueue(&self._shutdown_reactions);
            }

        }
    }


    //--------------------------------------------//
    //------------ SavinaPong -------//
    //-------------------//
    mod savina_pong {
        //-- Generated by LFC @ 2021/09/24 19:48:39 --//
        #![allow(unused)]

        use ::reactor_rt::{LogicalInstant, PhysicalInstant, Duration};
        use ::reactor_rt::Offset::{After, Asap};
        use std::sync::{Arc, Mutex};



        /// Generated from /home/clem/Documents/LF/reactor-rust/benches/SavinaPong.lf:42
        ///
        /// --- main reactor SavinaPong(count: u32(1000000)) { ... }
        pub struct SavinaPong {

        }

        #[warn(unused)]
        impl SavinaPong {



        }

        /// Parameters for the construction of a [SavinaPong]
        #[derive(Clone)]
        pub struct SavinaPongParams {
            pub count: u32,
        }


        //------------------------//


        pub struct SavinaPongAdapter {
            _id: ::reactor_rt::ReactorId,
            _impl: SavinaPong,
            _params: SavinaPongParams,
            _startup_reactions: ::reactor_rt::ReactionSet,
            _shutdown_reactions: ::reactor_rt::ReactionSet,

        }

        impl SavinaPongAdapter {
            #[inline]
            fn user_assemble(_assembler: &mut ::reactor_rt::AssemblyCtx, _params: SavinaPongParams) -> Self {
                let SavinaPongParams { count, } = _params.clone();
                Self {
                    _id: _assembler.get_id(),
                    _params,
                    _startup_reactions: Default::default(),
                    _shutdown_reactions: Default::default(),
                    _impl: SavinaPong {

                    },

                }
            }
        }

        use ::reactor_rt::*; // after this point there's no user-written code

    impl ::reactor_rt::ReactorInitializer for SavinaPongAdapter {
        type Wrapped = SavinaPong;
        type Params = SavinaPongParams;
        const MAX_REACTION_ID: LocalReactionId = LocalReactionId::new_const(0);

        fn assemble(args: Self::Params, _assembler: &mut AssemblyCtx) -> Result<Self, AssemblyError> {
            // children reactors
            let (mut ping, mut pong,) = {
                let SavinaPongParams { count, } = args.clone();
                // --- ping = new Ping(count=count);
                let ping: super::PingAdapter = _assembler.assemble_sub(super::PingParams { count, })?;
                // --- pong = new Pong(expected=count);
                let pong: super::PongAdapter = _assembler.assemble_sub(super::PongParams { expected: count, })?;
                (ping, pong,)
            };

            let self_id = _assembler.fix_cur_id();
            // declared before sub-components, so their local id is between zero and MAX_REACTION_ID
            let [] = _assembler.new_reactions::<{Self::MAX_REACTION_ID.index()}>();

            // assemble self
            let mut _self: Self = Self::user_assemble(_assembler, args);


            {
                _self._startup_reactions = vec![];
                _self._shutdown_reactions = vec![];



                // Declare connections
                // --- ping.send -> pong.receive;
                _assembler.bind_ports(&mut ping.port_send, &mut pong.port_receive)?;
                // --- pong.send -> ping.receive;
                _assembler.bind_ports(&mut pong.port_send, &mut ping.port_receive)?;
            }
            _assembler.register_reactor(ping);
            _assembler.register_reactor(pong);

            Ok(_self)
        }
    }


        impl ReactorBehavior for SavinaPongAdapter {

            #[inline]
            fn id(&self) -> ReactorId {
                self._id
            }

            fn react_erased(&mut self, ctx: &mut ReactionCtx, rid: LocalReactionId) {
                match rid.raw() {


                    _ => panic!("Invalid reaction ID: {} should be < {}", rid, Self::MAX_REACTION_ID)
                }
            }

            fn cleanup_tag(&mut self, ctx: &CleanupCtx) {

            }

            fn enqueue_startup(&self, ctx: &mut StartupCtx) {
                ctx.enqueue(&self._startup_reactions);

            }

            fn enqueue_shutdown(&self, ctx: &mut StartupCtx) {
                ctx.enqueue(&self._shutdown_reactions);
            }

        }
    }



}
