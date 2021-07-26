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

/// Home of the scheduler component.
///

use std::cmp::Reverse;
use std::ops::Deref;
use std::sync::mpsc::{channel, Receiver, Sender};

use priority_queue::PriorityQueue;

use crate::*;

use super::*;
use index_vec::IndexVec;


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

/// Main public API for the scheduler. Contains the priority queue
/// and public launch routine with event loop.
pub struct SyncScheduler {
    /// The latest processed logical time (necessarily behind physical time)
    latest_logical_time: TimeCell,

    /// The receiver end of the communication channels. Reactions
    /// contexts each have their own [Sender]. The main event loop
    /// polls this to make progress.
    ///
    /// The receiver is unique.
    receiver: Receiver<ScheduledEvent>,

    /// A sender bound to the receiver, which may be cloned.
    canonical_sender: Sender<ScheduledEvent>,

    /// A queue of events, which orders events according to their logical time.
    /// It needs to be reversed so that smallest delay == greatest priority.
    /// TODO work out your own data structure that merges events scheduled at the same time
    queue: PriorityQueue<Event, Reverse<LogicalInstant>>,

    /// Initial time of the logical system. Only filled in
    /// when startup has been called.
    initial_time: Option<LogicalInstant>,
    /// Scheduled shutdown time. If not None, shutdown must
    /// be initiated at least at this physical time step.
    /// todo does this match lf semantics?
    shutdown_time: Option<LogicalInstant>,
    options: SchedulerOptions,

    /// All reactors.
    reactors: IndexVec<ReactorId, Box<dyn ReactorBehavior + 'static>>,
    reactor_id: ReactorId,
}

/// Helper struct to assemble reactors during initialization.
///
/// Params:
/// - `RA` - the type of the reactor currently being assembled
/// - `'x` - the lifetime of the execution
///
pub struct AssemblyCtx<'x> {
    scheduler: &'x mut SyncScheduler,
}

impl<'x> AssemblyCtx<'x> {
    #[inline]
    //noinspection RsSelfConvention
    pub fn get_next_id(&mut self) -> ReactorId {
        let cur = self.scheduler.reactor_id;
        self.scheduler.reactor_id += 1;
        cur
    }

    pub fn register_reactor<S: ReactorInitializer + 'static>(&mut self, child: S) {
        let vec_id = self.scheduler.reactors.push(Box::new(child));
        debug_assert_eq!(self.scheduler.reactors[vec_id].id(), vec_id, "Improper initialization order!");
    }

    #[inline]
    pub fn assemble_sub<S: ReactorInitializer>(&mut self, args: S::Params) -> S {
        AssemblyCtx::assemble_impl(&mut self.scheduler, args)
    }

    #[inline]
    fn assemble_impl<S: ReactorInitializer>(scheduler: &mut SyncScheduler, args: S::Params) -> S {
        let mut sub = AssemblyCtx { scheduler };
        S::assemble(args, &mut sub)
    }
}

impl SyncScheduler {
    pub fn run_main<R: ReactorInitializer + 'static>(options: SchedulerOptions, args: R::Params) {
        let mut scheduler = Self::new(options);
        let mut assembler = AssemblyCtx { scheduler: &mut scheduler };

        let main_reactor = R::assemble(args, &mut assembler);
        assembler.register_reactor(main_reactor);

        scheduler.startup();
        scheduler.launch_event_loop()
    }

    /// Creates a new scheduler. An empty scheduler doesn't
    /// do anything unless some events are pushed to the queue.
    /// See [Self::launch_async].
    pub fn new(options: SchedulerOptions) -> Self {
        let (sender, receiver) = channel::<ScheduledEvent>();
        Self {
            latest_logical_time: <_>::default(),
            receiver,
            canonical_sender: sender,
            queue: <_>::default(),
            initial_time: None,
            shutdown_time: None,
            options,
            reactors: <_>::default(),
            reactor_id: <_>::default(),
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
            enqueue_fun(reactor.deref(), &mut ctx);
        }
        // now execute all reactions that were scheduled
        // todo toposort reactions here
        let todo = ctx.ctx.do_next.clone();

        self.consume_wave(wave, todo)
    }

    fn consume_wave(&mut self, wave: ReactionWave, todo: Vec<GlobalReactionId>) {
        let logical_time = wave.logical_time;
        match wave.consume(self, todo) {
            WaveResult::Continue => {}
            WaveResult::StopRequested => {
                let time = logical_time.next_microstep();
                info!("Shutdown requested and scheduled at {}", self.display_tag(time));
                self.shutdown_time = Some(time)
            }
        }
    }

    pub(in super) fn exec_reaction(&mut self, ctx: &mut LogicalCtx, id: &GlobalReactionId) {
        let reactor = self.reactors.get_mut(id.container).unwrap();
        reactor.react_erased(ctx, id.local)
    }
}

// note: we can't use a method because sometimes it would self.push_event because it would borrow self twice...
macro_rules! push_event {
        ($_self:expr, $evt:expr, $time:expr) => {{
            let evt = $evt;
            let process_at = $time;
            trace!("Pushing {}", $_self.display_event(&evt, process_at));
            $_self.queue.push(evt, Reverse(process_at));
        }};
    }

impl SyncScheduler {

