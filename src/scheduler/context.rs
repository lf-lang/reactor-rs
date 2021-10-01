use std::borrow::{Borrow, BorrowMut};
use std::cmp::max;
use std::collections::HashSet;
use std::sync::mpsc::{Sender, SendError};

use crossbeam::thread::{Scope, ScopedJoinHandle};

use crate::*;
use crate::scheduler::depgraph::{DataflowInfo, ExecutableReactions};

use super::*;
use rayon::prelude::*;

/// The context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
/// Only the interactions declared at assembly time
/// are allowed.

// Implementation details:
// ReactionCtx is an API built around a ReactionWave. A single
// ReactionCtx may be used for multiple ReactionWaves, but
// obviously at disjoint times (&mut).
pub struct ReactionCtx<'a, 'x, 't> where 'x: 't {

    /// Remaining reactions to execute before the wave dies.
    /// Using [Option] and [Cow] optimises for the case where
    /// zero or exactly one port/action is set, and minimises
    /// copies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in into this
    /// data structure.
    todo: Option<Cow<'x, ExecutableReactions>>,

    /// Whether some reaction has called [Self::request_stop].
    requested_stop: bool,

    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    current_time: LogicalInstant,

    /// Sender to schedule events that should be executed later than this wave.
    tx: Sender<Event<'x>>,

    /// Start time of the program.
    initial_time: LogicalInstant,

    // globals
    thread_spawner: &'a Scope<'t>,
    dataflow: &'x DataflowInfo,
    latest_processed_tag: &'x TimeCell,
}



impl<'a, 'x, 't> ReactionCtx<'a, 'x, 't> where 'x: 't {
    pub(in super) fn new(tx: Sender<Event<'x>>,
                         current_time: LogicalInstant,
                         initial_time: LogicalInstant,
                         todo: Option<Cow<'x, ExecutableReactions>>,
                         dataflow: &'x DataflowInfo,
                         latest_processed_tag: &'x TimeCell,
                         thread_spawner: &'a Scope<'t>) -> Self {
        Self {
            todo,
            requested_stop: false,
            current_time,
            tx,
            initial_time,
            dataflow,
            latest_processed_tag,
            thread_spawner,
        }
    }


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
    /// # let port: &ReadablePort<'_, u32> = panic!();
    /// if let Some(value) = ctx.get(port) {
    ///     // branch is taken if the port is set -- note, this moves `port`!
    /// }
    /// ```
    #[inline]
    pub fn get<T: Copy>(&self, container: &impl ReactionTrigger<T>) -> Option<T> {
        container.borrow().get_value(&self.get_logical_time(), &self.get_start_time())
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
    pub fn use_ref<T, O>(&self, container: &impl ReactionTrigger<T>, action: impl FnOnce(Option<&T>) -> O) -> O {
        container.borrow().use_value_ref(&self.get_logical_time(), &self.get_start_time(), action)
    }

    /// Executes the provided closure on the value of the port,
    /// only if it is present. The value is fetched by reference
    /// and not copied.
    ///
    /// See also the similar [Self::use_ref].
    pub fn use_ref_opt<T, O>(&self, container: &impl ReactionTrigger<T>, action: impl FnOnce(&T) -> O) -> Option<O> {
        self.use_ref(container, |c| c.map(action))
    }

    /// Sets the value of the given port.
    ///
    /// The change is visible at the same logical time, i.e.
    /// the value propagates immediately. This may hence
    /// schedule more reactions that should execute at the
    /// same logical time.
    #[inline]
    pub fn set<'b, T, W>(&mut self, mut port: W, value: T)
        where T: Send + 'b,
              W: BorrowMut<WritablePort<'b, T>> {
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
        action.is_present(&self.get_logical_time(), &self.get_start_time())
    }

