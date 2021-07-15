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
use std::time::{Duration, Instant};


use reactor_rt::reaction_ids;
use reactor_rt::reaction_ids_helper;
use reactor_rt::*;
use reactor_rt::Offset::Asap;

/*
The ping/pong game from Savina benchmarks. This can be compared
to the C implementation (see results.md).

See original at https://github.com/icyphy/lingua-franca/blob/f5868bec199e02f784393f32b594be5df935e2ee/benchmark/C/Savina/PingPong.lf


 */

fn main() {
    let timeout = Some(Duration::from_millis(2));
    let scheduler = do_assembly(5, 1_000_000, timeout);
    scheduler.launch_async().join().unwrap();
}

fn do_assembly(numIterations: u32, count: u32, timeout: Option<Duration>) -> SyncScheduler {
    let mut rid = ReactorId::first();

    // ping = new Ping(count=count);
    let mut ping_cell = PingAssembler::assemble(&mut rid, count);

    // runner = new BenchmarkRunner(numIterations=numIterations);
    let mut runner_cell = BenchmarkRunnerAssembler::assemble(&mut rid, BenchmarkParams { numIterations, useInit: false, useCleanupIteration: false });

    {
        let mut ping = ping_cell._rstate.lock().unwrap();
        let mut runner = runner_cell._rstate.lock().unwrap();

        // runner.outIterationStart -> ping.inStart;
        bind_ports(&mut runner.outIterationStart, &mut ping.inStart);

        // ping.outFinished -> runner.inIterationFinish;
        bind_ports(&mut ping.outFinished, &mut runner.inIterationFinish);
    }


    // pong = new Pong();
    let mut pong_cell = PongAssembler::assemble(&mut rid, ());

    {
        let mut ping = ping_cell._rstate.lock().unwrap();
        let mut pong = pong_cell._rstate.lock().unwrap();

        // ping.outPing -> pong.inPing;
        bind_ports(&mut ping.outPing, &mut pong.inPing);

        // pong.outPong -> ping.inPong;
        bind_ports(&mut pong.outPong, &mut ping.inPong);
    }

    let mut scheduler = SyncScheduler::new(SchedulerOptions { timeout, keep_alive: false });

    scheduler.startup(|mut starter| {
        starter.start(&mut runner_cell);
        starter.start(&mut ping_cell);
        starter.start(&mut pong_cell);
    });

    scheduler
}


struct Ping {
    pings_left: u32,
    count: u32,
}

impl Ping {
    fn react_restart(&mut self, ctx: &mut LogicalCtx, serve: &LogicalAction) {
        self.pings_left = self.count;
        ctx.schedule(serve, Asap)
    }

    fn react_serve(&mut self, ctx: &mut LogicalCtx, outPing: &mut OutputPort<bool>) {
        self.pings_left -= 1;
        ctx.set(outPing, true)
    }

    fn react_reactToPong(&mut self, ctx: &mut LogicalCtx, outFinished: &mut OutputPort<bool>, serve: &LogicalAction) {
        if self.pings_left == 0 {
            ctx.set(outFinished, true)
        } else {
            ctx.schedule(serve, Asap)
        }
    }
}


struct PingDispatcher {
    _impl: Ping,

    inStart: InputPort<bool>,
    outFinished: OutputPort<bool>,

    outPing: OutputPort<bool>,
    inPong: InputPort<bool>,

    serve: LogicalAction,
}

reaction_ids!(enum PingReactions {R_Serve, R_ReactToPong,R_Start });

impl ReactorDispatcher for PingDispatcher {
    type ReactionId = PingReactions;
    type Wrapped = Ping;
    type Params = u32;

    fn assemble(args: Self::Params) -> Self {
        Self {
            _impl: Ping { pings_left: 0, count: args },
            inStart: Default::default(),
            outFinished: Default::default(),
            outPing: Default::default(),
            inPong: Default::default(),
            serve: LogicalAction::new(None, "serve"),
        }
    }

    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId) {
        match rid {
            PingReactions::R_Serve => {
                self._impl.react_serve(ctx, &mut self.outPing)
            }
            PingReactions::R_ReactToPong => {
                self._impl.react_reactToPong(ctx, &mut self.outFinished, &self.serve)
            }
            PingReactions::R_Start => {
                self._impl.react_restart(ctx, &self.serve)
            }
        }
    }
}


