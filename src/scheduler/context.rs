use std::collections::HashSet;
use std::sync::mpsc::Sender;

use crate::*;
use super::*;

/// The context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
/// Only the interactions declared at assembly time
/// are allowed.
///
/// LogicalCtx is an API built around a ReactionWave. A single
/// LogicalCtx may be used for multiple ReactionWaves, but
/// obviously at disjoint times (&mut).
///
pub struct LogicalCtx<'a> {
    wave: &'a mut ReactionWave,

    /// Remaining reactions to execute before the wave dies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in order into this
    /// queue.
    pub(in super) do_next: TagExecutionPlan,

    requested_stop: bool,
}

impl LogicalCtx<'_> {
    /// Get the current value of a port at this time.
    #[inline]
    pub fn get<T: Copy>(&self, port: &InputPort<T>) -> Option<T> {
        port.get()
    }

    /// Execute the provided closure on the value of the port,
    /// if it is present. The value is fetched by reference and
    /// not copied.
    #[inline]
    pub fn use_ref<T, F, O>(&self, port: &InputPort<T>, action: F) -> Option<O> where F: FnOnce(&T) -> O {
        port.use_ref(action)
    }

    /// Get the value of an action at this time.
    #[inline]
    pub fn get_action<T: Clone>(&self, action: &LogicalAction<T>) -> Option<T> {
        action.get_value(self.get_logical_time())
    }

    #[inline]
    pub fn is_action_present<T: Clone>(&self, action: &LogicalAction<T>) -> bool {
        action.is_present(self.get_logical_time())
    }

    /// Sets the value of the given output port. The change
    /// is visible at the same logical time, ie the value
    /// propagates immediately. This may hence schedule more
    /// reactions that should execute on the same logical
    /// step.
    #[inline]
    pub fn set<T>(&mut self, port: &mut OutputPort<T>, value: T) {
        // TODO topology information & deduplication
        //  Eg for a diamond situation this will execute reactions several times...
        //  This is why I added a set to patch it
        port.set_impl(value, |downstream| self.enqueue_now(downstream));
    }

    /// Schedule an action to run after its own implicit time delay,
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    #[inline]
    pub fn schedule<T: Clone>(&mut self, action: &LogicalAction<T>, offset: Offset) {
        self.schedule_with_v(action, None, offset)
    }

    #[inline]
    pub fn schedule_with_v<T: Clone>(&mut self, action: &LogicalAction<T>, value: Option<T>, offset: Offset) {
        self.schedule_impl(action, value, offset);
    }

    // todo hide this better
    #[doc(hidden)]
    #[inline]
    pub fn maybe_reschedule(&mut self, timer: &Timer) {
        if timer.is_periodic() {
            self.enqueue_later(&timer.downstream, self.wave.logical_time + timer.period);
        }
    }

    // private
    #[inline]
    fn schedule_impl<K, T: Clone>(&mut self, action: &Action<K, T>, value: Option<T>, offset: Offset) {
        let eta = action.make_eta(self.wave.logical_time, offset.to_duration());
        action.schedule_future_value(eta, value);
        self.enqueue_later(&action.downstream, eta);
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

    #[inline]
    pub fn get_physical_time(&self) -> PhysicalInstant {
        PhysicalInstant::now()
    }

    #[inline]
    pub fn get_logical_time(&self) -> LogicalInstant {
        self.wave.logical_time
    }

    #[inline]
    pub fn get_elapsed_logical_time(&self) -> Duration {
        self.get_logical_time().instant - self.wave.initial_time.instant
    }

    #[inline]
    pub fn get_elapsed_physical_time(&self) -> Duration {
        self.get_physical_time() - self.wave.initial_time.instant
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
    pub fn schedule_physical<T: Clone>(&mut self, action: &PhysicalAction<T>, value: Option<T>, offset: Offset) {
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
    pub fn new_ctx(&mut self) -> LogicalCtx {
        LogicalCtx {
            do_next: TagExecutionPlan::new_empty(self.logical_time),
            wave: self,
            requested_stop: false,
        }
    }

    /// Execute the wave until completion.
    /// The parameter is the list of reactions to start with.
    /// Todo topological info to split into independent subgraphs
    ///
    /// Returns whether some reaction called [LogicalCtx#request_stop]
    /// or not.
    pub fn consume(mut self, scheduler: &mut SyncScheduler, mut todo: TagExecutionPlan) -> WaveResult {
        // We can share it, to reuse the allocation of the do_next buffer
        // reactions that have already been processed.
        // In some situations (diamonds) this is necessary.
        // Possibly with more static information we can avoid that.
        let mut done: HashSet<GlobalReactionId> = HashSet::new();
        let mut requested_stop = false;

        while !todo.is_empty() {
            let mut ctx = self.new_ctx();
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

            todo = ctx.do_next;
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