    /// Schedule an action to trigger at some point in the future.
    /// The action will trigger after its own implicit time delay,
    /// plus an optional additional time delay (see [Offset]).
    ///
    /// This is like [Self::schedule_with_v], where the value is [None].
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, LogicalAction, Offset::*};
    /// use std::time::Duration;
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let action: &mut LogicalAction<String> = panic!();
    /// ctx.schedule(action, Asap); // will be executed one microstep from now (+ own delay)
    /// ctx.schedule(action, AfterMillis(2)); // will be executed 2 milliseconds from now (+ own delay)
    /// ctx.schedule(action, After(Duration::from_nanos(120)));
    /// ```
    #[inline]
    pub fn schedule<T: Send>(&mut self, action: &mut LogicalAction<T>, offset: Offset) {
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
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, LogicalAction, Offset::*};
    /// use std::time::Duration;
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let action: &mut LogicalAction<&'static str> = panic!();
    /// // will be executed 2 milliseconds (+ own delay) from now with that value.
    /// ctx.schedule_with_v(action, Some("value"), AfterMillis(2));
    /// // will be executed one microstep from now, with no value
    /// ctx.schedule_with_v(action, None, Asap);
    /// // that's equivalent to
    /// ctx.schedule(action, Asap);
    /// ```
    #[inline]
    pub fn schedule_with_v<T: Send>(&mut self, action: &mut LogicalAction<T>, value: Option<T>, offset: Offset) {
        self.schedule_impl(action, value, offset);
    }

    #[inline]
    fn schedule_impl<K, T: Send>(&mut self, action: &mut Action<K, T>, value: Option<T>, offset: Offset) {
        let eta = action.make_eta(self.get_logical_time(), offset.to_duration());
        action.schedule_future_value(eta, value);
        let downstream = self.dataflow.reactions_triggered_by(&action.get_id());
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
            let downstream = self.dataflow.reactions_triggered_by(&timer.get_id());
            self.enqueue_later(downstream, self.get_logical_time() + timer.period);
        }
    }


    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    #[inline]
    pub(in crate) fn enqueue_later(&mut self, downstream: &'x ExecutableReactions, process_at: LogicalInstant) {
        debug_assert!(process_at > self.get_logical_time());

        // todo merge events at equal tags by merging their dependencies
        let evt = Event {
            reactions: Cow::Borrowed(downstream),
            tag: process_at,
        };
        // fixme we have mut access to scheduler, we should be able to avoid using a sender...
        self.tx.send(evt).unwrap();
    }

    #[inline]
    pub(in crate) fn enqueue_now(&mut self, downstream: Cow<'x, ExecutableReactions>) {
        match &mut self.todo {
            Some(ref mut do_next) => do_next.to_mut().absorb(downstream.as_ref()),
            None => self.todo = Some(downstream)
        }
    }

    pub(in crate) fn make_executable(&self, reactions: &Vec<GlobalReactionId>) -> ExecutableReactions {
        let mut result = ExecutableReactions::new();
        for r in reactions {
            self.dataflow.augment(&mut result, *r)
        }
        result
    }

    pub(in crate) fn reactions_triggered_by(&self, trigger: TriggerId) -> &'x ExecutableReactions {
        self.dataflow.reactions_triggered_by(&trigger)
    }

    /// Spawn a new thread that can use a [PhysicalSchedulerLink]
    /// to push asynchronous events to the reaction queue. This is
    /// only useful with [physical actions](crate::PhysicalAction).
    ///
    /// Since the thread is allowed to keep references into the
    /// internals of the scheduler, it is joined when the scheduler
    /// shuts down.
    /// todo clarify: will scheduler wait for joining,
    ///  possibly indefinitely? will thread be terminated?
    ///
    /// ### Example
    ///
    /// ```no_run
    /// # use reactor_rt::*;
    /// fn some_reaction(ctx: &mut ReactionCtx, phys_action: &PhysicalActionRef<u32>) {
    ///     let phys_action = phys_action.clone(); // clone to move it into other thread
    ///     ctx.spawn_physical_thread(move |link| {
    ///         std::thread::sleep(Duration::from_millis(200));
    ///         // This will push an event whose tag is the
    ///         // current physical time at the point of this
    ///         // statement.
    ///         link.schedule_physical_with_v(&phys_action, Some(123), Offset::Asap).unwrap();
    ///     });
    /// }
    /// ```
    ///
    pub fn spawn_physical_thread<F, R>(&mut self, f: F) -> ScopedJoinHandle<R>
        where F: FnOnce(&mut PhysicalSchedulerLink<'_, 'x, 't>) -> R,
              F: 'x + Send,
              R: 'x + Send {
        let tx = self.tx.clone();
        let latest_processed_tag = self.latest_processed_tag;
        let dataflow = self.dataflow;

        self.thread_spawner.spawn(move |subscope| {
            let mut link = PhysicalSchedulerLink {
                latest_processed_tag,
                tx,
                dataflow,
                thread_spawner: subscope,
            };
            f(&mut link)
        })
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
        self.initial_time
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
        self.current_time
    }

    /// Returns the amount of logical time elapsed since the
    /// start of the program. This does not take microsteps
    /// into account.
    #[inline]
    pub fn get_elapsed_logical_time(&self) -> Duration {
        self.get_logical_time().instant - self.get_start_time().instant
    }

    /// Returns the amount of physical time elapsed since the
    /// start of the program.
    ///
    /// Since this uses [Self::get_physical_time], be aware that
    /// this function's result may change over time.
    #[inline]
    pub fn get_elapsed_physical_time(&self) -> Duration {
        self.get_physical_time() - self.get_start_time().instant
    }

    /// Returns a string representation of the given time.
    ///
    /// The string is nicer than just using Debug, because
    /// it is relative to the start time of the execution ([Self::get_start_time]).
    #[inline]
    pub fn display_tag(&self, tag: LogicalInstant) -> String {
        display_tag_impl(self.initial_time, tag)
    }

    /// Asserts that the current tag is equals to the tag
    /// `(T0 + duration_since_t0, microstep)`. Panics if
    /// that is not the case.
    #[cfg(feature = "test-utils")]
    pub fn assert_tag_eq(&self, tag_spec: TagSpec) {
        let expected_tag = tag_spec.to_tag(self.get_start_time());

        if expected_tag != self.get_logical_time() {
            panic!("Expected tag to be {}, but found {}",
                   self.display_tag(expected_tag),
                   self.display_tag(self.get_logical_time()))
        }
    }

    fn fork(&self) -> Self {
        Self {
            todo: None,
            requested_stop: self.requested_stop,
            current_time: self.current_time,
            tx: self.tx.clone(),
            initial_time: self.initial_time,
            thread_spawner: self.thread_spawner,
            dataflow: self.dataflow,
            latest_processed_tag: self.latest_processed_tag,
        }
    }

    /// Execute the wave until completion.
    /// The parameter is the list of reactions to start with.
    pub(in super) fn process_entire_tag<'r>(
        mut self,
        scheduler: &mut SyncScheduler<'_, 'x, '_>,
        reactors: &mut ReactorVec<'r>,
    ) {

        // set of reactions that have been executed
        let mut executed: HashSet<GlobalReactionId> = HashSet::new();
        // The maximum layer number we've seen as of now.
        // This must be increasing monotonically.
        let mut max_layer = 0usize;

        let mut requested_stop = false;
        loop {
            let mut progress = false;
            match self.todo.take() {
                None => {
                    // nothing to do
                    break;
                }
                Some(todo) => {
                    for (layer_no, batch) in todo.batches() {
                        // none of the reactions in the batch have data dependencies
                        progress = true;

                        for reaction_id in batch {
                            // todo parallelize this loop. Each task needs
                            //  - a ref mut into the reactors vec -> we need unsafe
                            //  - a cloned Sender
                            //  - a separate ReactionCtx -> then we need to merge all parallel ctx.do_next
                            //  - ....maybe we need ReactionWave back, so that making a new ReactionCtx is cheaper...
                            trace!("  - Executing {}", scheduler.display_reaction(*reaction_id));
                            let reactor = &mut reactors[reaction_id.0.container()];

                            // this may append new elements into the queue,
                            // which is why we can't use an iterator
                            reactor.react_erased(&mut self, reaction_id.0.local());
                            requested_stop |= self.requested_stop;

                            debug_assert!(executed.insert(*reaction_id), "Duplicate reaction");
                        }

                        if cfg!(debug_assertions) {
                            debug_assert!(layer_no >= max_layer, "Reaction dependencies were not respected {} < {}", layer_no, max_layer);
                            max_layer = max(max_layer, layer_no);
                        }
                    }
                }
            }

            if !progress {
                // no new batch, we're done
                break;
            }
        }

        if requested_stop {
            scheduler.request_stop(self.get_logical_time().next_microstep());
        }

        // cleanup tag-specific resources, eg clear port values
        let ctx = CleanupCtx { tag: self.get_logical_time() };
        // TODO measure performance of cleaning up all reactors w/ virtual dispatch like this.
        for reactor in reactors {
            reactor.cleanup_tag(&ctx)
        }
    }
}


