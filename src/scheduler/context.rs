
use std::borrow::{Borrow, BorrowMut};
use std::cmp::max;
use std::sync::mpsc::{Sender, SendError};

use crossbeam_utils::thread::{Scope, ScopedJoinHandle};
use smallvec::SmallVec;

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
pub struct ReactionCtx<'a, 'x, 't>(RContextInner<'a, 'x, 't>) ;


impl<'a, 'x, 't> ReactionCtx<'a, 'x, 't> where 'x: 't {
    pub(in super) fn new(tx: Sender<Event<'x>>,
                         tag: EventTag,
                         initial_time: PhysicalInstant,
                         todo: Option<Cow<'x, ExecutableReactions>>,
                         dataflow: &'x DataflowInfo,
                         thread_spawner: &'a Scope<'t>) -> Self {
        Self(RContextInner {
            insides: RContextForwardableStuff {
                todo_now: todo,
                future_events: Default::default(),
            },
            tag,
            tx,
            initial_time,
            dataflow,
            thread_spawner,
        })
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
        container.borrow().get_value(&self.get_tag(), &self.get_start_time())
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
        container.borrow().use_value_ref(&self.get_tag(), &self.get_start_time(), action)
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
        action.is_present(&self.get_tag(), &self.get_start_time())
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
    /// # use reactor_rt::{Duration, ReactionCtx, LogicalAction, Offset::*, after, delay};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let action: &mut LogicalAction<String> = panic!();
    /// ctx.schedule(action, Asap);         // will be executed one microstep from now (+ own delay)
    /// ctx.schedule(action, after!(2 ms)); // will be executed 2 milliseconds from now (+ own delay)
    /// ctx.schedule(action, After(delay!(2 ms)));             // equivalent to the previous
    /// ctx.schedule(action, After(Duration::from_millis(2))); // equivalent to the previous
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
    /// # use reactor_rt::{Duration, ReactionCtx, LogicalAction, Offset::*, after, delay};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let action: &mut LogicalAction<&'static str> = panic!();
    /// // will be executed 2 milliseconds (+ own delay) from now with that value.
    /// ctx.schedule_with_v(action, Some("value"), after!(2 msec));
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
    fn schedule_impl<T: Send>(&mut self, action: &mut LogicalAction<T>, value: Option<T>, offset: Offset) {
        let eta = self.make_successor_tag(action.min_delay + offset.to_duration());
        action.schedule_future_value(eta, value);
        let downstream = self.0.dataflow.reactions_triggered_by(&action.get_id());
        self.enqueue_later(downstream, eta);
    }

    /// Reschedule a timer if need be. This is used by synthetic
    /// reactions that reschedule timers.
    // todo hide this better: this would require synthesizing
    //  the reaction within the runtime and not with the code generator
    #[doc(hidden)]
    #[inline]
    pub fn maybe_reschedule(&mut self, timer: &Timer) {
        if timer.is_periodic() {
            let downstream = self.0.dataflow.reactions_triggered_by(&timer.get_id());
            let tag = self.make_successor_tag(timer.period);
            self.enqueue_later(downstream, tag);
        }
    }


    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    #[inline]
    pub(in crate) fn enqueue_later(&mut self, downstream: &'x ExecutableReactions, tag: EventTag) {
        debug_assert!(tag > self.get_tag());

        let evt = Event {
            tag,
            payload: EventPayload::Reactions(Cow::Borrowed(downstream)),
        };
        self.0.insides.future_events.push(evt);
    }

    #[inline]
    pub(in crate) fn enqueue_now(&mut self, downstream: Cow<'x, ExecutableReactions>) {
        match &mut self.0.insides.todo_now {
            Some(ref mut do_next) => do_next.to_mut().absorb(downstream.as_ref()),
            None => self.0.insides.todo_now = Some(downstream)
        }
    }

    #[inline]
    pub(in crate) fn make_executable(&self, reactions: &ReactionSet) -> ExecutableReactions {
        reactions.iter().fold(
            ExecutableReactions::new(),
            |mut acc, r| {
                self.0.dataflow.augment(&mut acc, *r);
                acc
            })
    }

    fn reactions_triggered_by(&self, trigger: TriggerId) -> &'x ExecutableReactions {
        self.0.dataflow.reactions_triggered_by(&trigger)
    }

