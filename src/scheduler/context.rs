use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::reconnectable::{Receiver, SendError, Sender};
use smallvec::SmallVec;

use super::*;
use crate::assembly::*;
use crate::scheduler::dependencies::{DataflowInfo, ExecutableReactions, LevelIx};
use crate::*;

/// The context in which a reaction executes. Its API
/// allows mutating the event queue of the scheduler.
/// Only the interactions declared at assembly time
/// are allowed.

// Implementation details:
// ReactionCtx is an API built around a ReactionWave. A single
// ReactionCtx may be used for multiple ReactionWaves, but
// obviously at disjoint times (&mut).
pub struct ReactionCtx<'a, 'x> {
    pub(super) insides: RContextForwardableStuff<'x>,

    /// Logical time of the execution of this wave, constant
    /// during the existence of the object
    tag: EventTag,

    /// Level of the reaction being executed.
    pub(super) cur_level: LevelIx,

    /// ID of the reaction being executed.
    current_reaction: Option<GlobalReactionId>,

    /// Sender to schedule events that should be executed later than this wave.
    rx: &'a Receiver<PhysicalEvent>,

    /// Start time of the program.
    initial_time: Instant,

    // globals, also they might be copied and passed to AsyncCtx
    dataflow: &'x DataflowInfo,
    debug_info: DebugInfoProvider<'a>,
    /// Whether the scheduler has been shut down.
    was_terminated_atomic: &'a Arc<AtomicBool>,
    /// In ReactionCtx, this will only be true if this is the shutdown tag.
    /// It duplicates [Self::was_terminated_atomic], to avoid an atomic
    /// operation within [Self::is_shutdown].
    was_terminated: bool,
}

impl<'a, 'x> ReactionCtx<'a, 'x> {
    /// Returns the start time of the execution of this program.
    ///
    /// This is a logical instant with microstep zero.
    #[inline]
    pub fn get_start_time(&self) -> Instant {
        self.initial_time
    }