struct PingAssembler {
    _rstate: Arc<Mutex<PingDispatcher>>,
    react_Serve: Arc<ReactionInvoker>,
    react_ReactToPong: Arc<ReactionInvoker>,
    react_Start: Arc<ReactionInvoker>,
}

impl ReactorAssembler for PingAssembler {
    type RState = PingDispatcher;

    fn start(&mut self, link: SchedulerLink, ctx: &mut LogicalCtx) {
        // Ping::react_startup(ctx);
    }

    fn assemble(reactor_id: &mut ReactorId, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = reactor_id.get_and_increment();
        let mut reaction_id = 0;

        let react_Serve = new_reaction!(this_reactor, reaction_id, _rstate, R_Serve);
        let react_ReactToPong = new_reaction!(this_reactor, reaction_id, _rstate, R_ReactToPong);
        let react_Start = new_reaction!(this_reactor, reaction_id, _rstate, R_Start);

        { // declare local dependencies
            let mut statemut = _rstate.lock().unwrap();


            statemut.inStart.set_downstream(vec![react_Start.clone()].into());
            statemut.inPong.set_downstream(vec![react_ReactToPong.clone()].into());
            statemut.serve.set_downstream(vec![react_Serve.clone()].into());
        }

        Self {
            _rstate,
            react_Serve,
            react_ReactToPong,
            react_Start,
        }
    }
}

/*
PONG
 */


struct Pong;

impl Pong {
    fn react_reactToPing(&mut self, ctx: &mut LogicalCtx, outPong: &mut OutputPort<bool>) {
        ctx.set(outPong, true)
    }
}


struct PongDispatcher {
    _impl: Pong,

    inPing: InputPort<bool>,
    outPong: OutputPort<bool>,
}

reaction_ids!(enum PongReactions {R_ReactToPing});

impl ReactorDispatcher for PongDispatcher {
    type ReactionId = PongReactions;
    type Wrapped = Pong;
    type Params = ();

    fn assemble(args: Self::Params) -> Self {
        Self {
            _impl: Pong,
            inPing: Default::default(),
            outPong: Default::default(),
        }
    }

    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId) {
        match rid {
            PongReactions::R_ReactToPing => {
                self._impl.react_reactToPing(ctx, &mut self.outPong)
            }
        }
    }
}


struct PongAssembler {
    _rstate: Arc<Mutex<PongDispatcher>>,
    react_ReactToPing: Arc<ReactionInvoker>,
}

impl ReactorAssembler for PongAssembler {
    type RState = PongDispatcher;

    fn start(&mut self, link: SchedulerLink, ctx: &mut LogicalCtx) {
        // Ping::react_startup(ctx);
    }

    fn assemble(reactor_id: &mut ReactorId, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = reactor_id.get_and_increment();
        let mut reaction_id = 0;

        let react_ReactToPing = new_reaction!(this_reactor, reaction_id, _rstate, R_ReactToPing);

        { // declare local dependencies
            let mut statemut = _rstate.lock().unwrap();

            statemut.inPing.set_downstream(vec![react_ReactToPing.clone()].into());
        }

        Self {
            _rstate,
            react_ReactToPing,
        }
    }
}


/*

The benchmark runner

TODO move this into a reusable file?

https://github.com/icyphy/lingua-franca/blob/c-benchmarks/benchmark/C/Savina/BenchmarkRunner.lf

 */


struct BenchmarkRunner {
    count: u32,
    start_time: Instant,
    measured_times: Vec<Duration>,
    // params
    use_init: bool,
    use_cleanup_iteration: bool,
    num_iterations: u32,
}


impl BenchmarkRunner {
    /*
    R_CleanupIteration,
    R_CleanupDone,
    R_NextIteration,
    R_IterationDone,
    R_Finish
     */

    fn react_Startup(&self, link: SchedulerLink, ctx: &mut LogicalCtx, nextIteration: &LogicalAction, initBenchmark: &LogicalAction) {
        if self.use_init {
            ctx.schedule(initBenchmark, Asap)
        } else {
            ctx.schedule(nextIteration, Asap)
        }
    }

    fn react_Init(&mut self, ctx: &mut LogicalCtx, outInitializeStart: &mut OutputPort<bool>) {
        ctx.set(outInitializeStart, true)
    }

