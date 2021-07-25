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

use std::sync::mpsc::{channel, Receiver, Sender};

use priority_queue::PriorityQueue;

use crate::*;

use super::{Event, ReactionWave, TimeCell};
use std::ops::Deref;

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
    receiver: Receiver<Event>,

    /// A sender bound to the receiver, which may be cloned.
    canonical_sender: Sender<Event>,

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
    reactors: Vec<Box<Arc<Mutex<dyn ErasedReactorDispatcher + 'static>>>>,
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
    pub fn get_next_id(&mut self) -> ReactorId {
        self.scheduler.reactor_id.get_and_increment()
    }

    pub fn consume_child_reactor<S: ReactorDispatcher + 'static>(&mut self, child: Arc<Mutex<S>>) {
        // note that the child ID does not correspond to the index in that vector.
        #[cfg(debug_assertions)]
            {
                let child = child.lock().unwrap();
                assert_eq!(child.id().to_usize(), self.scheduler.reactors.len(), "Improper initialization order!");
            }

        self.scheduler.reactors.push(Box::new(child))
    }

    #[inline]
    pub fn assemble_sub<S: ReactorDispatcher>(&mut self, args: S::Params) -> Arc<Mutex<S>> {
        AssemblyCtx::assemble_impl(&mut self.scheduler, args)
    }

    #[inline]
    fn assemble_impl<S: ReactorDispatcher>(scheduler: &mut SyncScheduler, args: S::Params) -> Arc<Mutex<S>> {
        let mut sub = AssemblyCtx { scheduler };
        S::assemble(args, &mut sub)
    }
}

impl SyncScheduler {
    pub fn run_main<R: ReactorDispatcher>(options: SchedulerOptions, args: R::Params) {
        let mut scheduler = Self::new(options);
        AssemblyCtx::assemble_impl::<R>(&mut scheduler, args);
        scheduler.startup();
        scheduler.launch_event_loop()
    }

    fn get_reactor(&self, id: ReactorId) -> &Box<Arc<Mutex<dyn ErasedReactorDispatcher + 'static>>> {
        self.reactors.get(id.to_usize()).unwrap()
    }

    /// Creates a new scheduler. An empty scheduler doesn't
    /// do anything unless some events are pushed to the queue.
    /// See [Self::launch_async].
    pub fn new(options: SchedulerOptions) -> Self {
        let (sender, receiver) = channel::<Event>();
        Self {
            latest_logical_time: <_>::default(),
            receiver,
            canonical_sender: sender,
            queue: PriorityQueue::new(),
            initial_time: None,
            shutdown_time: None,
            options,
            reactors: Vec::new(),
            reactor_id: ReactorId::first(),
        }
    }


    /// Fix the origin of the logical timeline to the current
    /// physical time, and allows running the startup reactions
    /// of all reactors in the provided closure (see [ReactorAssembler::start]).
    ///
    /// Possible usage:
    /// ```ignore
    /// let mut scheduler = SyncScheduler::new();
    ///
    /// scheduler.startup(|mut starter| {
    ///     starter.start(&mut s_cell);
    ///     starter.start(&mut p_cell);
    /// });
    /// ```
    ///
    fn startup(&mut self) {
        let initial_time = LogicalInstant::now();
        self.initial_time = Some(initial_time);
        if let Some(timeout) = self.options.timeout {
            self.shutdown_time = Some(initial_time + timeout)
        }
        self.execute_wave(initial_time, ErasedReactorDispatcher::enqueue_startup);
    }

    fn shutdown(&mut self) {
        let shutdown_time = self.shutdown_time.unwrap_or_else(LogicalInstant::now);
        self.execute_wave(shutdown_time, ErasedReactorDispatcher::enqueue_shutdown);
    }

    fn execute_wave(&mut self,
                    time: LogicalInstant,
                    enqueue_fun: fn(&(dyn ErasedReactorDispatcher + 'static), &mut StartupCtx)) {
        let mut wave = self.new_wave(time);
        let mut ctx = StartupCtx { ctx: wave.new_ctx() };
        for reactor in &self.reactors {
            let reactor = reactor.lock().unwrap();
            enqueue_fun(reactor.deref(), &mut ctx);
        }
        // now execute all reactions that were scheduled
        // todo toposort reactions here
        let todo = ctx.ctx.do_next.clone();
        wave.consume(todo);
    }


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
            let now = PhysicalInstant::now();
            if let Some(shutdown_t) = self.shutdown_time {
                // we need to shutdown even if there are more events in the queue
                if now > shutdown_t.instant {
                    break;
                }
            }

            // flush pending events, this doesn't block
            while let Ok(evt) = self.receiver.try_recv() {
                self.push_event(evt);
            }

            if let Some((evt, _)) = self.queue.pop() {
                // execute the wave for this event.
                self.step(evt);
            } else if let Some(evt) = self.receive_event(now) { // this may block
                self.push_event(evt);
                continue;
            } else {
                // all senders have hung up, or timeout
                #[cfg(bench)] {
                    eprintln!("Shutting down scheduler");
                }
                break;
            }
        } // end loop

        // self destructor is called here
    }

    fn receive_event(&mut self, now: PhysicalInstant) -> Option<Event> {
        if self.options.keep_alive {
            if let Some(shutdown_t) = self.shutdown_time {
                if now < shutdown_t.instant {
                    // we don't have to shutdown yet, so we can wait
                    #[cfg(bench)] {
                        eprintln!("Waiting for next event.");
                    }
                    return self.receiver.recv_timeout(shutdown_t.instant.duration_since(now)).ok();
                }
            }
        }
        None
    }

    /// Push a single event to the event queue
    fn push_event(&mut self, evt: Event) {
        #[cfg(bench)] {
            eprintln!("Pushing {:?}.", evt);
        }

        let eta = evt.process_at;
        self.queue.push(evt, Reverse(eta));
    }

    /// Execute a wave. This may make the calling thread
    /// (the scheduler one) sleep, if the expected processing
    /// time (logical) is ahead of current physical time.
    fn step(&mut self, event: Event) {
        #[cfg(bench)] {
            eprintln!("Next event has tag {}", event.process_at);
        }

        let time = Self::catch_up_physical_time(event.process_at);
        self.latest_logical_time.lock().unwrap().set(time); // set the time so that scheduler links can know that.
        self.new_wave(time).consume(event.todo);
    }

    fn catch_up_physical_time(up_to_time: LogicalInstant) -> LogicalInstant {
        let now = PhysicalInstant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
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

    // todo
    // #[inline]
    // pub fn scheduler_link(&mut self) -> SchedulerLink {
    //     SchedulerLink {
    //         last_processed_logical_time: self.scheduler.latest_logical_time.clone(),
    //         sender: self.scheduler.canonical_sender.clone(),
    //     }
    // }
}