    /// Returns the current physical time.
    ///
    /// Repeated invocation of this method may produce different
    /// values, although [Instant] is monotonic. The
    /// physical time is necessarily greater than the logical time.
    #[inline]
    pub fn get_physical_time(&self) -> Instant {
        Instant::now()
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

    /// Returns whether this tag is the shutdown tag of the
    /// application. If so, it's necessarily the very last
    /// invocation of the current reaction (on a given reactor
    /// instance).
    ///
    /// Repeated invocation of this method will always produce
    /// the same value.
    #[inline]
    pub fn is_shutdown(&self) -> bool {
        self.was_terminated
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

    /// Returns the number of active workers in the execution of
    /// a reactor program.
    ///
    /// Return values:
    /// * `1` if threading is not enabled.
    /// * If threading is enabled and a number of workers was specified,
    ///   it returns that number.
    /// * And if the number of workers was left unspecified,
    ///   the return value might vary.
    pub fn num_workers(&self) -> usize {
        cfg_if::cfg_if! {
            if #[cfg(feature = "parallel-runtime")] {
                rayon::current_num_threads()
            } else {
                1
            }
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
    /// # use reactor_rt::{ReactionCtx, Port};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: &Port<u32> = panic!();
    /// if let Some(value) = ctx.get(port) {
    ///     // branch is taken if the port is set
    /// }
    /// ```
    #[inline]
    pub fn get<T: Copy>(&self, container: &impl ReactionTrigger<T>) -> Option<T> {
        container.borrow().get_value(&self.get_tag(), &self.get_start_time())
    }

    /// Returns a reference to the current value of a port or action at this
    /// logical time. If the value is absent, [Option::None] is
    /// returned.  This is the case if the action or port is
    /// not present ([Self::is_present]), or if no value was
    /// scheduled (action values are optional, see [Self::schedule_with_v]).
    ///
    /// This does not require the value to be Copy, however, the implementation
    /// of this method currently may require unsafe code. The method is therefore
    /// not offered when compiling with the `no-unsafe` feature.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::{Port, ReactionCtx};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: &Port<u32> = panic!();
    /// if let Some(value) = ctx.get_ref(port) {
    ///     // value is a ref to the internal value
    /// }
    /// ```
    #[inline]
    #[cfg(not(feature = "no-unsafe"))]
    pub fn get_ref<'q, T>(&self, container: &'q impl crate::triggers::ReactionTriggerWithRefAccess<T>) -> Option<&'q T> {
        container.get_value_ref(&self.get_tag(), &self.get_start_time())
    }

    /// Executes the provided closure on the value of the port
    /// or action. The value is fetched by reference and not
    /// copied.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, Port};
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let port: &Port<String> = panic!();
    /// let len = ctx.use_ref(port, |str| str.map(String::len).unwrap_or(0));
    /// // equivalent to
    /// let len = ctx.use_ref_opt(port, String::len).unwrap_or(0);
    /// ```
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, Port};
    /// # let ctx: &mut ReactionCtx = unimplemented!();
    /// # let port: &Port<String> = unimplemented!();
    ///
    /// if let Some(str) = ctx.use_ref_opt(port, Clone::clone) {
    ///     // only entered if the port value is present, so no need to check is_present
    /// }
    /// ```
    ///
    /// See also the similar [Self::use_ref_opt].
    #[inline]
    pub fn use_ref<T, O>(&self, container: &impl ReactionTrigger<T>, action: impl FnOnce(Option<&T>) -> O) -> O {
        container
            .borrow()
            .use_value_ref(&self.get_tag(), &self.get_start_time(), action)
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
    pub fn set<T>(&mut self, port: &mut Port<T>, value: T)
    where
        T: Sync,
    {
        if cfg!(debug_assertions) {
            self.check_set_port_is_legal(port)
        }
        port.set_impl(Some(value));
        self.enqueue_now(Cow::Borrowed(self.reactions_triggered_by(port.get_id())));
    }

    fn check_set_port_is_legal<T: Sync>(&self, port: &mut Port<T>) {
        let port_id = port.get_id();
        let port_container = self.debug_info.id_registry.get_trigger_container(port_id).unwrap();
        let reaction_container = self.current_reaction.unwrap().0.container();
        match port.get_kind() {
            PortKind::Input => {
                let port_grandpa = self.debug_info.id_registry.get_container(port_container);
                assert_eq!(
                    Some(reaction_container),
                    port_grandpa,
                    "Input port {} can only be set by reactions of its grandparent, got reaction {}",
                    self.debug_info.id_registry.fmt_component(port_id),
                    self.debug_info.display_reaction(self.current_reaction.unwrap()),
                );
            }
            PortKind::Output => {
                assert_eq!(
                    reaction_container,
                    port_container,
                    "Input port {} can only be set by reactions of its parent, got reaction {}",
                    self.debug_info.id_registry.fmt_component(port_id),
                    self.debug_info.display_reaction(self.current_reaction.unwrap()),
                );
            }
            // todo
            PortKind::ChildInputReference => {}
            PortKind::ChildOutputReference => {}
        }
    }

    /// Sets the value of the given port, if the given value is `Some`.
    /// Otherwise the port is not set and no reactions are triggered.
    ///
    /// The change is visible at the same logical time, i.e.
    /// the value propagates immediately. This may hence
    /// schedule more reactions that should execute at the
    /// same logical time.
    ///
    /// ```no_run
    /// # use reactor_rt::{ReactionCtx, Port};
    /// # let ctx: &mut ReactionCtx = unimplemented!();
    /// # let source: &Port<u32> = unimplemented!();
    /// # let sink: &mut Port<u32> = unimplemented!();
    ///
    /// ctx.set_opt(sink, ctx.get(source));
    /// // equivalent to
    /// if let Some(value) = ctx.get(source) {
    ///     ctx.set(sink, value);
    /// }
    /// ```
    ///
    #[inline]
    pub fn set_opt<T>(&mut self, port: &mut Port<T>, value: Option<T>)
    where
        T: Sync,
    {
        if let Some(v) = value {
            self.set(port, v)
        }
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
    /// plus an optional additional time delay (see [Offset]). This
    /// delay is added to the current logical (resp. physical) time
    /// for logical (resp. physical) actions.
    ///
    /// This is like [Self::schedule_with_v], where the value is [None].
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::prelude::*;
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let action: &mut LogicalAction<String> = panic!();
    /// ctx.schedule(action, Asap);         // will be executed one microstep from now (+ own delay)
    /// ctx.schedule(action, after!(2 ms)); // will be executed 2 milliseconds from now (+ own delay)
    /// ctx.schedule(action, After(delay!(2 ms)));             // equivalent to the previous
    /// ctx.schedule(action, After(Duration::from_millis(2))); // equivalent to the previous
    /// ```
    #[inline]
    pub fn schedule<T: Sync>(&mut self, action: &mut impl SchedulableAsAction<T>, offset: Offset) {
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
    /// plus an optional additional time delay (see [Offset]). This
    /// delay is added to the current logical (resp. physical) time
    /// for logical (resp. physical) actions.
    ///
    /// ### Examples
    ///
    /// ```no_run
    /// # use reactor_rt::prelude::*;
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
    pub fn schedule_with_v<T: Sync>(&mut self, action: &mut impl SchedulableAsAction<T>, value: Option<T>, offset: Offset) {
        action.schedule_with_v(self, value, offset)
    }

    /// Add new reactions to execute later (at least 1 microstep later).
    ///
    /// This is used for actions.
    #[inline]
    pub(crate) fn enqueue_later(&mut self, downstream: &'x ExecutableReactions, tag: EventTag) {
        debug_assert!(tag > self.get_tag());

        let evt = Event::execute(tag, Cow::Borrowed(downstream));
        self.insides.future_events.push(evt);
    }

    #[inline]
    pub(crate) fn enqueue_now(&mut self, downstream: Cow<'x, ExecutableReactions<'x>>) {
        match &mut self.insides.todo_now {
            Some(ref mut do_next) => do_next.to_mut().absorb_after(downstream.as_ref(), self.cur_level.next()),
            None => self.insides.todo_now = Some(downstream),
        }
    }

    fn reactions_triggered_by(&self, trigger: TriggerId) -> &'x ExecutableReactions<'x> {
        self.dataflow.reactions_triggered_by(&trigger)
    }

    fn make_successor_tag(&self, offset_from_now: Duration) -> EventTag {
        self.get_tag().successor(offset_from_now)
    }

    /// Spawn a new thread that can use a [AsyncCtx]
    /// to push asynchronous events to the reaction queue. This is
    /// only useful with [physical actions](crate::PhysicalAction).
    ///
    /// Since the thread is allowed to keep references into the
    /// internals of the scheduler, it is joined when the scheduler
    /// shuts down. This means the scheduler will wait for the
    /// thread to finish its task. For that reason, the thread's
    /// closure should not execute an infinite loop, it should at
    /// least check that the scheduler has not been terminated by
    /// polling [AsyncCtx::was_terminated].
    ///
    /// ### Example
    ///
    /// ```no_run
    /// # use reactor_rt::prelude::*;
    /// fn some_reaction(ctx: &mut ReactionCtx, phys_action: &PhysicalActionRef<u32>) {
    ///     let phys_action = phys_action.clone(); // clone to move it into other thread
    ///     ctx.spawn_physical_thread(move |link| {
    ///         std::thread::sleep(Duration::from_millis(200));
    ///         // This will push an event whose tag is the
    ///         // current physical time at the point of this
    ///         // statement.
    ///         link.schedule_physical_with_v(&phys_action, Some(123), Asap).unwrap();
    ///     });
    /// }
    /// ```
    ///
    pub fn spawn_physical_thread<F, R>(&mut self, f: F) -> JoinHandle<R>
    where
        // is there a practical reason to encapsulate this?
        F: FnOnce(&mut AsyncCtx) -> R,
        F: Send + 'static,
        R: Send + 'static,
    {
        let tx = self.rx.new_sender();
        let initial_time = self.initial_time;
        let was_terminated = self.was_terminated_atomic.clone();

        std::thread::spawn(move || {
            let mut link = AsyncCtx { tx, initial_time, was_terminated };
            f(&mut link)
        })
    }

    /// Request that the application shutdown, possibly with
    /// a particular offset. Just like for actions, even a zero
    /// offset will only trigger the special `shutdown` trigger
    /// at the earliest one microstep after the current tag.
    ///
    /// ```no_run
    /// # use reactor_rt::prelude::*;
    /// # let ctx: &mut ReactionCtx = panic!();
    /// # let action: &mut LogicalAction<&'static str> = panic!();
    /// // trigger shutdown on the next microstep
    /// ctx.request_stop(Asap);
    ///
    /// // trigger shutdown in *at most* 1 msec (in logical time).
    /// // If in the meantime, another `request_stop` call schedules
    /// // shutdown for an earlier tag, that one will be honored instead.
    /// ctx.request_stop(after!(1 msec));
    /// ```
    #[inline]
    pub fn request_stop(&mut self, offset: Offset) {
        let tag = self.make_successor_tag(offset.to_duration());

        let evt = Event::terminate_at(tag);
        self.insides.future_events.push(evt);
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

    /// Execute the given reaction with the given reactor.
    #[inline]
    pub(super) fn execute(&mut self, reactor: &mut ReactorBox, reaction_id: GlobalReactionId) {
        trace!(
            "  - Executing {} (level {})",
            self.debug_info.display_reaction(reaction_id),
            self.cur_level
        );
        debug_assert_eq!(reactor.id(), reaction_id.0.container(), "Wrong reactor");
        self.current_reaction.replace(reaction_id);
        reactor.react(self, reaction_id.0.local());
        self.current_reaction.take();
    }

    pub(super) fn new(
        rx: &'a Receiver<PhysicalEvent>,
        tag: EventTag,
        initial_time: Instant,
        todo: ReactionPlan<'x>,
        dataflow: &'x DataflowInfo,
        debug_info: DebugInfoProvider<'a>,
        was_terminated_atomic: &'a Arc<AtomicBool>,
        was_terminated: bool,
    ) -> Self {
        Self {
            insides: RContextForwardableStuff { todo_now: todo, future_events: Default::default() },
            cur_level: Default::default(),
            tag,
            current_reaction: None,
            rx,
            initial_time,
            dataflow,
            was_terminated_atomic,
            debug_info,
            was_terminated,
        }
    }

    /// Fork a context. Some things are shared, but not the
    /// mutable stuff.
    #[cfg(feature = "parallel-runtime")]
    pub(super) fn fork(&self) -> Self {
        Self {
            insides: Default::default(),

            // all of that is common to all contexts
            tag: self.tag,
            rx: self.rx,
            cur_level: self.cur_level,
            initial_time: self.initial_time,
            dataflow: self.dataflow,
            was_terminated: self.was_terminated,
            was_terminated_atomic: self.was_terminated_atomic,
            debug_info: self.debug_info.clone(),
            current_reaction: self.current_reaction,
        }
    }
}

/// Info that executing reactions need to make known to the scheduler.
#[derive(Default)]
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

#[cfg(feature = "parallel-runtime")]
impl RContextForwardableStuff<'_> {
    pub(super) fn merge(mut self, other: Self) -> Self {
        self.absorb(other);
        self
    }

    pub(super) fn absorb(&mut self, mut other: Self) {
        self.todo_now = ExecutableReactions::merge_cows(self.todo_now.take(), other.todo_now);
        self.future_events.append(&mut other.future_events);
    }
}

/// A type that can affect the logical event queue to implement
/// asynchronous physical actions. This is a "link" to the event
/// system, from the outside world.
///
/// See [ReactionCtx::spawn_physical_thread].
///
#[derive(Clone)]
pub struct AsyncCtx {
    tx: Sender<PhysicalEvent>,
    initial_time: Instant,
    /// Whether the scheduler has been terminated.
    was_terminated: Arc<AtomicBool>,
}

impl AsyncCtx {
    /// Returns true if the scheduler has been shutdown. When
    /// that's true, calls to other methods of this type will
    /// fail with [SendError].
    pub fn was_terminated(&self) -> bool {
        self.was_terminated.load(Ordering::SeqCst)
    }

    /// Request that the application shutdown, possibly with
    /// a particular offset from the current physical time.
    ///
    /// This may fail if this is called while the scheduler
    /// has already been shutdown. An Ok result is also not
    /// a guarantee that the event will be processed: the
    /// scheduler may be in the process of shutting down,
    /// or its shutdown might be programmed for a logical
    /// time which precedes the current physical time.
    pub fn request_stop(&mut self, offset: Offset) -> Result<(), SendError<()>> {
        // physical time must be ahead of logical time so
        // this event is scheduled for the future
        let tag = EventTag::absolute(self.initial_time, Instant::now() + offset.to_duration());

        let evt = PhysicalEvent::terminate_at(tag);
        self.tx.send(evt).map_err(|e| {
            warn!("Event could not be sent! {:?}", e);
            SendError(())
        })
    }

    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// Note that this locks the action.
    ///
    /// This may fail if this is called while the scheduler
    /// has already been shutdown. An Ok result is also not
    /// a guarantee that the event will be processed: the
    /// scheduler may be in the process of shutting down,
    /// or its shutdown might be programmed for a logical
    /// time which precedes the current physical time.
    ///
    pub fn schedule_physical<T: Sync>(
        &mut self,
        action: &PhysicalActionRef<T>,
        offset: Offset,
    ) -> Result<(), SendError<Option<T>>> {
        self.schedule_physical_with_v(action, None, offset)
    }

    /// Schedule an action to run after its own implicit time delay
    /// plus an optional additional time delay. These delays are in
    /// logical time.
    ///
    /// Note that this locks the action.
    ///
    /// This may fail if this is called while the scheduler
    /// has already been shutdown. An Ok result is also not
    /// a guarantee that the event will be processed: the
    /// scheduler may be in the process of shutting down,
    /// or its shutdown might be programmed for a logical
    /// time which precedes the current physical time.
    ///
    pub fn schedule_physical_with_v<T: Sync>(
        &mut self,
        action: &PhysicalActionRef<T>,
        value: Option<T>,
        offset: Offset,
    ) -> Result<(), SendError<Option<T>>> {
        // physical time must be ahead of logical time so
        // this event is scheduled for the future
        action
            .use_mut_p(value, |action, value| {
                let tag = EventTag::absolute(self.initial_time, Instant::now() + offset.to_duration());
                action.0.schedule_future_value(tag, value);

                let evt = PhysicalEvent::trigger(tag, action.get_id());
                self.tx.send(evt).map_err(|e| {
                    warn!("Event could not be sent! {:?}", e);
                    SendError(action.0.forget_value(&tag))
                })
            })
            .unwrap_or_else(|value| Err(SendError(value)))
    }
}

/// Implemented by LogicalAction and PhysicalAction references
/// to give access to [ReactionCtx::schedule] and variants.
pub trait SchedulableAsAction<T: Sync> {
    #[doc(hidden)]
    fn schedule_with_v(&mut self, ctx: &mut ReactionCtx, value: Option<T>, offset: Offset);
}

impl<T: Sync> SchedulableAsAction<T> for LogicalAction<T> {
    fn schedule_with_v(&mut self, ctx: &mut ReactionCtx, value: Option<T>, offset: Offset) {
        let eta = ctx.make_successor_tag(self.0.min_delay + offset.to_duration());
        self.0.schedule_future_value(eta, value);
        let downstream = ctx.dataflow.reactions_triggered_by(&self.get_id());
        ctx.enqueue_later(downstream, eta);
    }
}

impl<T: Sync> SchedulableAsAction<T> for PhysicalActionRef<T> {
    fn schedule_with_v(&mut self, ctx: &mut ReactionCtx, value: Option<T>, offset: Offset) {
        self.use_mut_p(value, |action, value| {
            let tag = EventTag::absolute(ctx.initial_time, Instant::now() + offset.to_duration());
            action.0.schedule_future_value(tag, value);
            let downstream = ctx.dataflow.reactions_triggered_by(&action.get_id());
            ctx.enqueue_later(downstream, tag);
        })
        .ok();
    }
}

/// An offset from the current event.
///
/// This is to be used with [ReactionCtx::schedule].
#[derive(Copy, Clone, Debug)]
pub enum Offset {
    /// Specify that the trigger will fire at least after
    /// the provided duration.
    ///
    /// If the duration is zero (eg [Asap](Self::Asap)), it does not
    /// mean that the trigger will fire right away. For actions, the
    /// action's inherent minimum delay must be taken into account,
    /// and even with a zero minimal delay, a delay of one microstep
    /// is applied.
    ///
    /// You can use the [after!()](crate::after) macro, instead
    /// of using this directly. For instance:
    /// ```
    /// # use reactor_rt::prelude::*;
    /// assert_eq!(after!(15 ms), After(Duration::from_millis(15)));
    /// ```
    After(Duration),

    /// Specify that the trigger will fire as soon as possible.
    /// This does not mean that the action will trigger right away. The
    /// action's inherent minimum delay must be taken into account,
    /// and even with a zero minimal delay, a delay of one microstep
    /// is applied. This is equivalent to
    /// ```
    /// # use reactor_rt::prelude::*;
    /// assert_eq!(Asap, After(Duration::ZERO));
    /// ```
    Asap,
}

impl Offset {
    #[inline]
    pub(crate) fn to_duration(self) -> Duration {
        match self {
            Offset::After(d) => d,
            Offset::Asap => Duration::ZERO,
        }
    }
}

impl PartialEq<Self> for Offset {
    fn eq(&self, other: &Self) -> bool {
        self.to_duration() == other.to_duration()
    }
}

impl Eq for Offset {}

impl Hash for Offset {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_duration().hash(state);
    }
}

/// Cleans up a tag
/// TODO get rid of this!
///  At least for multiports it's really bad
///  Maybe we can keep a set of the ports that are present in ReactionCtx
#[doc(hidden)]
pub struct CleanupCtx {
    /// Tag we're cleaning up
    pub tag: EventTag,
}

impl CleanupCtx {
    pub fn cleanup_multiport<T: Sync>(&self, port: &mut Multiport<T>) {
        // todo bound ports don't need to be cleared
        for channel in port {
            channel.clear_value()
        }
    }

    pub fn cleanup_port<T: Sync>(&self, port: &mut Port<T>) {
        port.clear_value()
    }

    pub fn cleanup_logical_action<T: Sync>(&self, action: &mut LogicalAction<T>) {
        action.0.forget_value(&self.tag);
    }

    pub fn cleanup_physical_action<T: Sync>(&self, action: &mut PhysicalActionRef<T>) {
        action.use_mut(|a| a.0.forget_value(&self.tag)).ok();
    }
}
