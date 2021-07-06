/// Home of the scheduler component.
///

use std::cell::Cell;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use priority_queue::PriorityQueue;

use super::*;

/// An order to execute some reaction
type ReactionOrder = Arc<ReactionInvoker>;
/// The internal cell type used to store a thread-safe mutable logical time value.
type TimeCell = Arc<Mutex<Cell<LogicalTime>>>;

/// A simple tuple of (expected processing time, reactions to execute).
#[derive(Eq, PartialEq, Hash)]
struct Event {
    process_at: LogicalTime,
    todo: Vec<ReactionOrder>,
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
    queue: PriorityQueue<Event, Reverse<LogicalTime>>,

    /// Initial time of the logical system. Only filled in
    /// when startup has been called.
    initial_time: Option<LogicalTime>,
}

impl SyncScheduler {
    /// Creates a new scheduler. An empty scheduler doesn't
    /// do anything unless some events are pushed to the queue.
    /// See [Self::launch_async].
    pub fn new() -> Self {
        let (sender, receiver) = channel::<Event>();
        Self {
            latest_logical_time: <_>::default(),
            receiver,
            canonical_sender: sender,
            queue: PriorityQueue::new(),
            initial_time: None,
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
    pub fn startup(&mut self, startup_actions: impl FnOnce(StartupCtx)) {
        let initial_time = LogicalTime::now();
        self.initial_time = Some(initial_time);
        let startup_wave = self.new_wave(initial_time);
        startup_actions(StartupCtx { scheduler: self, initial_wave: startup_wave });
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
    pub fn launch_async(mut self, timeout: Duration) -> JoinHandle<()> {
        use std::thread;
        thread::spawn(move || {
            /************************************************
             * This is the main event loop of the scheduler *
             ************************************************/
            loop {

                // flush pending events, this doesn't block
                while let Ok(evt) = self.receiver.try_recv() {
                    self.push_event(evt);
                }

                if let Some((evt, _)) = self.queue.pop() {
                    // execute the wave for this event.
                    self.step(evt);
                } else if let Ok(evt) = self.receiver.recv_timeout(timeout) { // this will block
                    self.push_event(evt);
                    continue;
                } else {
                    // all senders have hung up, or timeout
                    #[cfg(not(feature = "benchmarking"))] {
                        eprintln!("Shutting down scheduler, channel timed out after {} ms", timeout.as_millis());
                    }
                    break;
                }
            } // end loop
            assert!(self.queue.is_empty(), "Program exited with pending events!");
            // self destructor is called here
        })
    }

    /// Push a single event to the event queue
    fn push_event(&mut self, evt: Event) {
        let eta = evt.process_at;
        self.queue.push(evt, Reverse(eta));
    }

    /// Execute a wave. This may make the calling thread
    /// (the scheduler one) sleep, if the expected processing
    /// time (logical) is ahead of current physical time.
    fn step(&mut self, event: Event) {
        let time = Self::catch_up_physical_time(event.process_at);
        self.latest_logical_time.lock().unwrap().set(time); // set the time so that scheduler links can know that.
        self.new_wave(time).consume(event.todo);
    }

    fn catch_up_physical_time(up_to_time: LogicalTime) -> LogicalTime {
        let now = Instant::now();
        if now < up_to_time.instant {
            let t = up_to_time.instant - now;
            std::thread::sleep(t); // todo: see crate shuteyes for nanosleep capabilities on linux/macos platforms
            LogicalTime::now()
        } else {
            LogicalTime { instant: now, microstep: 0 }
        }
    }

    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    fn new_wave(&self, logical_time: LogicalTime) -> ReactionWave {
        ReactionWave {
            logical_time,
            sender: self.canonical_sender.clone(),
        }
    }
}

/// The API of [SyncScheduler::startup].
pub struct StartupCtx<'a> {
    scheduler: &'a mut SyncScheduler,
    initial_wave: ReactionWave,
}

impl<'a> StartupCtx<'a> {
    /// Execute the startup reaction of the given assembler.
    pub fn start(&mut self, r: &mut impl ReactorAssembler) {
        let ctx = SchedulerLink {
            last_processed_logical_time: self.scheduler.latest_logical_time.clone(),
            sender: self.scheduler.canonical_sender.clone(),
        };
        r.start(ctx, &mut self.initial_wave.new_ctx())
    }
}

/// A "wave" of reactions executing at the same logical time.
/// Waves can enqueue new reactions to execute at the same time,
/// they're processed in exec order.
///
/// todo would there be a way to "split" waves into workers?
struct ReactionWave {
    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    logical_time: LogicalTime,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<Event>,

}

impl ReactionWave {
    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    fn enqueue_later(&mut self, downstream: &ToposortedReactions, process_at: LogicalTime) {
        debug_assert!(process_at > self.logical_time);

        // todo merge events at equal tags by merging their dependencies
        let evt = Event { process_at, todo: downstream.clone() };
        self.sender.send(evt).unwrap();
    }