    fn react_InitDone(&mut self, ctx: &mut LogicalCtx, nextIteration: &LogicalAction) {
        ctx.schedule(nextIteration, Asap)
    }

    fn react_CleanupIteration(&mut self, ctx: &mut LogicalCtx, outCleanupIterationStart: &mut OutputPort<bool>) {
        ctx.set(outCleanupIterationStart, true)
    }

    fn react_CleanupDone(&mut self, ctx: &mut LogicalCtx, nextIteration: &LogicalAction) {
        ctx.schedule(nextIteration, Asap)
    }

    fn react_NextIteration(&mut self, ctx: &mut LogicalCtx, outIterationStart: &mut OutputPort<bool>, finish: &LogicalAction) {
        if self.count < self.num_iterations {
            self.start_time = Instant::now();
            ctx.set(outIterationStart, true)
        } else {
            ctx.schedule(finish, Asap)
        }
    }

    fn react_IterationDone(&mut self, ctx: &mut LogicalCtx, nextIteration: &LogicalAction, cleanupIteration: &LogicalAction) {
        let end_time = ctx.get_physical_time();
        let iteration_time = end_time - self.start_time;

        self.measured_times.push(iteration_time);
        self.count += 1;

        // in the C benchmark this is a print
        println!("Iteration: {}\t Duration: {} ms\n", self.count, iteration_time.as_millis());

        if self.use_cleanup_iteration {
            ctx.schedule(cleanupIteration, Asap)
        } else {
            ctx.schedule(nextIteration, Asap)
        }
    }

    fn react_Finish(&mut self, _ctx: &mut LogicalCtx) {
        self.measured_times.sort();
        let best = self.measured_times.first().unwrap();
        let worst = self.measured_times.last().unwrap();
        let median = self.measured_times[self.measured_times.len() / 2];


        println!("Exec summary");
        println!("Best time:\t{} ms", best.as_millis());
        println!("Worst time:\t{} ms", worst.as_millis());
        println!("Median time:\t{} ms", median.as_millis());
    }
}

struct BenchmarkRunnerDispatcher {
    _impl: BenchmarkRunner,

    // inStart: InputPort<bool>,

    outIterationStart: OutputPort<bool>,
    inIterationFinish: InputPort<bool>,

    outInitializeStart: OutputPort<bool>,
    inInitializeFinish: InputPort<bool>,

    outCleanupIterationStart: OutputPort<bool>,
    inCleanupIterationFinish: InputPort<bool>,

    initBenchmark: LogicalAction,
    cleanupIteration: LogicalAction,
    nextIteration: LogicalAction,
    finish: LogicalAction,
}

reaction_ids!(enum BenchmarkRunnerReactions {
    // R_InStart,
    R_Init,
    R_InitDone,
    R_CleanupIteration,
    R_CleanupDone,
    R_NextIteration,
    R_IterationDone,
    R_Finish
});

#[derive(Copy, Clone)]
struct BenchmarkParams {
    numIterations: u32,
    useInit: bool,
    useCleanupIteration: bool,
}

impl ReactorDispatcher for BenchmarkRunnerDispatcher {
    type ReactionId = BenchmarkRunnerReactions;
    type Wrapped = BenchmarkRunner;
    type Params = BenchmarkParams;

