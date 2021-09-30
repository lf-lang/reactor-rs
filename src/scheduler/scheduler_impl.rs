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

//! Home of the scheduler component.



use std::fmt::Write;
use std::sync::mpsc::{channel, Receiver, Sender};

use crossbeam::scope;
use crossbeam::thread::Scope;
use index_vec::IndexVec;

use crate::*;
use crate::CleanupCtx;
use crate::scheduler::depgraph::{DataflowInfo, ExecutableReactions};

use super::*;

pub struct SchedulerOptions {
    pub keep_alive: bool,
    pub timeout: Option<Duration>,
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self {
            keep_alive: false,
            timeout: None,
        }
    }
}

/// The runtime scheduler.
pub struct SyncScheduler<'a, 'x, 't> where 'x: 't {
    /// The latest processed logical time (necessarily behind physical time).
    /// This is Clone, Send and Sync; it's accessible from the physical contexts
    /// handed out to asynchronous event producers (physical triggers).
    latest_processed_tag: &'x TimeCell,

    dataflow: &'x DataflowInfo,

    thread_spawner: &'a Scope<'t>,

    /// The receiver end of the communication channels. Reactions
    /// contexts each have their own [Sender]. The main event loop
    /// polls this to make progress.
    ///
    /// The receiver is unique.
    rx: Receiver<Event<'x>>,

    /// A sender bound to the receiver, which may be cloned.
    tx: Sender<Event<'x>>,

    /// Initial time of the logical system. Only filled in
    /// when startup has been called.
    initial_time: Option<LogicalInstant>,
    /// Scheduled shutdown time. If not None, shutdown must
    /// be initiated at least at this physical time step.
    /// todo does this match lf semantics?
    shutdown_time: Option<LogicalInstant>,
    options: SchedulerOptions,

    /// All reactors.
    reactors: IndexVec<ReactorId, Box<dyn ReactorBehavior + 'static + Send>>,

    id_registry: IdRegistry,
}

impl<'a, 'x, 't> SyncScheduler<'a, 'x, 't> where 'x: 't {
    pub fn run_main<R: ReactorInitializer + Send + 'static>(options: SchedulerOptions, args: R::Params) {
        let mut root_assembler = RootAssembler::default();
        let mut assembler = AssemblyCtx::new::<R>(&mut root_assembler, ReactorDebugInfo::root::<R::Wrapped>());

        let main_reactor = R::assemble(args, &mut assembler).unwrap();
        assembler.register_reactor(main_reactor);


        let RootAssembler { graph, reactors, id_registry, .. } = root_assembler;