/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside world.
///
/// todo this doesn't have capacity to call request_stop
///
/// See [ReactionCtx::spawn_physical_thread].
#[derive(Clone)]
pub struct PhysicalSchedulerLink<'a, 'x, 't> {
    latest_processed_tag: &'x TimeCell,
    tx: Sender<Event<'x>>,
    dataflow: &'x DataflowInfo,
    thread_spawner: &'a Scope<'t>,
}

impl PhysicalSchedulerLink<'_, '_, '_> {
    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// This may fail if this is called while the scheduler has already
    /// been shutdown. todo prevent this
    pub fn schedule_physical<T: Send>(&mut self, action: &PhysicalActionRef<T>, offset: Offset) -> Result<(), SendError<Option<T>>> {
        self.schedule_physical_with_v(action, None, offset)
    }

    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// This may fail if this is called while the scheduler has already
    /// been shutdown. todo prevent this
    pub fn schedule_physical_with_v<T: Send>(
        &mut self,
        action: &PhysicalActionRef<T>,
        value: Option<T>,
        offset: Offset,
    ) -> Result<(), SendError<Option<T>>> {

        // we have to fetch the time at which the logical timeline is currently running,
        // this may be far behind the current physical time
        let time_in_logical_subsystem = self.latest_processed_tag.load();
        action.use_mut(|action| {
            let tag = action.make_eta(time_in_logical_subsystem, offset.to_duration());
            action.schedule_future_value(tag, value);

            let downstream = self.dataflow.reactions_triggered_by(&action.get_id());
            let evt = Event { reactions: Cow::Borrowed(downstream), tag };
            self.tx.send(evt).map_err(|e| {
                warn!("Event could not be sent! {:?}", e);
                SendError(action.forget_value(&tag))
            })
        })
    }
}


