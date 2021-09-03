use std::collections::HashSet;
use std::sync::mpsc::Sender;

use crate::*;
use super::*;
use std::borrow::{Borrow, BorrowMut};

/// The context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
/// Only the interactions declared at assembly time
/// are allowed.

// Implementation details:
// ReactionCtx is an API built around a ReactionWave. A single
// ReactionCtx may be used for multiple ReactionWaves, but
// obviously at disjoint times (&mut).
pub struct ReactionCtx<'a> {
    /// The reaction wave for the current tag.
    wave: &'a mut ReactionWave,

    /// Remaining reactions to execute before the wave dies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in order into this
    /// queue.
    pub(in super) do_next: TagExecutionPlan,

    /// Whether some reaction has called [Self::request_stop].
    requested_stop: bool,
}

impl ReactionCtx<'_> {
    /// Returns the current value of a port at this logical time.
    /// If the value is absent, [Option::None] is returned.
    ///
    /// The value is copied out. See also [Self::use_ref] if this
    /// is to be avoided.
    #[inline]
    pub fn get<'a, T, I>(&self, port: I) -> Option<T>
        where T: Copy + 'a,
              I: Borrow<ReadablePort<'a, T>> {
        port.borrow().get()
    }

    /// Executes the provided closure on the value of the port,
    /// if it is present.
    ///
    /// The value is fetched by reference and not copied.
    #[inline]
    pub fn use_ref<'a, T, I, O>(&self, port: I, action: impl FnOnce(&T) -> O) -> Option<O>
        where I: Borrow<ReadablePort<'a, T>>,
              T: 'a {
        port.borrow().use_ref(action)
    }

    /// Sets the value of the given port.
    ///
    /// The change is visible at the same logical time, ie
    /// the value propagates immediately. This may hence
    /// schedule more reactions that should execute at the
    /// same logical time.
    #[inline]
    pub fn set<'a, T, W>(&mut self, mut port: W, value: T)
        where W: BorrowMut<WritablePort<'a, T>>,
              T: 'a {

        // TODO topology information & deduplication
        //  Eg for a diamond situation this will execute reactions several times...
        //  This is why I added a set to patch it
        port.borrow_mut().set_impl(value, |downstream| self.enqueue_now(downstream));
    }

    /// Get the value of an action at this logical time.
    ///
    /// The value is cloned out. The value may be absent,
    /// in which case [Option::None] is returned. This is
    /// the case if the action is not present ([Self::is_action_present]),
    /// or if no value was scheduled (see [Self::schedule_with_v]).
    #[inline]
    pub fn get_action<T: Clone>(&self, action: &LogicalAction<T>) -> Option<T> {
        action.get_value(self.get_logical_time())
    }

    /// Returns true if the given action was triggered at the
    /// current logical time.
    ///
    /// If so, then it may, but must not, present a value ([Self::get_action]).
    #[inline]
    pub fn is_action_present<T: Clone>(&self, action: &LogicalAction<T>) -> bool {
        action.is_present(self.get_logical_time())
    }

    /// Schedule an action to trigger at some point in the future.
    ///
    /// This is like [Self::schedule_with_v], where the value is [None].
    #[inline]
    pub fn schedule<T: Clone>(&mut self, action: &mut LogicalAction<T>, offset: Offset) {
        self.schedule_with_v(action, None, offset)
    }

    /// Schedule an action to trigger at some point in the future,
    ///
    /// The action will carry the given value at the time it
    /// is triggered, unless it is overwritten by another call
    /// to this method. The value can be cleared by using `None`
    /// as a value. Note that even if the value is absent, the
    /// *action* will still be present at the time it is triggered
    /// (see [Self::is_action_present]).
    ///
    /// The action will trigger after its own implicit time delay,
    /// plus an optional additional time delay (see [Offset]).
    #[inline]
    pub fn schedule_with_v<T: Clone>(&mut self, action: &mut LogicalAction<T>, value: Option<T>, offset: Offset) {
        self.schedule_impl(action, value, offset);
    }

    // private
    #[inline]
    fn schedule_impl<K, T: Clone>(&mut self, action: &mut Action<K, T>, value: Option<T>, offset: Offset) {
        let eta = action.make_eta(self.wave.logical_time, offset.to_duration());
        action.schedule_future_value(eta, value);
        self.enqueue_later(&action.downstream, eta);
    }

    // todo hide this better
    /// Reschedule a timer if need be. This is used by synthetic
    /// reactions that reschedule timers.
    #[doc(hidden)]
    #[inline]
    pub fn maybe_reschedule(&mut self, timer: &Timer) {
        if timer.is_periodic() {
            self.enqueue_later(&timer.downstream, self.wave.logical_time + timer.period);
        }
    }


    #[inline]
    pub(in crate) fn enqueue_later(&mut self, downstream: &ReactionSet, process_at: LogicalInstant) {
        self.wave.enqueue_later(&downstream, process_at);
    }

    #[inline]
    pub(in crate) fn enqueue_now(&mut self, downstream: &ReactionSet) {
        self.do_next.accept(downstream.clone());
    }

    /// Request a shutdown which will be acted upon at the
    /// next microstep. Before then, the current tag is
    /// processed until completion.
    #[inline]
    pub fn request_stop(&mut self) {
        self.requested_stop = true;
    }

    /// Returns the start time of the execution of this program.
    ///
    /// This is a logical instant with microstep zero.
    #[inline]
    pub fn get_start_time(&self) -> LogicalInstant {
        self.wave.initial_time
    }

    /// Returns the current physical time.
    ///
    /// Repeated invocation of this method may produce different
    /// values, although [PhysicalInstant] is monotonic. The
    /// physical time is necessarily greater than the logical time.
    #[inline]
    pub fn get_physical_time(&self) -> PhysicalInstant {
        PhysicalInstant::now()
    }

    /// Returns the current logical time.
    ///
    /// Logical time is frozen during the execution of a reaction.
    /// Repeated invocation of this method will always produce
    /// the same value.
    #[inline]
    pub fn get_logical_time(&self) -> LogicalInstant {
        self.wave.logical_time
    }

    /// Returns the amount of logical time elapsed since the
    /// start of the program.
    #[inline]
    pub fn get_elapsed_logical_time(&self) -> Duration {
        self.get_logical_time().instant - self.wave.initial_time.instant
    }

    /// Returns the amount of physical time elapsed since the
    /// start of the program.
    ///
    /// Since this uses [Self::get_physical_time], be aware that
    /// this function's result may change over time.
    #[inline]
    pub fn get_elapsed_physical_time(&self) -> Duration {
        self.get_physical_time() - self.wave.initial_time.instant
    }

    /// Returns a string representation of the given time.
    ///
    /// The string is nicer than just using Debug, because
    /// it is relative to the start time of the execution ([Self::get_start_time]).
    #[inline]
    pub fn display_tag(&self, tag: LogicalInstant) -> String {
        display_tag_impl(self.wave.initial_time, tag)
    }
}

