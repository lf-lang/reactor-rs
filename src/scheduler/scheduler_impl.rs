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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::reconnectable::*;

use super::assembly_impl::RootAssembler;
use super::*;
use crate::assembly::*;
use crate::scheduler::dependencies::DataflowInfo;
use crate::*;

/// Construction parameters for the scheduler.
///
/// LFC uses target properties to set them. With the "cli"
/// feature, generated programs also feature CLI options to
/// override the defaults at runtime.
#[derive(Default)]
pub struct SchedulerOptions {
    /// If true, we won't shut down the scheduler as soon as
    /// the event queue is empty, provided there are still
    /// live threads that can send messages to the scheduler
    /// asynchronously.
    pub keep_alive: bool,

    /// Timeout of reactor execution. If provided, the reactor
    /// program will be shut down *at the latest* at `T0 + timeout`.
    /// Calls to `request_stop` may make the program terminate earlier.
    pub timeout: Option<Duration>,

    /// Max number of threads to use in the thread pool.
    /// If zero, uses one thread per core. Ignored unless
    /// building with feature `parallel-runtime`.
    pub threads: usize,

    /// If true, dump the dependency graph to a file before
    /// starting execution.
    pub dump_graph: bool,
}

// Macros are placed a bit out of order to avoid exporting them
// (they're only visible in code placed AFTER them).
// We use macros instead of private methods as the borrow checker
// needs to know we're borrowing disjoint parts of self at any time.

macro_rules! debug_info {
    ($e:expr) => {
        DebugInfoProvider { id_registry: &$e.id_registry }
    };
}

macro_rules! push_event {
    ($scheduler:expr, $evt:expr) => {{
        trace!("Pushing {}", debug_info!($scheduler).display_event(&$evt));
        $scheduler.event_queue.push($evt);
    }};
}

/// The runtime scheduler.
///
/// Lifetime parameters: 'x and 't are carried around everywhere,
/// 'x allows us to take references into the dataflow graph, and
/// 't to spawn new scoped threads for physical actions. 'a is more
/// useless but is needed to compile.
pub struct SyncScheduler<'x> {
    /// The latest processed logical time (necessarily behind physical time).
    latest_processed_tag: Option<EventTag>,

    /// Reference to the data flow graph, which allows us to
    /// order reactions properly for each tag.
    dataflow: &'x DataflowInfo,

    /// All reactors.
    reactors: ReactorVec<'x>,

    /// Pending events/ tags to process.
    event_queue: EventQueue<'x>,

    /// Receiver through which asynchronous events are
    /// communicated to the scheduler. We only block when
    /// no events are ready to be processed.
    rx: Receiver<PhysicalEvent>,

    /// Initial time of the logical system.
    #[allow(unused)] // might be useful someday
    initial_time: Instant,

    /// Scheduled shutdown time. If Some, shutdown will be
    /// initiated at that logical time.
    ///
    /// This is set when an event sent by a [ReactionCtx::request_stop]
    /// is *processed* (so, at its given tag), and upon
    /// initialization if a timeout was specified.
    shutdown_time: Option<EventTag>,

    /// Whether the app has been terminated. Only used for
    /// communication with asynchronous threads. Set by the
    /// scheduler only.
    was_terminated: Arc<AtomicBool>,

    /// Debug information.
    id_registry: DebugInfoRegistry,
}

impl<'x> SyncScheduler<'x> {
    pub fn run_main<R: ReactorInitializer + 'static>(options: SchedulerOptions, args: R::Params) {
        let start = Instant::now();
        info!("Starting assembly...");
        let (reactors, graph, id_registry) = RootAssembler::assemble_tree::<R>(args);
        let time = Instant::now() - start;
        info!("Assembly done in {} µs...", time.as_micros());

        if options.dump_graph {
            use std::fs::File;
            use std::io::Write;

            let path = std::env::temp_dir().join("reactors.dot");

            File::create(path.clone())
                .and_then(|mut dot_file| writeln!(dot_file, "{}", graph.format_dot(&id_registry)))
                .expect("Error while writing DOT file");
            eprintln!("Wrote dot file to {}", path.to_string_lossy());
        }

        // collect dependency information
        let dataflow_info = DataflowInfo::new(graph).map_err(|e| e.lift(&id_registry)).unwrap();

