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
use std::marker::PhantomData;

use std::sync::mpsc::{channel, Receiver, Sender};

use priority_queue::PriorityQueue;

use crate::*;

use super::{Event, ReactionWave, TimeCell};

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
    shutdown_time: Option<PhysicalInstant>,
    options: SchedulerOptions,

    /// All reactors.
    reactors: Vec<Box<dyn ErasedReactorDispatcher + 'static>>,
    reactor_id: ReactorId,
}

pub struct AssemblyCtx<'a, T: ReactorAssembler> {
    id: &'a mut ReactorId,
    my_id: ReactorId,
    _p: PhantomData<T>,
}

impl<'a, T: ReactorAssembler> AssemblyCtx<'a, T> {
    #[inline]
    pub fn get_id(&self) -> ReactorId {
        self.my_id
    }

    pub fn assemble_sub<R: ReactorAssembler>(&mut self, args: <R::RState as ReactorDispatcher>::Params) -> R {
        AssemblyCtx::<R>::do_assembly(&mut self.id, args)
    }

    fn do_assembly<R: ReactorAssembler>(id: &mut ReactorId, args: <R::RState as ReactorDispatcher>::Params) -> R {
        let mut sub = AssemblyCtx { my_id: id.get_and_increment(), id, _p: PhantomData };
        R::assemble(&mut sub, args)
    }
}

impl SyncScheduler {
    fn do_assembly<R: ReactorAssembler>(&mut self, args: <R::RState as ReactorDispatcher>::Params) -> R {
        AssemblyCtx::<R>::do_assembly(&mut self.reactor_id, args)
    }

    pub fn run_main<R: ReactorAssembler>(options: SchedulerOptions, args: <R::RState as ReactorDispatcher>::Params) {
        let mut scheduler = Self::new(options);
        let mut topcell: R = scheduler.do_assembly(args);
        scheduler.startup(|mut starter| {
            topcell.start(&mut starter);
        });
        scheduler.launch_sync()
    }

    fn get_reactor(&self, id: ReactorId) -> &Box<dyn ErasedReactorDispatcher + 'static> {
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
    /// TODO why not merge launch_async into this function
    pub fn startup(&mut self, startup_actions: impl FnOnce(&mut StartupCtx)) {
        let initial_time = LogicalInstant::now();
        self.initial_time = Some(initial_time);
        if let Some(timeout) = self.options.timeout {
            self.shutdown_time = Some(initial_time.instant + timeout)
        }
        let mut startup_wave = self.new_wave(initial_time);
        let mut startup_ctx = StartupCtx {
            scheduler: self,
            ctx: startup_wave.new_ctx(),
        };
        startup_actions(&mut startup_ctx);
        let todo = startup_ctx.ctx.do_next.clone();
        startup_wave.consume(todo);
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
    fn launch_sync(mut self) {
        /************************************************
         * This is the main event loop of the scheduler *
         ************************************************/
        loop {
            let now = PhysicalInstant::now();
            if let Some(shutdown_t) = self.shutdown_time {
                // we need to shutdown even if there are more events in the queue
                if now > shutdown_t {
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
                // todo i'm not 100% sure that this cfg(bench) works properly
                // Previously I was using a cfg(not(feature = "benchmarking")) with a custom feature
                // but that's tedious to use.
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
                if now < shutdown_t {
                    // we don't have to shutdown yet, so we can wait
                    #[cfg(bench)] {
                        eprintln!("Waiting for next event.");
                    }
                    return self.receiver.recv_timeout(shutdown_t.duration_since(now)).ok();
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
    scheduler: &'a mut SyncScheduler,
    ctx: LogicalCtx<'a>,
}

impl<'a> StartupCtx<'a> {
    #[inline]
    pub fn logical_ctx<'b>(&'b mut self) -> &'b mut LogicalCtx<'a> {
        &mut self.ctx
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