/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside world.
#[derive(Clone)]
pub struct SchedulerLink {
    last_processed_logical_time: TimeCell,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<ScheduledEvent>,
}

impl SchedulerLink {
    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    pub fn schedule_physical<T: Clone>(&mut self, action: &mut PhysicalAction<T>, value: Option<T>, offset: Offset) {
        // we have to fetch the time at which the logical timeline is currently running,
        // this may be far behind the current physical time
        let time_in_logical_subsystem = self.last_processed_logical_time.lock().unwrap().get();
        let process_at = action.make_eta(time_in_logical_subsystem, offset.to_duration());
        action.schedule_future_value(process_at, value);

        // todo merge events at equal tags by merging their dependencies
        let evt = Event { todo: action.downstream.clone() };
        self.sender.send(ScheduledEvent(evt, process_at)).unwrap();
    }
}


/// A "wave" of reactions executing at the same logical time.
/// Waves can enqueue new reactions to execute at the same time,
/// they're processed in exec order.
///
/// todo would there be a way to "split" waves into workers?
pub(in super) struct ReactionWave {
    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    pub logical_time: LogicalInstant,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<ScheduledEvent>,

    /// Start time of the program.
    initial_time: LogicalInstant,
}

impl ReactionWave {
    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    pub fn new(sender: Sender<ScheduledEvent>,
               current_time: LogicalInstant,
               initial_time: LogicalInstant) -> ReactionWave {
        ReactionWave {
            logical_time: current_time,
            sender,
            initial_time,
        }
    }

    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    #[inline]
    pub fn enqueue_later(&mut self, downstream: &ReactionSet, process_at: LogicalInstant) {
        debug_assert!(process_at > self.logical_time);

        // todo merge events at equal tags by merging their dependencies
        let evt = Event { todo: downstream.clone() };
        self.sender.send(ScheduledEvent(evt, process_at)).unwrap();
    }

