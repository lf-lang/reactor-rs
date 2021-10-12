use std::borrow::{Borrow, BorrowMut};

use crossbeam_channel::{ReconnectableReceiver, Sender, SendError};
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
pub struct ReactionCtx<'a, 'x, 't> where 'x: 't {
    pub(super) insides: RContextForwardableStuff<'x>,

    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    tag: EventTag,

    /// Layer of the reaction being executed.
    cur_layer: usize,

    /// Sender to schedule events that should be executed later than this wave.
    rx: &'a ReconnectableReceiver<Event<'x>>,

    /// Start time of the program.
    initial_time: PhysicalInstant,

    // globals
    thread_spawner: &'a Scope<'t>,
    dataflow: &'x DataflowInfo,
}


impl<'a, 'x, 't> ReactionCtx<'a, 'x, 't> where 'x: 't {
    pub(in super) fn new(rx: &'a ReconnectableReceiver<Event<'x>>,
                         tag: EventTag,
                         initial_time: PhysicalInstant,
                         todo: ReactionPlan<'x>,
                         dataflow: &'x DataflowInfo,
                         thread_spawner: &'a Scope<'t>) -> Self {
        Self {
            insides: RContextForwardableStuff {
                todo_now: todo,
                future_events: Default::default(),
            },
            cur_layer: 0,
            tag,
            rx,
            initial_time,
            dataflow,
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
        let downstream = self.dataflow.reactions_triggered_by(&action.get_id());
        self.enqueue_later(downstream, eta);
    }



    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    #[inline]
    pub(in crate) fn enqueue_later(&mut self, downstream: &'x ExecutableReactions, tag: EventTag) {
        debug_assert!(tag > self.get_tag());

        let evt = Event::execute(tag, Cow::Borrowed(downstream));
        self.insides.future_events.push(evt);
    }

    #[inline]
    pub(in crate) fn enqueue_now(&mut self, downstream: Cow<'x, ExecutableReactions<'x>>) {
        match &mut self.insides.todo_now {
            Some(ref mut do_next) => do_next.to_mut().absorb_after(downstream.as_ref(), self.cur_layer + 1),
            None => self.insides.todo_now = Some(downstream)
        }
    }

    fn reactions_triggered_by(&self, trigger: TriggerId) -> &'x ExecutableReactions<'x> {
        self.dataflow.reactions_triggered_by(&trigger)
    }

    fn make_successor_tag(&self, offset_from_now: Duration) -> EventTag {
        self.get_tag().successor(offset_from_now)
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
        let tx = self.rx.new_sender();
        let dataflow = self.dataflow;
        let initial_time = self.initial_time;

        self.thread_spawner.spawn(move |subscope| {
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

        let evt = Event::terminate_at(tag);
        self.insides.future_events.push(evt);
    }

    /// Returns the start time of the execution of this program.
    ///
    /// This is a logical instant with microstep zero.
    #[inline]
    pub fn get_start_time(&self) -> PhysicalInstant {
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
    pub fn get_logical_time(&self) -> Instant {
        self.tag.to_logical_time(self.get_start_time())
    }

    /// Returns the tag at which the reaction executes.
    ///
    /// Repeated invocation of this method will always produce
    /// the same value.
    #[inline]
    pub fn get_tag(&self) -> EventTag {
        self.tag
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

    /// Reschedule a periodic timer if need be.
    /// This is called by a reaction synthesized for each timer.
    // note: reactions can't call this as they're only passed a shared reference to a timer.
    #[doc(hidden)]
    #[inline]
    pub fn reschedule_timer(&mut self, timer: &mut Timer) {
        if timer.is_periodic() {
            let downstream = self.reactions_triggered_by(timer.get_id());
            self.enqueue_later(downstream, self.make_successor_tag(timer.period));
        }
    }

    /// Schedule the first triggering of the given timer.
    /// This is called by a reaction synthesized for each timer.
    // note: reactions can't call this as they're only passed a shared references to timers.
    #[doc(hidden)]
    #[inline]
    pub fn bootstrap_timer(&mut self, timer: &mut Timer) {
        // we're in startup
        let downstream = self.reactions_triggered_by(timer.get_id());
        if timer.offset.is_zero() {
            // no offset
            self.enqueue_now(Cow::Borrowed(downstream))
        } else {
            self.enqueue_later(downstream, self.make_successor_tag(timer.offset))
        }
    }

    pub(super) fn take_reactions_enqueued_now(&mut self) -> ReactionPlan<'x> {
        self.insides.todo_now.take()
    }

    pub(super) fn set_cur_layer(&mut self, cur_layer: usize) {
        self.cur_layer = cur_layer;
    }

    pub(super) fn drain_reactions_enqueued_later(&mut self) -> impl Iterator<Item=Event<'x>> + '_ {
        self.insides.future_events.drain(..)
    }

    /// Fork a context. Some things are shared, but not the
    /// mutable stuff.
    #[cfg(feature = "parallel-runtime")]
    pub(super) fn fork(&self) -> Self {
        Self {
            insides: Default::default(),

            // all of that is common to all contexts
            tag: self.tag,
            tx: self.tx.clone(),
            cur_layer: self.cur_layer,
            initial_time: self.initial_time,
            thread_spawner: self.thread_spawner,
            dataflow: self.dataflow,
        }
    }
}

/// Info that executing reactions need to make known to the scheduler.
pub(super) struct RContextForwardableStuff<'x> {
    /// Remaining reactions to execute before the wave dies.
    /// Using [Option] and [Cow] optimises for the case where
    /// zero or exactly one port/action is set, and minimises
    /// copies.
    ///
    /// This is mutable: if a reaction sets a port, then the
    /// downstream of that port is inserted in into this
    /// data structure.
    pub(super) todo_now: ReactionPlan<'x>,

    /// Events that were produced for a strictly greater
    /// logical time than a current one.
    pub(super) future_events: SmallVec<[Event<'x>; 4]>,
}

impl Default for RContextForwardableStuff<'_> {
    fn default() -> Self {
        Self {
            todo_now: None,
            future_events: Default::default(),
        }
    }
}

#[cfg(feature = "parallel-runtime")]
impl RContextForwardableStuff<'_> {
    pub(super) fn merge(mut self, mut other: Self) -> Self {
        self.todo_now = ExecutableReactions::merge_cows(self.todo_now, other.todo_now);
        self.future_events.append(&mut other.future_events);
        self
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
        let tag = EventTag::absolute(self.initial_time, Instant::now() + offset.to_duration());

        let evt = Event::terminate_at(tag);
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
            let tag = EventTag::absolute(self.initial_time, Instant::now() + offset.to_duration());
            action.schedule_future_value(tag, value);

            let downstream = self.dataflow.reactions_triggered_by(&action.get_id());
            let evt = Event::execute(tag, Cow::Borrowed(downstream));
            self.tx.send(evt).map_err(|e| {
                warn!("Event could not be sent! {:?}", e);
                SendError(action.forget_value(&tag))
            })
        })
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
    /// You can use the [after!()](crate::after) macro, instead
    /// of using this directly. For instance:
    /// ```
    /// # use reactor_rt::{Duration, Offset::After, after};
    /// assert_eq!(After(Duration::from_millis(15)),
    ///            after!(15 ms)) // more concise
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
}

impl Offset {
    pub(crate) const ZERO: Duration = Duration::from_millis(0);

    #[inline]
    pub(in crate) fn to_duration(&self) -> Duration {
        match self {
            Offset::After(d) => d.clone(),
            Offset::Asap => Offset::ZERO,
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