    /// Launch the event loop in an auxiliary thread. Returns
    /// the handle for that thread.
    ///
    /// Note that this will do nothing unless some other part
    /// of the reactor program pushes events into the queue,
    /// for instance,
    /// - some thread is scheduling physical actions through a [SchedulerLink]
    /// - some startup reaction has set a port or scheduled a logical action
    /// Both of those should be taken care of by calling [Self::startup]
    /// before launching the scheduler.
    ///
    /// The loop exits when the queue has been empty for a longer
    /// time than the specified timeout. The timeout should be
    /// chosen with care to the application requirements.
    // TODO track whether there are live [SchedulerLink] to prevent idle spinning?
    fn launch_event_loop(mut self) {
        /************************************************
         * This is the main event loop of the scheduler *
         ************************************************/
        loop {

            // flush pending events, this doesn't block
            for ScheduledEvent(evt, process_at) in self.receiver.try_iter() {
                push_event!(self, evt, process_at);
            }

            if let Some((evt, Reverse(process_at))) = self.queue.pop() {
                if self.is_after_shutdown(process_at) {
                    trace!("Event is late, shutting down {}", self.display_tag(process_at));
                    break;
                }
                // execute the wave for this event.
                trace!("Processing event for tag {}", self.display_tag(process_at));
                self.step(evt, process_at);
            } else if let Some(ScheduledEvent(evt, process_at)) = self.receive_event() { // this may block
                push_event!(self, evt, process_at);
                continue;
            } else {
                // all senders have hung up, or timeout
                break;
            }
        } // end loop

        self.queue.clear();
        info!("Scheduler is shutting down...");
        self.shutdown();
        info!("Scheduler has been shut down")

        // self destructor is called here
    }

    /// Returns whether the given event should be ignored and
    /// the event loop be terminated. This would be the case
    /// if the tag of the event is later than the projected
    /// shutdown time. Such 'late' events may be emitted by
    /// the shutdown wave.
    fn is_after_shutdown(&self, t: LogicalInstant) -> bool {
        self.shutdown_time.map(|shutdown_t| shutdown_t < t).unwrap_or(false)
    }

    fn receive_event(&mut self) -> Option<ScheduledEvent> {
        let now = PhysicalInstant::now();
        if self.options.keep_alive {
            if let Some(shutdown_t) = self.shutdown_time {
                if now < shutdown_t.instant {
                    // we don't have to shutdown yet, so we can wait
                    let timeout = shutdown_t.instant.duration_since(now);

                    trace!("Will wait for next event {} ns", timeout.as_nanos());

                    return self.receiver.recv_timeout(timeout).ok();
                }
            }
        }
        None
    }

    /// Push a single event to the event queue
    fn push_event(&mut self, evt: Event, process_at: LogicalInstant) {
        trace!("Pushing {}", self.display_event(&evt, process_at));

        self.queue.push(evt, Reverse(process_at));
    }

    /// Execute a wave. This may make the calling thread
    /// (the scheduler one) sleep, if the expected processing
    /// time (logical) is ahead of current physical time.
    fn step(&mut self, event: Event, process_at: LogicalInstant) {
        let time = Self::catch_up_physical_time(process_at);
        self.latest_logical_time.lock().unwrap().set(time); // set the time so that scheduler links can know that.

        let wave = self.new_wave(time);
        self.consume_wave(wave, event.todo)
    }

    fn catch_up_physical_time(up_to_time: LogicalInstant) -> LogicalInstant {
        let now = PhysicalInstant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
            trace!("  - Need to sleep {} ns", t.as_nanos());
            std::thread::sleep(t); // todo: see crate shuteyes for nanosleep capabilities on linux/macos platforms
        }
        // note this doesn't use `now` because we use
        // scheduled time identity to associate values
        // with actions
        //                        vvvvvvvvvv
        LogicalInstant { instant: up_to_time.instant, microstep: MicroStep::ZERO }
    }

    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    fn new_wave(&self, logical_time: LogicalInstant) -> ReactionWave {
        ReactionWave::new(
            self.canonical_sender.clone(),
            logical_time,
            // note: initializing self.initial_time is the
            // first thing done during startup so the unwrap
            // should never panic
            self.initial_time.unwrap(),
        )
    }

    fn display_tag(&self, tag: LogicalInstant) -> String {
        let elapsed = tag.instant - self.initial_time.unwrap().instant;
        format!("(T0 + {} ns = {} ms, {})", elapsed.as_nanos(), elapsed.as_millis(), tag.microstep)
    }

    fn display_event(&self, evt: &Event, process_at: LogicalInstant) -> String {
        format!("Event(at {}: run {})", self.display_tag(process_at), CommaList(&evt.todo))
    }
}

/// The API of [SyncScheduler::startup].
pub struct StartupCtx<'a> {
    ctx: LogicalCtx<'a>,
}

impl<'a> StartupCtx<'a> {
    #[inline]
    pub fn enqueue(&mut self, reactions: &ReactionSet) {
        self.ctx.enqueue_now(reactions)
    }

    pub fn start_timer(&mut self, t: &Timer) {
        if t.offset.is_zero() {
            // no offset
            self.ctx.enqueue_now(&t.downstream)
        } else {
            self.ctx.enqueue_later(&t.downstream, self.ctx.get_logical_time() + t.offset)
        }
    }

    // todo physical actions
    // #[inline]
    // pub fn scheduler_link(&mut self) -> SchedulerLink {
    //     SchedulerLink {
    //         last_processed_logical_time: self.scheduler.latest_logical_time.clone(),
    //         sender: self.scheduler.canonical_sender.clone(),
    //     }
    // }
}


