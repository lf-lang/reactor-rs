use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::time::Instant;

use super::ReactionPlan;
use crate::scheduler::dependencies::{DataflowInfo, ExecutableReactions};
use crate::triggers::TriggerId;
use crate::*;

/// The tag of an event.
///
/// Tags correspond to a point on the logical timeline, and also
/// implement *superdense time*, which means an
/// infinite sequence of tags may be processed for any logical
/// instant. The label on this sequence is called the *microstep*
/// of the tag.
///
/// Use the [tag!](crate::tag) macro to create this struct with
/// convenient syntax.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct EventTag {
    /// The time offset from the origin of the logical timeline.
    /// Knowing the start time of the application is necessary to
    /// convert this to an absolute [Instant] (see [Self::to_logical_time]).
    pub offset_from_t0: Duration,
    /// The microstep of this tag.
    pub microstep: MicroStep,
}

impl EventTag {
    /// The tag of the startup event.
    pub const ORIGIN: EventTag = EventTag {
        offset_from_t0: Duration::from_millis(0),
        microstep: MicroStep::ZERO,
    };

    /// Returns the logical instant for this tag, using the
    /// initial time `t0`.
    #[inline]
    pub fn to_logical_time(&self, t0: Instant) -> Instant {
        t0 + self.offset_from_t0
    }

    /// Returns the amount of time elapsed since the start
    /// of the app.
    ///
    /// ```no_run
    /// # use std::time::Instant;
    /// # use reactor_rt::EventTag;
    /// # let t0: Instant = unimplemented!();
    /// # let tag1: EventTag = unimplemented!();
    /// assert_eq!(tag1.duration_since_start(), tag1.to_logical_time(t0) - t0)
    /// ```
    #[inline]
    pub fn duration_since_start(&self) -> Duration {
        self.offset_from_t0
    }

    /// Returns the microstep of this tag.
    #[inline]
    pub fn microstep(&self) -> MicroStep {
        self.microstep
    }

    /// Create a tag for the zeroth microstep of the given instant.
    #[inline]
    pub(crate) fn absolute(t0: Instant, instant: Instant) -> Self {
        Self {
            offset_from_t0: instant - t0,
            microstep: MicroStep::ZERO,
        }
    }

    /// Create a new tag from its offset from t0 and a microstep.
    /// Use the [tag!](crate::tag) macro for more convenient syntax.
    #[inline]
    pub fn offset(offset_from_t0: Duration, microstep: crate::time::MS) -> Self {
        Self {
            offset_from_t0,
            microstep: MicroStep::new(microstep),
        }
    }

    /// Returns a tag that is strictly greater than this one.
    #[inline]
    pub(crate) fn successor(self, offset: Duration) -> Self {
        if offset.is_zero() {
            self.next_microstep()
        } else {
            Self {
                offset_from_t0: self.offset_from_t0 + offset,
                microstep: MicroStep::ZERO,
            }
        }
    }

    #[inline]
    pub(crate) fn next_microstep(&self) -> Self {
        Self {
            offset_from_t0: self.offset_from_t0,
            microstep: self.microstep + 1,
        }
    }

    #[inline]
    pub(crate) fn now(t0: Instant) -> Self {
        Self {
            offset_from_t0: Instant::now() - t0,
            microstep: MicroStep::ZERO,
        }
    }
}

impl Display for EventTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let elapsed = self.offset_from_t0;
        write!(
            f,
            "(T0 + {} ns = {} ms, {})",
            elapsed.as_nanos(),
            elapsed.as_millis(),
            self.microstep
        )
    }
}

/// A tagged event of the reactor program. Events are tagged
/// with the logical instant at which they must be processed.
/// They are queued and processed in order. See [self::EventQueue].
///
/// [self::AsyncCtx] may only communicate with
/// the scheduler by sending events.
#[derive(Debug)]
pub(super) struct Event<'x> {
    /// The tag at which the reactions to this event must be executed.
    /// This is always > to the latest *processed* tag, by construction
    /// of the reactor application.
    pub(super) tag: EventTag,
    /// A set of reactions to execute.
    pub reactions: ReactionPlan<'x>,
    /// Whether we should terminate the application at
    /// the tag of this event (after processing the tag).
    pub terminate: bool,
}

impl<'x> Event<'x> {
    pub fn absorb(&mut self, other: Event<'x>) {
        debug_assert_eq!(self.tag, other.tag);
        self.reactions = ExecutableReactions::merge_cows(self.reactions.take(), other.reactions);
        self.terminate |= other.terminate;
    }

    pub fn execute(tag: EventTag, reactions: Cow<'x, ExecutableReactions<'x>>) -> Self {
        Self { tag, reactions: Some(reactions), terminate: false }
    }
    pub fn terminate_at(tag: EventTag) -> Self {
        Self { tag, reactions: None, terminate: true }
    }
}

/// An event sent by a physical action from an asynchronous
/// thread. This is distinct from [Event] so as not to have
/// to send references, which require quantifying the lifetime
/// of the event and event queue and everything.
pub(super) struct PhysicalEvent {
    /// The tag.
    pub tag: EventTag,
    /// The ID of the physical action that triggered this event.
    pub trigger_id: Option<TriggerId>,
    pub terminate: bool,
}

impl PhysicalEvent {
    /// Turn a [PhysicalEvent] into an [Event] within the scheduler.
    pub(super) fn make_executable(self, dataflow: &DataflowInfo) -> Event {
        let PhysicalEvent { tag, trigger_id, terminate } = self;
        Event {
            tag,
            terminate,
            reactions: trigger_id.map(|id| Cow::Borrowed(dataflow.reactions_triggered_by(&id))),
        }
    }

    pub fn trigger(tag: EventTag, trigger: TriggerId) -> Self {
        Self { tag, trigger_id: Some(trigger), terminate: false }
    }
    pub fn terminate_at(tag: EventTag) -> Self {
        Self { tag, trigger_id: None, terminate: true }
    }
}

/// A queue of pending [Event]s. Events are ordered by tag,
/// so this is not a FIFO queue.
#[derive(Default)]
pub(super) struct EventQueue<'x> {
    /// This list is sorted by the tag of each event (in ascending order).
    ///
    /// But insertion is at worse O(n)... And it's easy to build
    /// a pathological program where this worse case is always hit.
    /// Theoretically using a tree/ heap would be useful here.
    ///        .
    ///       ..
    ///      ...
    ///     ....
    value_list: VecDeque<Event<'x>>,
}

impl<'x> EventQueue<'x> {
    /// Removes and returns the earliest tag
    pub fn take_earliest(&mut self) -> Option<Event<'x>> {
        self.value_list.pop_front()
    }

    // todo perf: we could make a more optimal function to push a
    //  lot of events at once. Consider the following algorithm:
    //  - start with a sorted `self.value_list` and a (non-sorted) `new_evts: Vec<Event>`
    //  - sort the new events in place (in a Cow maybe). They'll
    //  probably come in already sorted but we can't assume this.
    //  Use an algorithm that best-cases for sorted data. (eg https://crates.io/crates/dmsort)
    //  - take the earliest new event and binary search to insert it.
    //  - then do the same thing but only on the remaining (to the right)
    //  portion of `self.value_list`. Basically the routine of an insertion
    //  sort.

    /// Push an event into the heap.
    pub(super) fn push(&mut self, evt: Event<'x>) {
        match self.value_list.binary_search_by_key(&evt.tag, |e| e.tag) {
            Ok(idx) => self.value_list[idx].absorb(evt),
            Err(idx) => self.value_list.insert(idx, evt),
        }
    }
}