        #[cfg(feature = "graph-dump")] {
            eprintln!("{}", graph.format_dot(&id_registry));
        }

        // collect dependency information
        let dependency_info = DataflowInfo::new(graph).unwrap();
        let time_cell = AtomicCell::new(LogicalInstant::now());

        // Using thread::scope here introduces an unnamed lifetime for
        // the scope, which is captured as 't by the SyncScheduler.
        // This is useful because it captures the constraint that the
        // time_cell and dependency_info outlive 't, so that physical
        // contexts can be spawned in a thread that captures references
        // to 'x.
        scope(|scope| {
            let mut scheduler = SyncScheduler::new(
                options,
                reactors,
                id_registry,
                &dependency_info,
                &time_cell,
                scope,
            );

            info!("Triggering startup...");
            scheduler.startup();
            scheduler.launch_event_loop();
        }).unwrap();
    }

    /// Creates a new scheduler. An empty scheduler doesn't
    /// do anything unless some events are pushed to the queue.
    /// See [Self::launch_async].
    fn new(
        options: SchedulerOptions,
        reactors: IndexVec<ReactorId, Box<dyn ReactorBehavior + 'static + Send>>,
        id_registry: IdRegistry,
        dependency_info: &'x DataflowInfo,
        latest_processed_tag: &'x TimeCell,
        thread_spawner: &'a Scope<'t>,
    ) -> Self {
        let (sender, receiver) = channel::<Event<'x>>();
        Self {
            latest_processed_tag,
            rx: receiver,
            tx: sender,
            initial_time: None,
            shutdown_time: None,
            options,
            dataflow: dependency_info,
            reactors,
            id_registry,
            thread_spawner,
        }
    }


    /// Fix the origin of the logical timeline to the current
    /// physical time, and runs the startup reactions
    /// of all reactors.
    ///
    fn startup(&mut self) {
        let initial_time = LogicalInstant::now();
        self.initial_time = Some(initial_time);
        if let Some(timeout) = self.options.timeout {
            trace!("Timeout specified, will shut down at tag {}", self.display_tag(initial_time + timeout));

            self.shutdown_time = Some(initial_time + timeout)
        }

        debug_assert!(!self.reactors.is_empty(), "No registered reactors");
        self.execute_wave(initial_time, ReactorBehavior::enqueue_startup);
    }

    fn shutdown(&mut self) {
        let shutdown_time = self.shutdown_time.unwrap_or_else(LogicalInstant::now);
        self.execute_wave(shutdown_time, ReactorBehavior::enqueue_shutdown);
    }

    fn execute_wave(&mut self,
                    time: LogicalInstant,
                    enqueue_fun: fn(&(dyn ReactorBehavior + 'static), &mut StartupCtx)) {
        let mut wave = self.new_wave(time);
        let mut ctx = StartupCtx { ctx: wave.new_ctx() };
        for reactor in &self.reactors {
            enqueue_fun(reactor.as_ref(), &mut ctx);
        }
        // now execute all reactions that were scheduled
        let todo = ctx.ctx.do_next;

        self.consume_wave(wave, todo.unwrap_or_default())
    }

    fn consume_wave(&mut self, wave: ReactionWave<'_, 'x, 't>, plan: Cow<'x, ExecutableReactions>) {
        let logical_time = wave.logical_time;
        match wave.consume(self, plan) {
            WaveResult::Continue => {}
            WaveResult::StopRequested => {
                let time = logical_time.next_microstep();
                info!("Shutdown requested and scheduled at {}", self.display_tag(time));
                self.shutdown_time = Some(time)
            }
        }

        // cleanup tag-specific resources, eg clear port values

        let ctx = CleanupCtx { tag: logical_time };
        // TODO measure performance of cleaning up all reactors w/ virtual dispatch like this.
        for reactor in &mut self.reactors {
            reactor.cleanup_tag(&ctx)
        }
    }

    pub(in super) fn get_reactor_mut(&mut self, id: ReactorId) -> &mut Box<dyn ReactorBehavior + Send> {
        &mut self.reactors[id]
    }

    /// Launch the event loop in this thread.
    ///
    /// Note that this assumes [startup] has already been called.
    fn launch_event_loop(mut self) {
        let mut event_queue: EventQueue<'x> = Default::default();

        /************************************************
         * This is the main event loop of the scheduler *
         ************************************************/
        loop {
            // flush pending events, this doesn't block
            for evt in self.rx.try_iter() {
                self.do_push_event(&mut event_queue, evt);
            }

            if let Some(evt) = event_queue.take_earliest() {
                if self.is_after_shutdown(evt.tag) {
                    trace!("Event is late, shutting down - event tag: {}", self.display_tag(evt.tag));
                    break;
                }
                // execute the wave for this event.
                trace!("Processing event for tag {}", self.display_tag(evt.tag));
                self.step(evt);
            } else if let Some(evt) = self.receive_event() { // this may block
                self.do_push_event(&mut event_queue, evt);
                continue;
            } else {
                // all senders have hung up, or timeout
                break;
            }
        } // end loop

        info!("Scheduler is shutting down...");
        self.shutdown();
        info!("Scheduler has been shut down")

        // self destructor is called here
    }

    fn do_push_event(&self, event_queue: &mut EventQueue<'x>, evt: Event<'x>) {
        trace!("Pushing {}", self.display_event(&evt, evt.tag));
        event_queue.insert(&self.dataflow, evt);
    }

    /// Returns whether the given event should be ignored and
    /// the event loop be terminated. This would be the case
    /// if the tag of the event is later than the projected
    /// shutdown time. Such 'late' events may be emitted by
    /// the shutdown wave.
    fn is_after_shutdown(&self, t: LogicalInstant) -> bool {
        self.shutdown_time.map(|shutdown_t| shutdown_t < t).unwrap_or(false)
    }

    fn receive_event(&mut self) -> Option<Event<'x>> {
        let now = PhysicalInstant::now();
        //fixme keepalive doesn't work as in C
        // if self.options.keep_alive {
        if let Some(shutdown_t) = self.shutdown_time {
            if now < shutdown_t.instant {
                // we don't have to shutdown yet, so we can wait
                let timeout = shutdown_t.instant.duration_since(now);

                trace!("Will wait for next event {} ns", timeout.as_nanos());

                return self.rx.recv_timeout(timeout).ok();
            }
            // }
        }
        None
    }

    /// Execute a wave. This may make the calling thread
    /// (the scheduler one) sleep, if the expected processing
    /// time (logical) is ahead of current physical time.
    fn step(&mut self, event: Event<'x>) {
        let time = Self::catch_up_physical_time(event.tag);
        self.latest_processed_tag.store(time); // set the time so that scheduler links can know that.

        let wave = self.new_wave(time);
        self.consume_wave(wave, event.reactions);
    }

    fn catch_up_physical_time(up_to_time: LogicalInstant) -> LogicalInstant {
        let now = PhysicalInstant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
            trace!("  - Need to sleep {} ns", t.as_nanos());
            std::thread::sleep(t); // todo: see crate shuteyes for nanosleep capabilities on linux/macos platforms
        } else if now > up_to_time.instant {
            let delay = now - up_to_time.instant;
            trace!("  - Running late by {} ns = {} µs = {} ms", delay.as_nanos(), delay.as_micros(), delay.as_millis())
        }
        // note this doesn't use `now` because we use
        // scheduled time identity to associate values
        // with actions
        up_to_time
    }

    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    fn new_wave(&self, current_time: LogicalInstant) -> ReactionWave<'a, 'x, 't> {
        ReactionWave::new(
            self.tx.clone(),
            current_time,
            // note: initializing self.initial_time is the
            // first thing done during startup so the unwrap
            // should never panic
            self.initial_time.unwrap(),
            self.dataflow,
            self.latest_processed_tag,
            self.thread_spawner,
        )
    }

    fn display_tag(&self, tag: LogicalInstant) -> String {
        display_tag_impl(self.initial_time.unwrap(), tag)
    }

    fn display_event(&self, evt: &Event, process_at: LogicalInstant) -> String {
        let mut str = format!("Event(at {}: run [", self.display_tag(process_at));

        for (layer_no, batch) in evt.reactions.batches() {
            write!(str, "{}: ", layer_no).unwrap();
            join_to!(&mut str, batch.iter(), ", ", "{", "}", |x| self.display_reaction(*x)).unwrap();
        }

        str += "])";
        str
    }

    #[inline]
    pub(in super) fn display_reaction(&self, global: GlobalReactionId) -> String {
        self.id_registry.fmt_reaction(global)
    }
}

/// Allows directly enqueuing reactions for a future,
/// unspecified logical time. This is only relevant
/// during the initialization of reactors.
pub struct StartupCtx<'b, 'a, 'x, 't> {
    ctx: ReactionCtx<'b, 'a, 'x, 't>,
}

impl StartupCtx<'_, '_, '_, '_> {
    #[inline]
    #[doc(hidden)]
    pub fn enqueue(&mut self, reactions: &Vec<GlobalReactionId>) {
        self.ctx.enqueue_now(Cow::Owned(self.ctx.make_executable(reactions)))
    }

    #[doc(hidden)]
    pub fn start_timer(&mut self, t: &Timer) {
        let downstream = self.ctx.reactions_triggered_by(t.get_id());
        if t.offset.is_zero() {
            // no offset
            self.ctx.enqueue_now(Cow::Borrowed(downstream))
        } else {
            self.ctx.enqueue_later(downstream, self.ctx.get_logical_time() + t.offset)
        }
    }
}