    fn make_successor_tag(&self, offset_from_now: Duration) -> EventTag {
        self.get_tag().successor(self.get_start_time(), offset_from_now)
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
        let tx = self.0.tx.clone();
        let dataflow = self.0.dataflow;
        let initial_time = self.0.initial_time;

        self.0.thread_spawner.spawn(move |subscope| {
            let mut link = PhysicalSchedulerLink {
                tx,
                dataflow,
                initial_time,
                thread_spawner: subscope,
            };
            f(&mut link)
        })
    }

    /// Request a shutdown which will be acted upon at the
    /// next microstep. Before then, the current tag is
    /// processed until completion.
    #[inline]
    pub fn request_stop(&mut self, offset: Offset) {
        let tag = self.make_successor_tag(offset.to_duration());

        let evt = Event { tag, payload: EventPayload::Terminate };
        self.0.insides.future_events.push(evt);
    }

    /// Returns the start time of the execution of this program.
    ///
    /// This is a logical instant with microstep zero.
    #[inline]
    pub fn get_start_time(&self) -> PhysicalInstant {
        self.0.initial_time
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
    pub fn get_logical_time(&self) -> Instant {
        self.0.tag.to_logical_time(self.get_start_time())
    }

    /// Returns the tag at which the reaction executes.
    ///
    /// Repeated invocation of this method will always produce
    /// the same value.
    #[inline]
    fn get_tag(&self) -> EventTag {
        self.0.tag
    }

    /// Returns the amount of logical time elapsed since the
    /// start of the program. This does not take microsteps
    /// into account.
    #[inline]
    pub fn get_elapsed_logical_time(&self) -> Duration {
        self.get_logical_time() - self.get_start_time()
    }

    /// Returns the amount of physical time elapsed since the
    /// start of the program.
    ///
    /// Since this uses [Self::get_physical_time], be aware that
    /// this function's result may change over time.
    #[inline]
    pub fn get_elapsed_physical_time(&self) -> Duration {
        self.get_physical_time() - self.get_start_time()
    }

    /// Returns a string representation of the given time.
    ///
    /// The string is nicer than just using Debug, because
    /// it is relative to the start time of the execution ([Self::get_start_time]).
    #[inline]
    pub fn display_tag(&self, tag: EventTag) -> String {
        display_tag_impl(self.0.initial_time, tag)
    }

    /// Asserts that the current tag is equals to the tag
    /// `(T0 + duration_since_t0, microstep)`. Panics if
    /// that is not the case.
    #[cfg(feature = "test-utils")]
    pub fn assert_tag_eq(&self, tag_spec: TagSpec) {
        let expected_tag = tag_spec.to_tag(self.get_start_time());

        if expected_tag != self.get_tag() {
            panic!("Expected tag to be {}, but found {}",
                   self.display_tag(expected_tag),
                   self.display_tag(self.get_tag()))
        }
    }


    /// Execute the wave until completion.
    /// The parameter is the list of reactions to start with.
    pub(in super) fn process_entire_tag(
        mut self,
        debug: DebugInfoProvider<'_>,
        reactors: &mut ReactorVec<'_>,
        mut push_future_event:  impl FnMut(Event<'x>),
    ) {

        // The maximum layer number we've seen as of now.
        // This must be increasing monotonically.
        let mut max_layer = 0usize;

        loop {
            let mut progress = false;
            match self.0.insides.todo_now.take() {
                None => {
                    // nothing to do
                    break;
                }
                Some(todo) => {
                    for (layer_no, batch) in todo.batches() {
                        // none of the reactions in the batch have data dependencies
                        progress = true;

                        if cfg!(feature = "parallel_runtime") {
                            #[cfg(feature = "parallel_runtime")]
                                parallel_rt_impl::process_batch(&mut self, &debug, reactors, batch);

                            for evt in self.0.insides.future_events.drain(..) {
                                push_future_event(evt)
                            }
                        } else {
                            // the impl for non-parallel runtime
                            for reaction_id in batch {
                                trace!("  - Executing {}", debug.display_reaction(*reaction_id));
                                let reactor = &mut reactors[reaction_id.0.container()];

                                reactor.react_erased(&mut self, reaction_id.0.local());
                                // the reaction invocation may have mutated self.0.insides:
                                // - todo_now: reactions that need to be executed next -> they're
                                // processed in the next loop iteration
                                // - future_events: handled now
                                for evt in self.0.insides.future_events.drain(..) {
                                    push_future_event(evt)
                                }
                            }
                        }

                        if cfg!(debug_assertions) {
                            debug_assert!(layer_no >= max_layer, "Reaction dependencies were not respected ({} < {})", layer_no, max_layer);
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

        // cleanup tag-specific resources, eg clear port values
        let ctx = CleanupCtx { tag: self.get_tag() };
        // TODO measure performance of cleaning up all reactors w/ virtual dispatch like this.
        for reactor in reactors {
            reactor.cleanup_tag(&ctx)
        }
    }
}

#[cfg(feature = "parallel_runtime")]
mod parallel_rt_impl {
    use std::collections::HashSet;

    use rayon::prelude::*;

    use super::*;

    pub(super) fn process_batch(
        ctx: &mut ReactionCtx<'_, '_, '_>,
        debug: &DebugInfoProvider<'_>,
        reactors: &mut ReactorVec<'_>,
        batch: &HashSet<GlobalReactionId>,
    ) {
        let reactors_mut = UnsafeSharedPointer(reactors.raw.as_mut_ptr());

        let final_result =
            batch.iter()
                .par_bridge()
                .fold_with(
                    CloneableCtx(ctx.0.fork()),
                    |CloneableCtx(ctx_inner), reaction_id| {
                        trace!("  - Executing {}", debug.display_reaction(*reaction_id));
                        let reactor = unsafe {
                            // safety:
                            // - no two reactions in the batch refer belong to the same reactor
                            // - the vec does not change size so there is no reallocation
                            &mut *reactors_mut.0.add(reaction_id.0.container().index())
                        };

                        // this may append new elements into the queue,
                        // which is why we can't use an iterator
                        let mut ctx = ReactionCtx(ctx_inner);
                        reactor.react_erased(&mut ctx, reaction_id.0.local());

                        CloneableCtx(ctx.0)
                    },
                )
                // .fold_with(ctx.0.fork(),
                //            |ctx_inner, reaction_id| {
                //            })
                .fold(|| RContextForwardableStuff::default(), |cx1, cx2| cx1.merge(cx2.0.insides))
                .reduce(|| Default::default(), RContextForwardableStuff::merge);

        ctx.0.insides = final_result;
    }


    struct UnsafeSharedPointer<T>(*mut T);

    unsafe impl<T> Send for UnsafeSharedPointer<T> {}

    unsafe impl<T> Sync for UnsafeSharedPointer<T> {}


    /// We need a Clone bound to use fold_with, but this clone
    /// implementation is not general purpose so I hide it.
    struct CloneableCtx<'a, 'x, 't>(RContextInner<'a, 'x, 't>);

    impl Clone for CloneableCtx<'_, '_, '_> {
        fn clone(&self) -> Self {
            Self(self.0.fork())
        }
    }
}

struct RContextInner<'a, 'x, 't> where 'x: 't {
    insides: RContextForwardableStuff<'x>,

    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    tag: EventTag,

    /// Sender to schedule events that should be executed later than this wave.
    tx: Sender<Event<'x>>,

    /// Start time of the program.
    initial_time: PhysicalInstant,

    // globals
    thread_spawner: &'a Scope<'t>,
    dataflow: &'x DataflowInfo,
}

impl<'x, 't> RContextInner<'_, 'x, 't> where 'x: 't {
    /// Fork a context. Some things are shared, but not the
    /// mutable stuff.
    #[cfg(feature = "parallel_runtime")]
    fn fork(&self) -> Self {
        Self {
            insides: Default::default(),

            // all of that is common to all contexts
            tag: self.tag,
            tx: self.tx.clone(),
            initial_time: self.initial_time,
            thread_spawner: self.thread_spawner,
            dataflow: self.dataflow,
        }
    }
}

/// Info that executing reactions need to make known to the scheduler.
struct RContextForwardableStuff<'x> {
    /// Remaining reactions to execute before the wave dies.
    /// Using [Option] and [Cow] optimises for the case where
    /// zero or exactly one port/action is set, and minimises
    /// copies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in into this
    /// data structure.
    todo_now: Option<Cow<'x, ExecutableReactions>>,

    /// Events that were produced for a strictly greater
    /// logical time than a current one.
    future_events: SmallVec<[Event<'x>; 4]>,
}

impl Default for RContextForwardableStuff<'_> {
    fn default() -> Self {
        Self {
            todo_now: None,
            future_events: Default::default(),
        }
    }
}

#[cfg(feature = "parallel_runtime")]
impl<'x> RContextForwardableStuff<'x> {
    fn merge(mut self, mut other: Self) -> Self {
        self.todo_now = Self::merge_cows(self.todo_now, other.todo_now);
        self.future_events.append(&mut other.future_events);
        self
    }

    fn merge_cows(x: Option<Cow<'x, ExecutableReactions>>,
                  y: Option<Cow<'x, ExecutableReactions>>) -> Option<Cow<'x, ExecutableReactions>> {
        match (x, y) {
            (None, None) => None,
            (Some(x), None) | (None, Some(x)) => Some(x),
            (Some(Cow::Owned(mut x)), Some(y)) | (Some(y), Some(Cow::Owned(mut x))) => {
                x.absorb(&y);
                Some(Cow::Owned(x))
            },
            (Some(mut x), Some(y)) => {
                x.to_mut().absorb(&y);
                Some(x)
            }
        }
    }
}

/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside world.
///
/// See [ReactionCtx::spawn_physical_thread].
#[derive(Clone)]
pub struct PhysicalSchedulerLink<'a, 'x, 't> {
    tx: Sender<Event<'x>>,
    initial_time: Instant,
    dataflow: &'x DataflowInfo,
    #[allow(unused)] // maybe add a spawn_physical_thread to this type
    thread_spawner: &'a Scope<'t>,
}

impl PhysicalSchedulerLink<'_, '_, '_> {
    /// Request that the application shutdown, possibly with
    /// a particular offset.
    ///
    /// This may fail if this is called while the scheduler has already
    /// been shutdown. todo prevent this
    pub fn request_stop(&mut self, offset: Offset) -> Result<(), SendError<()>> {
        // physical time must be ahead of logical time so
        // this event is scheduled for the future
        let tag = EventTag::pure(self.initial_time, Instant::now() + offset.to_duration());

        let evt = Event { tag, payload: EventPayload::Terminate };
        self.tx.send(evt).map_err(|e| {
            warn!("Event could not be sent! {:?}", e);
            SendError(())
        })
    }

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
        // physical time must be ahead of logical time so
        // this event is scheduled for the future
        action.use_mut(|action| {
            let tag = EventTag::pure(self.initial_time, Instant::now() + offset.to_duration());
            action.schedule_future_value(tag, value);

            let downstream = self.dataflow.reactions_triggered_by(&action.get_id());
            let evt = Event { tag, payload: EventPayload::Reactions(Cow::Borrowed(downstream)) };
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
    /// The initial time of the application. This is the tag
    /// at which the `startup` trigger is triggered.
    T0,
    /// Represents the tag that is at the given offset from
    /// the initial tag ([T0]).
    At(Duration),
    /// Like [At](Self::At), but you can mention a microstep.
    Tag(Duration, crate::time::MS),
}

#[cfg(feature = "test-utils")]
impl TagSpec {
    fn to_tag(self, t0: Instant) -> EventTag {
        match self {
            TagSpec::T0 => EventTag::pure(t0, t0),
            TagSpec::At(offset) => EventTag::offset(t0, offset),
            TagSpec::Tag(offset, step) => EventTag::offset_with_micro(t0, offset, MicroStep::new(step)),
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
    ///
    /// You can use this in conjunction with the [after!()](crate::after)
    /// macro, for instance:
    /// ```
    /// # use reactor_rt::Duration;
    /// ```
    After(Duration),

    /// Will be scheduled as soon as possible. This does not
    /// mean that the action will trigger right away. The
    /// action's inherent minimum delay must be taken into account,
    /// and even with a zero minimal delay, a delay of one microstep
    /// is applied. This is equivalent to
    /// ```no_compile
    /// # use reactor_rt::{Duration, Offset::After};
    /// After(Duration::ZERO)
    /// ```
    Asap,

    /// Will be scheduled at least after the provided duration,
    /// which is given in seconds. This is equivalent
    /// to
    /// ```no_compile
    /// # use reactor_rt::Duration;
    /// After(Duration::from_secs(_))
    /// ```
    AfterSeconds(u64),

    /// Will be scheduled at least after the provided duration,
    /// which is given in milliseconds (ms). This is equivalent
    /// to
    /// ```no_compile
    /// # use reactor_rt::Duration;
    /// After(Duration::from_millis(_))
    /// ```
    AfterMillis(u64),

    /// Will be scheduled at least after the provided duration,
    /// which is given in microseconds (µs). This is equivalent
    /// to
    /// ```no_compile
    /// # use reactor_rt::Duration;
    /// After(Duration::from_micros(_))
    /// ```
    AfterMicros(u64),

    /// Will be scheduled at least after the provided duration,
    /// which is given in microseconds (µs). This is equivalent
    /// to
    /// ```no_compile
    /// # use reactor_rt::Duration;
    /// After(Duration::from_nanos(_))
    /// ```
    AfterNanos(u64),

}

impl Offset {
    pub(crate) const ZERO: Duration = Duration::from_millis(0);

    #[inline]
    pub(in crate) fn to_duration(&self) -> Duration {
        match self {
            Offset::After(d) => d.clone(),
            Offset::Asap => Offset::ZERO,
            // todo remove those
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
    pub tag: EventTag,
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

/// Allows directly enqueuing reactions for a future,
/// unspecified logical time. This is only relevant
/// during the initialization of reactors.
pub struct StartupCtx<'a, 'x, 't> {
    ctx: ReactionCtx<'a, 'x, 't>,
}

/// A set of reactions.
#[doc(hidden)]
pub type ReactionSet = Vec<GlobalReactionId>;

impl<'a, 'x, 't> StartupCtx<'a, 'x, 't> {
    pub(super) fn new(ctx: ReactionCtx<'a, 'x, 't>) -> Self {
        Self { ctx }
    }

    pub(super) fn todo_now(self) -> Option<Cow<'x, ExecutableReactions>> {
        self.ctx.0.insides.todo_now
    }

    #[inline]
    #[doc(hidden)]
    pub fn enqueue(&mut self, reactions: &ReactionSet) {
        self.ctx.enqueue_now(Cow::Owned(self.ctx.make_executable(reactions)))
    }

    #[doc(hidden)]
    pub fn start_timer(&mut self, t: &Timer) {
        let downstream = self.ctx.reactions_triggered_by(t.get_id());
        if t.offset.is_zero() {
            // no offset
            self.ctx.enqueue_now(Cow::Borrowed(downstream))
        } else {
            self.ctx.enqueue_later(downstream, self.ctx.make_successor_tag(t.offset))
        }
    }
}
