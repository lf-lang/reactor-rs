use std::borrow::{Borrow, BorrowMut};
use std::cmp::max;
use std::collections::HashSet;
use std::sync::mpsc::Sender;

use crate::*;
use crate::scheduler::depgraph::{DataflowInfo, ExecutableReactions};

use super::*;

/// The context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
/// Only the interactions declared at assembly time
/// are allowed.

// Implementation details:
// ReactionCtx is an API built around a ReactionWave. A single
// ReactionCtx may be used for multiple ReactionWaves, but
// obviously at disjoint times (&mut).
pub struct ReactionCtx<'a, 'x> {
    /// The reaction wave for the current tag.
    wave: &'a mut ReactionWave<'x>,

    /// Remaining reactions to execute before the wave dies.
    /// Using [Option] and [Cow] optimises for the case where
    /// zero or exactly one port/action is set, and minimises
    /// copies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in into this
    /// data structure.
    pub(in super) do_next: Option<Cow<'x, ExecutableReactions>>,

    /// Whether some reaction has called [Self::request_stop].
    requested_stop: bool,
}

impl<'x> ReactionCtx<'_, 'x> {

    /// Returns the current value of a port or action at this
    /// logical time. If the value is absent, [Option::None] is
    /// returned.  This is the case if the action or port is
    /// not present ([Self::is_present]), or if no value was
    /// scheduled (action values are optional, see [Self::schedule_with_v]).
    ///
    /// The value is copied out. See also [Self::use_ref] if this
    /// is to be avoided.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, ReadablePort};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: ReadablePort<'_, u32> = panic!();
    /// if let Some(value) = ctx.get(port) {
    ///     // branch is taken if the port is set -- note, this moves `port`!
    /// }
    /// ```
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, ReadablePort};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: ReadablePort<'_, u32> = panic!();
    ///
    /// let value_opt = ctx.get(&port); // you can pass the port by reference
    /// let value_opt = ctx.get(port); // or by value (but this moves it out)
    /// ```
    ///
    #[inline]
    pub fn get<T, C>(&self, container: C) -> Option<T>
        where T: Copy,
              C: ReactionTrigger<T> {
        container.borrow().get_value(&self.get_logical_time())
    }

    /// Executes the provided closure on the value of the port
    /// or action. The value is fetched by reference and not
    /// copied.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, ReadablePort};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: &ReadablePort<String> = panic!();
    /// let len = ctx.use_ref(port, |str| str.map(String::len).unwrap_or(0));
    /// // equivalent to
    /// let len = ctx.use_ref_opt(port, String::len).unwrap_or(0);
    /// ```
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, ReadablePort};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: &ReadablePort<String> = panic!();
    ///
    /// if let Some(str) = ctx.use_ref_opt(port, Clone::clone) {
    ///     // only entered if the port value is present, so no need to check is_present
    /// }
    /// ```
    ///
    /// See also the similar [Self::use_ref_opt].
    #[inline]
    pub fn use_ref<C, T, O>(&self, container: C, action: impl FnOnce(Option<&T>) -> O) -> O
        where C: ReactionTrigger<T> {
        container.borrow().use_value_ref(&self.get_logical_time(), action)
    }

    /// Executes the provided closure on the value of the port,
    /// only if it is present. The value is fetched by reference
    /// and not copied.
    ///
    /// See also the similar [Self::use_ref].
    pub fn use_ref_opt<C, T, O>(&self, container: C, action: impl FnOnce(&T) -> O) -> Option<O>
        where C: ReactionTrigger<T> {
        self.use_ref(container, |c| c.map(action))
    }

    /// Sets the value of the given port.
    ///
    /// The change is visible at the same logical time, i.e.
    /// the value propagates immediately. This may hence
    /// schedule more reactions that should execute at the
    /// same logical time.
    #[inline]
    pub fn set<'a, T, W>(&mut self, mut port: W, value: T)
        where T: 'a,
              W: BorrowMut<WritablePort<'a, T>> {
        let port = port.borrow_mut();
        port.set_impl(value);
        self.enqueue_now(Cow::Borrowed(self.reactions_triggered_by(port.get_id())))
    }

    /// Returns true if the given action was triggered at the
    /// current logical time.
    ///
    /// If so, then it may, but must not, present a value ([Self::get]).
    #[inline]
    pub fn is_present<T>(&self, action: &impl ReactionTrigger<T>) -> bool {
        action.is_present(&self.get_logical_time())
    }

    /// Schedule an action to trigger at some point in the future.
    ///
    /// This is like [Self::schedule_with_v], where the value is [None].
    #[inline]
    pub fn schedule<T>(&mut self, action: &mut LogicalAction<T>, offset: Offset) {
        self.schedule_with_v(action, None, offset)
    }

    /// Schedule an action to trigger at some point in the future,
    ///
    /// The action will carry the given value at the time it
    /// is triggered, unless it is overwritten by another call
    /// to this method. The value can be cleared by using `None`
    /// as a value. Note that even if the value is absent, the
    /// *action* will still be present at the time it is triggered
    /// (see [Self::is_present]).
    ///
    /// The action will trigger after its own implicit time delay,
    /// plus an optional additional time delay (see [Offset]).
    #[inline]
    pub fn schedule_with_v<T>(&mut self, action: &mut LogicalAction<T>, value: Option<T>, offset: Offset) {
        self.schedule_impl(action, value, offset);
    }

    #[inline]
    fn schedule_impl<K, T>(&mut self, action: &mut Action<K, T>, value: Option<T>, offset: Offset) {
        let eta = action.make_eta(self.wave.logical_time, offset.to_duration());
        action.schedule_future_value(eta, value);
        let downstream = self.wave.dataflow.reactions_triggered_by(&action.get_id());
        self.enqueue_later(downstream, eta);
    }

    // todo hide this better: this would require synthesizing
    //  the reaction within the runtime and not with the code generator
    /// Reschedule a timer if need be. This is used by synthetic
    /// reactions that reschedule timers.
    #[doc(hidden)]
    #[inline]
    pub fn maybe_reschedule(&mut self, timer: &Timer) {
        if timer.is_periodic() {
            let downstream = self.wave.dataflow.reactions_triggered_by(&timer.get_id());
            self.enqueue_later(downstream, self.wave.logical_time + timer.period);
        }
    }


    #[inline]
    pub(in crate) fn enqueue_later(&mut self, downstream: &'x ExecutableReactions, process_at: LogicalInstant) {
        self.wave.enqueue_later(&downstream, process_at);
    }

    #[inline]
    pub(in crate) fn enqueue_now(&mut self, downstream: Cow<'x, ExecutableReactions>) {
        match &mut self.do_next {
            Some(ref mut do_next) => self.wave.dataflow.merge(do_next.to_mut(), downstream.as_ref()),
            None => {
                self.do_next = Some(downstream);
            }
        }
    }

    pub(in crate) fn make_executable(&self, reactions: &Vec<GlobalReactionId>) -> ExecutableReactions {
        let mut result = ExecutableReactions::new();
        for r in reactions {
            self.wave.dataflow.augment(&mut result, *r)
        }
        result
    }

    pub(in crate) fn reactions_triggered_by(&self, trigger: TriggerId) -> &'x ExecutableReactions {
        self.wave.dataflow.reactions_triggered_by(&trigger)
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
    /// start of the program. This does not take microsteps
    /// into account.
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

    /// Asserts that the current tag is equals to the tag
    /// `(T0 + duration_since_t0, microstep)`. Panics if
    /// that is not the case.
    pub fn assert_tag_eq(&self,
                         duration_since_t0: Duration,
                         microstep: crate::time::MS) {
        let expected_tag = LogicalInstant {
            instant: self.get_start_time().instant + duration_since_t0,
            microstep: MicroStep::new(microstep),
        };

        if expected_tag != self.get_logical_time() {
            panic!("Expected tag to be {}, but found {}", self.display_tag(expected_tag), self.display_tag(self.get_logical_time()))
        }
    }
}