    #[inline]
    pub fn new_ctx(&mut self) -> ReactionCtx {
        ReactionCtx {
            do_next: TagExecutionPlan::new_empty(self.logical_time),
            wave: self,
            requested_stop: false,
        }
    }

    /// Execute the wave until completion.
    /// The parameter is the list of reactions to start with.
    /// Todo topological info to split into independent subgraphs
    ///
    /// Returns whether some reaction called [ReactionCtx#request_stop]
    /// or not.
    pub fn consume(mut self, scheduler: &mut SyncScheduler, mut todo: TagExecutionPlan) -> WaveResult {
        // We can share it, to reuse the allocation of the do_next buffer
        // reactions that have already been processed.
        // In some situations (diamonds) this is necessary.
        // Possibly with more static information we can avoid that.
        let mut done: HashSet<GlobalReactionId> = HashSet::new();
        let mut requested_stop = false;
        let mut ctx = self.new_ctx();

        while !todo.is_empty() {
            for Batch(reactor_id, local_ids) in todo.drain() {
                let reactor = scheduler.get_reactor_mut(reactor_id);
                for reaction_id in local_ids.iter() {
                    let global = GlobalReactionId::new(reactor_id, reaction_id);
                    if done.insert(global) {
                        trace!("  - Executing {}", global);
                        // this may append new elements into the queue,
                        // which is why we can't use an iterator
                        reactor.react_erased(&mut ctx, reaction_id);
                        requested_stop |= ctx.requested_stop;
                    }
                }
            }

            // doing this lets us reuse the allocations of these vectors
            std::mem::swap(&mut ctx.do_next, &mut todo);
        }

        if requested_stop {
            WaveResult::StopRequested
        } else {
            WaveResult::Continue
        }
    }
}

pub(in super) enum WaveResult {
    Continue,
    StopRequested,
}

/// The offset from the current logical time after which an
/// action is triggered.
///
/// This is to be used with [ReactionCtx.schedule].
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Offset {
    /// Will be scheduled as soon as possible. This does not
    /// mean that the action will trigger right away. The
    /// action's inherent minimum delay must be taken into account,
    /// and even with a zero minimal delay, a delay of one microstep
    /// is applied.
    Asap,

    /// Will be scheduled at least after the provided duration.
    After(Duration),
}

impl Offset {
    #[inline]
    pub(in crate) fn to_duration(&self) -> Duration {
        match self {
            Offset::Asap => Duration::from_millis(0),
            Offset::After(d) => d.clone()
        }
    }
}


/// Cleans up a tag
// #[doc(hidden)]
pub struct CleanupCtx {
    /// Tag we're cleaning up
    pub tag: LogicalInstant,
}

impl CleanupCtx {
    pub fn cleanup_port<T>(&self, port: &mut Port<T>) {
        port.clear_value()
    }
    pub fn cleanup_action<T: Clone>(&self, action: &mut LogicalAction<T>) {
        action.forget_value(self.tag)
    }
}