    fn assemble(args: Self::Params) -> Self {
        let _impl = BenchmarkRunner {
            count: 0,
            start_time: Instant::now(),
            measured_times: Vec::new(), // todo capacity
            use_cleanup_iteration: args.useCleanupIteration,
            use_init: args.useInit,
            num_iterations: args.numIterations,
        };

        Self {
            _impl,
            // inStart: Default::default(),
            outIterationStart: Default::default(),
            inIterationFinish: Default::default(),
            outInitializeStart: Default::default(),
            inInitializeFinish: Default::default(),
            outCleanupIterationStart: Default::default(),
            inCleanupIterationFinish: Default::default(),
            initBenchmark: LogicalAction::new(None, "init"),
            cleanupIteration: LogicalAction::new(None, "cleanup"),
            nextIteration: LogicalAction::new(None, "next"),
            finish: LogicalAction::new(None, "finish"),
        }
    }

    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId) {
        match rid {
            // BenchmarkRunnerReactions::R_InStart => {
            //     self._impl.react_InStart(ctx, &self.nextIteration, &self.initBenchmark)
            // }
            BenchmarkRunnerReactions::R_Init => {
                self._impl.react_Init(ctx, &mut self.outInitializeStart)
            }
            BenchmarkRunnerReactions::R_InitDone => {
                self._impl.react_InitDone(ctx, &self.nextIteration)
            }
            BenchmarkRunnerReactions::R_CleanupIteration => {
                self._impl.react_CleanupIteration(ctx, &mut self.outCleanupIterationStart)
            }
            BenchmarkRunnerReactions::R_CleanupDone => {
                self._impl.react_CleanupDone(ctx, &self.nextIteration)
            }
            BenchmarkRunnerReactions::R_NextIteration => {
                self._impl.react_NextIteration(ctx, &mut self.outIterationStart, &self.finish)
            }
            BenchmarkRunnerReactions::R_IterationDone => {
                self._impl.react_IterationDone(ctx, &self.nextIteration, &self.cleanupIteration)
            }
            BenchmarkRunnerReactions::R_Finish => {
                self._impl.react_Finish(ctx)
            }
        }
    }
}


struct BenchmarkRunnerAssembler {
    _rstate: Arc<Mutex<BenchmarkRunnerDispatcher>>,
    // react_InStart: Arc<ReactionInvoker>,
    react_Init: Arc<ReactionInvoker>,
    react_InitDone: Arc<ReactionInvoker>,
    react_CleanupIteration: Arc<ReactionInvoker>,
    react_CleanupDone: Arc<ReactionInvoker>,
    react_NextIteration: Arc<ReactionInvoker>,
    react_IterationDone: Arc<ReactionInvoker>,
    react_Finish: Arc<ReactionInvoker>,
}

impl ReactorAssembler for BenchmarkRunnerAssembler {
    type RState = BenchmarkRunnerDispatcher;


    fn start(&mut self, link: SchedulerLink, ctx: &mut LogicalCtx) {
        let  rstate = self._rstate.lock().unwrap();
        rstate._impl.react_Startup(link, ctx, &rstate.nextIteration, &rstate.initBenchmark);

        // BenchmarkRunner::react_startup(ctx);
    }

    fn assemble(reactor_id: &mut ReactorId, args: <Self::RState as ReactorDispatcher>::Params) -> Self {
        let mut _rstate = Arc::new(Mutex::new(Self::RState::assemble(args)));
        let this_reactor = reactor_id.get_and_increment();
        let mut reaction_id = 0;

        // let react_InStart = new_reaction!(rid, _rstate, R_InStart);
        let react_Init = new_reaction!(this_reactor, reaction_id, _rstate, R_Init);
        let react_InitDone = new_reaction!(this_reactor, reaction_id, _rstate, R_InitDone);
        let react_CleanupIteration = new_reaction!(this_reactor, reaction_id, _rstate, R_CleanupIteration);
        let react_CleanupDone = new_reaction!(this_reactor, reaction_id, _rstate, R_CleanupDone);
        let react_NextIteration = new_reaction!(this_reactor, reaction_id, _rstate, R_NextIteration);
        let react_IterationDone = new_reaction!(this_reactor, reaction_id, _rstate, R_IterationDone);
        let react_Finish = new_reaction!(this_reactor, reaction_id, _rstate, R_Finish);

        { // declare local dependencies
            let mut statemut = _rstate.lock().unwrap();


            // statemut.inStart.set_downstream(vec![react_InStart.clone()].into());
            statemut.inInitializeFinish.set_downstream(vec![react_InitDone.clone()].into());
            statemut.inCleanupIterationFinish.set_downstream(vec![react_CleanupDone.clone()].into());
            statemut.inIterationFinish.set_downstream(vec![react_IterationDone.clone()].into());


            statemut.cleanupIteration.set_downstream(vec![react_CleanupIteration.clone()].into());
            statemut.nextIteration.set_downstream(vec![react_NextIteration.clone()].into());
            statemut.finish.set_downstream(vec![react_Finish.clone()].into());
            statemut.initBenchmark.set_downstream(vec![react_Init.clone()].into());
        }

        Self {
            _rstate,
            // react_InStart,
            react_Init,
            react_InitDone,
            react_CleanupIteration,
            react_CleanupDone,
            react_NextIteration,
            react_IterationDone,
            react_Finish,
        }
    }
}