/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside world.
#[derive(Clone)]
pub struct SchedulerLink<'x> {
    last_processed_logical_time: TimeCell,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<Event<'x>>,

    dataflow: &'x DataflowInfo,
}

impl<'x> SchedulerLink<'x> {
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
        let downstream = self.dataflow.reactions_triggered_by(&action.get_id());
        let evt = Event::<'x> {
            reactions: Cow::Borrowed(downstream),
            tag: process_at,
        };
        self.sender.send(evt).unwrap();
    }
}


/// A "wave" of reactions executing at the same logical time.
/// Waves can enqueue new reactions to execute at the same time,
/// they're processed in exec order.
///
/// todo would there be a way to "split" waves into workers?
pub(in super) struct ReactionWave<'x> {
    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    pub logical_time: LogicalInstant,

    /// Sender to schedule events that should be executed later than this wave.
    sender: Sender<Event<'x>>,

    /// Start time of the program.
    initial_time: LogicalInstant,

    dataflow: &'x DataflowInfo,
}

impl<'x> ReactionWave<'x> {
    /// Create a new reaction wave to process the given
    /// reactions at some point in time.
    pub fn new(sender: Sender<Event<'x>>,
               current_time: LogicalInstant,
               initial_time: LogicalInstant,
               dataflow: &'x DataflowInfo) -> Self {
        ReactionWave {
            logical_time: current_time,
            sender,
            initial_time,
            dataflow,
        }
    }

    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    #[inline]
    pub fn enqueue_later(&mut self, downstream: &'x ExecutableReactions, process_at: LogicalInstant) {
        debug_assert!(process_at > self.logical_time);

        // todo merge events at equal tags by merging their dependencies
        let evt = Event {
            reactions: Cow::Borrowed(downstream),
            tag: process_at,
        };
        self.sender.send(evt).unwrap();
    }