/// See [ReactionCtx::assert_tag_eq]
#[cfg(feature = "test-utils")]
#[derive(Debug, Copy, Clone)]
pub enum TagSpec {
    T0,
    Milli(u64),
    MilliStep(u64, crate::time::MS),
    Tag(Duration, crate::time::MS),
}

#[cfg(feature = "test-utils")]
impl TagSpec {
    fn to_tag(self, t0: LogicalInstant) -> LogicalInstant {
        match self {
            TagSpec::T0 => t0,
            TagSpec::Milli(ms) => LogicalInstant {
                instant: t0.instant + Duration::from_millis(ms),
                microstep: MicroStep::ZERO,
            },
            TagSpec::MilliStep(ms, step) => LogicalInstant {
                instant: t0.instant + Duration::from_millis(ms),
                microstep: MicroStep::new(step),
            },
            TagSpec::Tag(offset, step) => LogicalInstant {
                instant: t0.instant + offset,
                microstep: MicroStep::new(step),
            }
        }
    }
}

/// The offset from the current logical time after which an
/// action is triggered.
///
/// This is to be used with [ReactionCtx::schedule].
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Offset {
    /// Will be scheduled at least after the provided duration.
    /// The other variants are just shorthands for common use-cases.
    After(Duration),

    /// Will be scheduled as soon as possible. This does not
    /// mean that the action will trigger right away. The
    /// action's inherent minimum delay must be taken into account,
    /// and even with a zero minimal delay, a delay of one microstep
    /// is applied. This is equivalent to
    /// ```no_compile
    /// # use std::time::Duration;
    /// After(Duration::ZERO)
    /// ```
    Asap,

    /// Will be scheduled at least after the provided duration,
    /// which is given in seconds. This is equivalent
    /// to
    /// ```no_compile
    /// # use std::time::Duration;
    /// After(Duration::from_secs(_))
    /// ```
    AfterSeconds(u64),

    /// Will be scheduled at least after the provided duration,
    /// which is given in milliseconds (ms). This is equivalent
    /// to
    /// ```no_compile
    /// # use std::time::Duration;
    /// After(Duration::from_millis(_))
    /// ```
    AfterMillis(u64),

    /// Will be scheduled at least after the provided duration,
    /// which is given in microseconds (µs). This is equivalent
    /// to
    /// ```no_compile
    /// # use std::time::Duration;
    /// After(Duration::from_micros(_))
    /// ```
    AfterMicros(u64),

    /// Will be scheduled at least after the provided duration,
    /// which is given in microseconds (µs). This is equivalent
    /// to
    /// ```no_compile
    /// # use std::time::Duration;
    /// After(Duration::from_nanos(_))
    /// ```
    AfterNanos(u64),

}

impl Offset {
    #[inline]
    pub(in crate) fn to_duration(&self) -> Duration {
        match self {
            Offset::After(d) => d.clone(),
            Offset::Asap => Duration::from_millis(0),
            Offset::AfterSeconds(s) => Duration::from_secs(*s),
            Offset::AfterMillis(ms) => Duration::from_millis(*ms),
            Offset::AfterMicros(us) => Duration::from_micros(*us),
            Offset::AfterNanos(us) => Duration::from_nanos(*us),
        }
    }
}


/// Cleans up a tag
#[doc(hidden)]
pub struct CleanupCtx {
    /// Tag we're cleaning up
    pub tag: LogicalInstant,
}

impl CleanupCtx {
    pub fn cleanup_port<T: Send>(&self, port: &mut Port<T>) {
        port.clear_value()
    }

    pub fn cleanup_logical_action<T: Send>(&self, action: &mut LogicalAction<T>) {
        action.forget_value(&self.tag);
    }

    pub fn cleanup_physical_action<T: Send>(&self, action: &mut PhysicalActionRef<T>) {
        action.use_mut(|a| a.forget_value(&self.tag));
    }
}