        // Using thread::scope here introduces an unnamed lifetime for
        // the scope, which is captured as 't by the SyncScheduler.
        // This is useful because it captures the constraint that the
        // dataflow_info outlives 't, so that physical contexts
        // can be spawned in threads that capture references
        // to 'x.
        let initial_time = Instant::now();
        #[cfg(feature = "parallel-runtime")]
        let rayon_thread_pool = rayon::ThreadPoolBuilder::new().num_threads(options.threads).build().unwrap();

        let scheduler = SyncScheduler::new(options, id_registry, &dataflow_info, reactors, initial_time);

        cfg_if::cfg_if! {
            if #[cfg(feature = "parallel-runtime")] {
                /// The unsafe impl is safe if scheduler instances
                /// are only sent between threads like this (their Rc
                /// internals are not copied).
                /// So long as the framework entirely controls the lifetime
                /// of SyncScheduler instances, this is enforceable.
                unsafe impl Send for SyncScheduler<'_> {}

                // install makes calls to parallel iterators use that thread pool
                rayon_thread_pool.install(|| scheduler.launch_event_loop());
            } else {
                scheduler.launch_event_loop();
            }
        }
    }

    /// Launch the event loop in this thread.
    fn launch_event_loop(mut self) {
        /************************************************
         * This is the main event loop of the scheduler *
         ************************************************/

        self.startup();

        loop {
            // flush pending events, this doesn't block
            for evt in self.rx.try_iter() {
                let evt = evt.make_executable(self.dataflow);
                push_event!(self, evt);
            }

            if let Some(evt) = self.event_queue.take_earliest() {
                if self.is_after_shutdown(evt.tag) {
                    trace!("Event is late, shutting down - event tag: {}", evt.tag);
                    break;
                }
                trace!("Processing event {}", self.debug().display_event(&evt));
                match self.catch_up_physical_time(evt.tag.to_logical_time(self.initial_time)) {
                    Ok(_) => {}
                    Err(async_event) => {
                        let async_event = async_event.make_executable(self.dataflow);
                        // an asynchronous event woke our sleep
                        if async_event.tag < evt.tag {
                            // reinsert both events to order them and try again.
                            push_event!(self, evt);
                            push_event!(self, async_event);
                            continue;
                        } else {
                            // we can process this event first and not care about the async event
                            push_event!(self, async_event);
                        }
                    }
                };
                // at this point we're at the correct time

                if evt.terminate || self.shutdown_time == Some(evt.tag) {
                    return self.shutdown(evt.tag, evt.reactions);
                }

                self.process_tag(false, evt.tag, evt.reactions);
            } else if let Some(evt) = self.receive_event() {
                let evt = evt.make_executable(self.dataflow);
                // this may block
                push_event!(self, evt);
                continue;
            } else {
                // all senders have hung up, or timeout
                info!("Event queue is empty forever, shutting down.");
                break;
            }
        } // end loop

        let shutdown_tag = self.shutdown_time.unwrap_or_else(|| EventTag::now(self.initial_time));
        self.shutdown(shutdown_tag, None);

        // self destructor is called here
    }

    /// Creates a new scheduler. An empty scheduler doesn't
    /// do anything unless some events are pushed to the queue.
    /// See [Self::launch_event_loop].
    fn new(
        options: SchedulerOptions,
        id_registry: DebugInfoRegistry,
        dependency_info: &'x DataflowInfo,
        reactors: ReactorVec<'x>,
        initial_time: Instant,
    ) -> Self {
        if !cfg!(feature = "parallel-runtime") && options.threads != 0 {
            warn!("'workers' runtime parameter has no effect unless feature 'parallel-runtime' is enabled")
        }

        if options.keep_alive {
            warn!("'keepalive' runtime parameter has no effect in the Rust target")
        }

        let (_, rx) = unbounded::<PhysicalEvent>();
        Self {
            rx,

            event_queue: Default::default(),
            reactors,

            initial_time,
            latest_processed_tag: None,
            shutdown_time: options.timeout.map(|timeout| {
                let shutdown_tag = EventTag::ORIGIN.successor(timeout);
                trace!("Timeout specified, will shut down at most at tag {}", shutdown_tag);
                shutdown_tag
            }),
            dataflow: dependency_info,
            id_registry,
            was_terminated: Default::default(),
        }
    }

    /// Fix the origin of the logical timeline to the current
    /// physical time, and runs the startup reactions
    /// of all reactors.
    fn startup(&mut self) {
        info!("Triggering startup...");
        debug_assert!(!self.reactors.is_empty(), "No registered reactors");

        let startup_reactions = self.dataflow.reactions_triggered_by(&TriggerId::STARTUP);
        self.process_tag(false, EventTag::ORIGIN, Some(Cow::Borrowed(startup_reactions)))
    }

    fn shutdown(&mut self, shutdown_tag: EventTag, reactions: ReactionPlan<'x>) {
        info!("Scheduler is shutting down, at {}", shutdown_tag);
        self.shutdown_time = Some(shutdown_tag);
        let default_plan: ReactionPlan<'x> = Some(Cow::Borrowed(self.dataflow.reactions_triggered_by(&TriggerId::SHUTDOWN)));
        let reactions = ExecutableReactions::merge_cows(reactions, default_plan);

        self.process_tag(true, shutdown_tag, reactions);

        // notify concurrent threads.
        self.was_terminated.store(true, Ordering::SeqCst);
        info!("Scheduler has been shut down")
    }

    /// Returns whether the given event should be ignored and
    /// the event loop be terminated. This would be the case
    /// if the tag of the event is later than the projected
    /// shutdown time. Such 'late' events may be emitted by
    /// the shutdown wave.
    fn is_after_shutdown(&self, t: EventTag) -> bool {
        self.shutdown_time.map(|shutdown_t| shutdown_t < t).unwrap_or(false)
    }

    /// Wait for an asynchronous event for as long as we can
    /// expect it.
    fn receive_event(&mut self) -> Option<PhysicalEvent> {
        if let Some(shutdown_t) = self.shutdown_time {
            let absolute = shutdown_t.to_logical_time(self.initial_time);
            if let Some(timeout) = absolute.checked_duration_since(Instant::now()) {
                trace!("Will wait for asynchronous event {} ns", timeout.as_nanos());
                self.rx.recv_timeout(timeout).ok()
            } else {
                trace!("Cannot wait, already past programmed shutdown time...");
                None
            }
        } else {
            trace!("Will wait for asynchronous event without timeout");
            self.rx.recv().ok()
        }
    }

    /// Sleep/wait until the given time OR an asynchronous
    /// event is received first.
    fn catch_up_physical_time(&mut self, target: Instant) -> Result<(), PhysicalEvent> {
        let now = Instant::now();

        if now < target {
            let t = target - now;
            trace!("  - Need to sleep {} ns", t.as_nanos());
            // we use recv_timeout as a thread::sleep so that
            // our sleep is interrupted properly when an async
            // event arrives
            match self.rx.recv_timeout(t) {
                Ok(async_evt) => {
                    trace!(
                        "  - Sleep interrupted by async event for tag {}, going back to queue",
                        async_evt.tag
                    );
                    return Err(async_evt);
                }
                Err(RecvTimeoutError::Timeout) => { /*great*/ }
                Err(RecvTimeoutError::Disconnected) => {
                    // ok, there are no physical actions in the program so it's useless to block on self.rx
                    // we still need to wait though..
                    if let Some(remaining) = target.checked_duration_since(Instant::now()) {
                        std::thread::sleep(remaining);
                    }
                }
            }
        }

        if now > target {
            let delay = now - target;
            trace!(
                "  - Running late by {} ns = {} µs = {} ms",
                delay.as_nanos(),
                delay.as_micros(),
                delay.as_millis()
            )
        }
        Ok(())
    }

    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    fn new_reaction_ctx<'a>(
        &self,
        tag: EventTag,
        todo: ReactionPlan<'x>,
        rx: &'a Receiver<PhysicalEvent>,
        debug_info: DebugInfoProvider<'a>,
        was_terminated_atomic: &'a Arc<AtomicBool>,
        was_terminated: bool,
    ) -> ReactionCtx<'a, 'x> {
        ReactionCtx::new(
            rx,
            tag,
            self.initial_time,
            todo,
            self.dataflow,
            debug_info,
            was_terminated_atomic,
            was_terminated,
        )
    }

    #[inline]
    pub(super) fn debug(&self) -> DebugInfoProvider {
        debug_info!(self)
    }

    /// Actually process a tag. The provided reactions are the
    /// root reactions that startup the "wave".
    fn process_tag(&mut self, is_shutdown: bool, tag: EventTag, mut reactions: ReactionPlan<'x>) {
        if cfg!(debug_assertions) {
            if let Some(latest) = self.latest_processed_tag {
                debug_assert!(tag > latest, "Tag ordering mismatch")
            }
        }
        self.latest_processed_tag = Some(tag);

        let mut next_level = reactions.as_ref().and_then(|todo| todo.first_batch());
        if next_level.is_none() {
            return;
        }

        let mut ctx = self.new_reaction_ctx(tag, None, &self.rx, debug_info!(self), &self.was_terminated, is_shutdown);

        while let Some((level_no, batch)) = next_level {
            let level_no = level_no.cloned();
            trace!("  - Level {}", level_no);
            ctx.cur_level = level_no.key;

            /// Minimum number of reactions (inclusive) required
            /// to parallelize reactions.
            /// TODO experiment with tweaking this
            const PARALLEL_THRESHOLD: usize = 3;

            if cfg!(feature = "parallel-runtime") && batch.len() >= PARALLEL_THRESHOLD {
                #[cfg(feature = "parallel-runtime")]
                parallel_rt_impl::process_batch(&mut ctx, &mut self.reactors, batch);
            } else {
                // the impl for non-parallel runtime
                for reaction_id in batch {
                    let reactor = &mut self.reactors[reaction_id.0.container()];
                    ctx.execute(reactor, *reaction_id);
                }
            }

            reactions = ExecutableReactions::merge_plans_after(reactions, ctx.insides.todo_now.take(), level_no.key.next());
            next_level = reactions.as_ref().and_then(|todo| todo.next_batch(level_no.as_ref()));
        }

        for evt in ctx.insides.future_events.drain(..) {
            push_event!(self, evt)
        }

        // cleanup tag-specific resources, eg clear port values
        let ctx = CleanupCtx { tag };
        // TODO measure performance of cleaning up all reactors w/ virtual dispatch like this.
        //   see also efforts in the C runtime to  avoid this
        for reactor in &mut self.reactors {
            reactor.cleanup_tag(&ctx)
        }
    }
}