    #[inline]
    pub fn new_ctx<'a>(&'a mut self) -> ReactionCtx<'a, 'x> {
        ReactionCtx {
            do_next: <_>::default(),
            wave: self,
            requested_stop: false,
        }
    }

    /// Execute the wave until completion.
    /// The parameter is the list of reactions to start with.
    ///
    /// Returns whether some reaction called [ReactionCtx#request_stop]
    /// or not.
    pub fn consume(mut self, scheduler: &mut SyncScheduler<'x>, mut todo: Cow<'x, ExecutableReactions>) -> WaveResult {

        // set of reactions that have been executed
        let mut executed: HashSet<GlobalReactionId> = HashSet::new();
        // The maximum layer number we've seen as of now.
        // This must be increasing monotonically.
        let mut max_layer = 0usize;

        let mut requested_stop = false;
        let mut ctx = self.new_ctx();
        loop {
            let mut progress = false;

            for (layer_no, reactions) in todo.batches() {
                progress = true;

                for reaction_id in reactions {
                    trace!("  - Executing {}", scheduler.display_reaction(*reaction_id));
                    let reactor = scheduler.get_reactor_mut(reaction_id.0.container());

                    // this may append new elements into the queue,
                    // which is why we can't use an iterator
                    reactor.react_erased(&mut ctx, reaction_id.0.local());
                    requested_stop |= ctx.requested_stop;

                    if cfg!(debug_assertions) {
                        assert!(executed.insert(*reaction_id), "Duplicate reaction");
                    }
                }


                if cfg!(debug_assertions) {
                    debug_assert!(layer_no >= max_layer, "Reaction dependencies were not respected {} < {}", layer_no, max_layer);
                    max_layer = max(max_layer, layer_no);
                }
            }

            if !progress {
                // no new batch, we're done
                break;
            }

            if let Some(cow) = ctx.do_next.take() {
                todo = cow;
            } else {
                // nothing more to do
                break;
            }
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
        action.forget_value(&self.tag)
    }
}