    fn new_ctx(&mut self) -> LogicalCtx {
        LogicalCtx { wave: self, do_next: Vec::new() }
    }

    /// Execute the wave until completion.
    /// The parameter is the list of reactions to start with.
    /// Todo topological info to split into independent subgraphs.
    fn consume(mut self, mut todo: Vec<ReactionOrder>) {
        let mut i = 0;
        // We can share it, to reuse the allocation of the do_next buffer
        let mut ctx = self.new_ctx();
        // reactions that have already been processed.
        // In some situations (diamonds) this is necessary.
        // Possibly with more static information we can avoid that.
        let mut done: HashSet<GlobalReactionId> = HashSet::new();

        while i < todo.len() {
            if let Some(reaction) = todo.get_mut(i) {
                if done.insert(reaction.id()) {
                    // this may append new elements into the queue,
                    // which is why we can't use an iterator
                    reaction.fire(&mut ctx);
                    // this clears the ctx.do_next buffer but retains its allocation
                    todo.append(&mut ctx.do_next);
                }
            }
            i += 1;
        }
    }
}

/// This is the context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler. Only the
/// interactions declared at assembly time are allowed.
///
pub struct LogicalCtx<'a> {
    wave: &'a mut ReactionWave,

    /// Remaining reactions to execute before the wave dies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in order into this
    /// queue.
    do_next: Vec<ReactionOrder>,
}

impl LogicalCtx<'_> {
    /// Get the value of a port at this time.
    pub fn get<T: Copy>(&self, port: &InputPort<T>) -> Option<T> {
        port.get()
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    pub fn set<T>(&mut self, port: &mut OutputPort<T>, value: T) {
        // TODO topology information & deduplication
        //  Eg for a diamond situation this will execute reactions several times...
        //  This is why I added a set to patch it

        port.set_impl(value, |downstream| {
            for reaction in downstream {
                // todo blindly appending possibly does not respect the topological sort
                self.do_next.push(reaction.clone());
            }
        });
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule(&mut self, action: &LogicalAction, offset: Offset) {
        self.schedule_impl(action, offset);
    }

    // private
    fn schedule_impl<T>(&mut self, action: &Action<T>, offset: Offset) {
        self.wave.enqueue_later(&action.downstream, action.make_eta(self.wave.logical_time, offset.to_duration()));
    }

    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
    }

    /// Request a shutdown which will be acted upon at the end
    /// of this reaction.
    pub fn request_shutdown(self) {
        unimplemented!()
    }

    pub fn get_logical_time(&self) -> LogicalTime {
        self.wave.logical_time
    }
}

/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside world.
#[derive(Clone)]
pub struct SchedulerLink {
    last_processed_logical_time: TimeCell,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<Event>,
}

impl SchedulerLink {
    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule_physical(&mut self, action: &PhysicalAction, offset: Offset) {
        // we have to fetch the time at which the logical timeline is currently running,
        // this may be far behind the current physical time
        let time_in_logical_subsystem = self.last_processed_logical_time.lock().unwrap().get();
        let process_at = action.make_eta(time_in_logical_subsystem, offset.to_duration());

        // todo merge events at equal tags by merging their dependencies
        let evt = Event { process_at, todo: action.downstream.clone() };
        self.sender.send(evt).unwrap();
    }
}