#[cfg(feature = "parallel-runtime")]
mod parallel_rt_impl {
    use rayon::prelude::*;

    use super::*;
    use crate::scheduler::dependencies::Level;

    pub(super) fn process_batch(ctx: &mut ReactionCtx<'_, '_>, reactors: &mut ReactorVec<'_>, batch: &Level) {
        let reactors_mut = UnsafeSharedPointer(reactors.raw.as_mut_ptr());

        ctx.insides.absorb(
            batch
                .iter()
                .par_bridge()
                .fold_with(CloneableCtx(ctx.fork()), |CloneableCtx(mut ctx), reaction_id| {
                    // capture the newtype instead of capturing its field, which is not Send
                    let reactors_mut = &reactors_mut;
                    let reactor = unsafe {
                        // safety:
                        // - no two reactions in the batch belong to the same reactor
                        // - the vec does not change size so there is no reallocation
                        &mut *reactors_mut.0.add(reaction_id.0.container().index())
                    };

                    ctx.execute(reactor, reaction_id);

                    CloneableCtx(ctx)
                })
                .fold(RContextForwardableStuff::default, |cx1, cx2| cx1.merge(cx2.0.insides))
                .reduce(Default::default, RContextForwardableStuff::merge),
        );
    }

    #[derive(Copy, Clone)]
    struct UnsafeSharedPointer<T>(*mut T);

    unsafe impl<T> Send for UnsafeSharedPointer<T> {}

    unsafe impl<T> Sync for UnsafeSharedPointer<T> {}

    /// We need a Clone bound to use fold_with, but this clone
    /// implementation is not general purpose so I hide it.
    struct CloneableCtx<'a, 'x>(ReactionCtx<'a, 'x>);

    impl Clone for CloneableCtx<'_, '_> {
        fn clone(&self) -> Self {
            Self(self.0.fork())
        }
    }
}
