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
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};

use crossbeam_utils::{thread::Scope, thread::scope};

use crate::*;
use crate::scheduler::depgraph::DataflowInfo;

use super::*;

/// Construction parameters for the scheduler.
///
/// LFC uses target properties to set them. With the "cli"
/// feature, generated programs also feature CLI options to
/// override the defaults at runtime.
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
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self {
            keep_alive: false,
            timeout: None,
        }
    }
}

// Macros are placed a bit out of order to avoid exporting them
// (they're only visible in code placed AFTER them).
// We use macros instead of private methods as the borrow checker
// needs to know we're borrowing disjoint parts of self at any time.

macro_rules! debug_info {
    ($e:expr) => {
        DebugInfoProvider {
            initial_time: $e.initial_time,
            id_registry: &$e.id_registry,
        }
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
pub struct SyncScheduler<'a, 'x, 't> where 'x: 't {
    /// The latest processed logical time (necessarily behind physical time).
    latest_processed_tag: Option<EventTag>,

    /// Reference to the data flow graph, which allows us to
    /// order reactions properly for each tag.
    dataflow: &'x DataflowInfo,

    /// Can spawn scoped threads, which are used for threads
    /// producing physical actions.
    thread_spawner: &'a Scope<'t>,

    /// All reactors.
    pub(super) reactors: ReactorVec<'x>,

    /// Pending events/ tags to process.
    event_queue: EventQueue<'x>,

    /// The receiver end of the communication channels. Reactions
    /// contexts each have their own [Sender]. The main event loop
    /// polls this to make progress.
    ///
    /// The receiver is unique.
    ///
    /// Since at least one sender ([tx]) is alive, this receiver
    /// will never report being disconnected.
    rx: Receiver<Event<'x>>,

    /// A sender bound to the receiver, which may be cloned.
    /// It's carried around in [PhysicalSchedulerLink] to
    /// handle asynchronous events. Synchronously produced
    /// go directly into the [event_queue].
    tx: Sender<Event<'x>>,


    /// Initial time of the logical system. Only filled in
    /// when startup has been called.
    initial_time: Instant,

    /// Scheduled shutdown time. If not None, shutdown must
    /// be initiated at least at this physical time step.
    /// todo does this match lf semantics?
    /// This is set when [EventPayload::Terminate] is received.
    /// todo there are two data flow paths that control shutdown, this one (self.shutdown_time)
    ///  and Terminate events sent through the sender. Unify them.
    shutdown_time: Option<EventTag>,
    options: SchedulerOptions,

    id_registry: IdRegistry,
}

impl<'a, 'x, 't> SyncScheduler<'a, 'x, 't> where 'x: 't {
    pub fn run_main<R: ReactorInitializer + Send + Sync + 'static>(options: SchedulerOptions, args: R::Params) {
        let mut root_assembler = RootAssembler::default();
        let mut assembler = AssemblyCtx::new::<R>(&mut root_assembler, ReactorDebugInfo::root::<R::Wrapped>());

        let main_reactor = R::assemble(args, &mut assembler).unwrap();
        assembler.register_reactor(main_reactor);


        let RootAssembler { graph, reactors, id_registry, .. } = root_assembler;

        #[cfg(feature = "graph-dump")] {
            eprintln!("{}", graph.format_dot(&id_registry));
        }

        // collect dependency information
        let dataflow_info = DataflowInfo::new(graph).unwrap();

        // Using thread::scope here introduces an unnamed lifetime for
        // the scope, which is captured as 't by the SyncScheduler.
        // This is useful because it captures the constraint that the
        // time_cell and dataflow_info outlive 't, so that physical
        // contexts can be spawned in a thread that captures references
        // to 'x.
        scope(|scope| {
            let initial_time = Instant::now();
            let scheduler = SyncScheduler::new(
                options,
                id_registry,
                &dataflow_info,
                scope,
                reactors,
                initial_time,
            );

            scheduler.launch_event_loop();
        }).unwrap();
    }

    /// Launch the event loop in this thread.
    fn launch_event_loop(mut self) {
        self.startup();

        /************************************************
         * This is the main event loop of the scheduler *
         ************************************************/
        loop {
            // flush pending events, this doesn't block
            for evt in self.rx.try_iter() {
                push_event!(self, evt);
            }

            if let Some(evt) = self.event_queue.take_earliest() {
                if self.is_after_shutdown(evt.tag) {
                    trace!("Event is late, shutting down - event tag: {}", self.debug().display_tag(evt.tag));
                    break;
                }
                trace!("Processing event for tag {}", self.debug().display_tag(evt.tag));
                match self.catch_up_physical_time(evt.tag.to_logical_time(self.initial_time)) {
                    Ok(_) => {},
                    Err(async_event) => {
                        // an asynchronous event woke our sleep
                        if async_event.tag < evt.tag {
                            // reinsert both events to order them and try again.
                            push_event!(self, evt);
                            push_event!(self, async_event);
                            continue
                        } else {
                            // we can process this event first and not care about the async event
                            push_event!(self, async_event);
                        }
                    }
                };

                match evt.payload {
                    EventPayload::Reactions(reactions) => self.process_tag(evt.tag, Some(reactions)),
                    EventPayload::Terminate => break,
                }
            } else if let Some(evt) = self.receive_event() { // this may block
                push_event!(self, evt);
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

    /// Creates a new scheduler. An empty scheduler doesn't
    /// do anything unless some events are pushed to the queue.
    /// See [Self::launch_event_loop].
    fn new(
        options: SchedulerOptions,
        id_registry: IdRegistry,
        dependency_info: &'x DataflowInfo,
        thread_spawner: &'a Scope<'t>,
        reactors: ReactorVec<'x>,
        initial_time: Instant,
    ) -> Self {
        let (tx, rx) = channel::<Event<'x>>();
        Self {
            rx,
            tx,

            event_queue: Default::default(),
            reactors,

            initial_time,
            latest_processed_tag: None,
            shutdown_time: None,
            options,
            dataflow: dependency_info,
            id_registry,
            thread_spawner,
        }
    }


    /// Fix the origin of the logical timeline to the current
    /// physical time, and runs the startup reactions
    /// of all reactors.
    fn startup(&mut self) {
        info!("Triggering startup...");
        let initial_time = self.initial_time;
        let initial_tag = EventTag::pure(initial_time, initial_time);
        if let Some(timeout) = self.options.timeout {
            let shutdown_tag = initial_tag.successor(initial_time, timeout);
            trace!("Timeout specified, will shut down at tag {}", self.debug().display_tag(shutdown_tag));
            self.shutdown_time = Some(shutdown_tag)
        }

        debug_assert!(!self.reactors.is_empty(), "No registered reactors");

        self.execute_wave(initial_tag, ReactorBehavior::enqueue_startup);
    }

    fn shutdown(&mut self) {
        let shutdown_time = self.shutdown_time.unwrap_or_else(|| EventTag::now(self.initial_time));
        self.execute_wave(shutdown_time, ReactorBehavior::enqueue_shutdown);
    }

    fn execute_wave(&mut self, tag: EventTag, enqueue_fun: fn(&(dyn ReactorBehavior + Send + 'x), &mut StartupCtx),
    ) {
        let mut startup_ctx = StartupCtx::new(self.new_reaction_ctx(tag, None));
        for reactor in self.reactors.iter() {
            enqueue_fun(reactor.as_ref(), &mut startup_ctx);
        }
        for evt in startup_ctx.take_future_events() {
            self.event_queue.push(evt);
        }
        self.process_tag(tag, startup_ctx.take_todo_now())
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
    fn receive_event(&mut self) -> Option<Event<'x>> {
        let now = PhysicalInstant::now();

        if !self.options.keep_alive {
            trace!("Won't wait without keepalive option");
            return None
        }

        return match self.shutdown_time {
            Some(shutdown_t) => {
                let shutdown_t = shutdown_t.to_logical_time(self.initial_time);
                if now < shutdown_t {
                    let timeout = shutdown_t.duration_since(now);
                    trace!("Will wait for asynchronous event {} ns", timeout.as_nanos());
                    self.rx.recv_timeout(timeout).map_err(|_| unreachable!("self.tx is alive")).ok()
                } else {
                    trace!("Cannot wait, already past programmed shutdown time...");
                    None
                }
            }
            None => {
                trace!("Will wait for asynchronous event indefinitely");
                self.rx.recv().map_err(|_| unreachable!("self.tx is alive")).ok()
            }
        };
    }

    /// Actually process a tag. The provided reactions are the
    /// root reactions that startup the "wave".
    fn process_tag(&mut self, tag: EventTag, reactions: Option<Cow<'x, ExecutableReactions>>) {
        if cfg!(debug_assertions) {
            if let Some(t) = self.latest_processed_tag {
                debug_assert!(tag > t, "Tag ordering mismatch")
            }
            self.latest_processed_tag = Some(tag);
        }

        if reactions.is_none() {
            return;
        }

        // note: we have to inline all this to prove to the
        // compiler that we're borrowing disjoint parts of self

        let ctx = self.new_reaction_ctx(tag, reactions);
        let event_q_borrow = &mut self.event_queue;
        let debug = debug_info!(self);
        ctx.process_entire_tag(debug, &mut self.reactors, |evt| event_q_borrow.push(evt))
    }

    /// Sleep/wait until the given time OR an asynchronous
    /// event is received first.
    fn catch_up_physical_time(&mut self, target: Instant) -> Result<(), Event<'x>> {
        let now = PhysicalInstant::now();

        if now < target {
            let t = target - now;
            trace!("  - Need to sleep {} ns", t.as_nanos());
            // we use recv_timeout as a thread::sleep so that
            // our sleep is interrupted properly when an async
            // event arrives
            match self.rx.recv_timeout(t) {
                Ok(async_evt) => {
                    trace!("  - Sleep interrupted by async event for tag {}, going back to queue", self.debug().display_tag(async_evt.tag));
                    return Err(async_evt)
                },
                Err(RecvTimeoutError::Timeout) => { /*great*/ },
                Err(RecvTimeoutError::Disconnected) => unreachable!("at least one sender should be alive in this scheduler instance")
            }
        }

        if now > target {
            let delay = now - target;
            trace!("  - Running late by {} ns = {} µs = {} ms", delay.as_nanos(), delay.as_micros(), delay.as_millis())
        }
        Ok(())
    }

    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    fn new_reaction_ctx(&self, tag: EventTag, todo: Option<Cow<'x, ExecutableReactions>>) -> ReactionCtx<'a, 'x, 't> {
        ReactionCtx::new(
            self.tx.clone(),
            tag,
            self.initial_time,
            todo,
            self.dataflow,
            self.thread_spawner,
        )
    }

    #[inline]
    pub(in super) fn debug(&self) -> DebugInfoProvider {
        debug_info!(self)
    }
}

/// Can format stuff for trace messages.
#[derive(Clone)]
pub(in super) struct DebugInfoProvider<'a> {
    id_registry: &'a IdRegistry,
    initial_time: Instant,
}

impl DebugInfoProvider<'_> {
    pub fn display_tag(&self, tag: EventTag) -> String {
        display_tag_impl(self.initial_time, tag)
    }

    pub fn display_event(&self, evt: &Event) -> String {
        match evt {
            Event { tag, payload: EventPayload::Reactions(reactions) } => {
                let mut str = format!("Event(at {}: run [", self.display_tag(*tag));

                for (layer_no, batch) in reactions.batches() {
                    write!(str, "{}: ", layer_no).unwrap();
                    join_to!(&mut str, batch.iter(), ", ", "{", "}", |x| self.display_reaction(*x)).unwrap();
                }

                str += "])";
                str
            }
            Event { tag, payload: EventPayload::Terminate } => {
                format!("Event(at {}: terminate program", self.display_tag(*tag))
            }
        }
    }

    #[inline]
    pub(in super) fn display_reaction(&self, global: GlobalReactionId) -> String {
        self.id_registry.fmt_reaction(global)
    }
}
