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


use std::borrow::Cow;
use std::time::Instant;

use index_vec::IndexVec;

pub use assembly::*;
pub use context::*;
pub(in self) use event_queue::*;
pub use scheduler_impl::*;

use crate::{Duration, MicroStep, PhysicalInstant, ReactorBehavior, ReactorId};

use self::depgraph::ExecutableReactions;

mod context;
mod scheduler_impl;
mod event_queue;
mod depgraph;
mod assembly;

/// The tag of an event.
///
/// Tags correspond to a point on the logical timeline, and also
/// implement *superdense time*, which means an
/// infinite sequence of tags may be processed for any logical
/// instant. The label on this sequence is called the *microstep*
/// of the tag.
///
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct EventTag {
    offset_from_t0: Duration,
    microstep: MicroStep,
}

impl EventTag {

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
    /// # let t0: Instant = todo!();
    /// # let tag1: EventTag = todo!();
    /// # let tag2: EventTag = todo!();
    /// assert_eq!(tag1.duration_since_start(t0), tag1.to_logical_time(t0) - t0)
    /// ```
    #[inline]
    pub fn duration_since_start(&self, _t0: Instant) -> Duration {
        self.offset_from_t0
    }

    /// Returns the microstep of this tag.
    #[inline]
    pub fn microstep(&self) -> MicroStep {
        self.microstep
    }

    // creator methods

    /// Create a tag for the zeroth microstep of the given instant.
    #[inline]
    pub(crate) fn pure(t0: Instant, instant: Instant) -> Self {
        Self { offset_from_t0: instant - t0, microstep: MicroStep::ZERO }
    }

    #[inline]
    #[allow(unused)]
    pub(crate) fn offset(_t0: Instant, offset_from_t0: Duration, microstep: MicroStep) -> Self {
        Self { offset_from_t0, microstep }
    }

    /// Returns a tag that is strictly greater than this one.
    #[inline]
    pub(crate) fn successor(self, _t0: Instant, offset: Duration) -> Self {
        if offset.is_zero() {
            self.next_microstep()
        } else {
            Self {
                offset_from_t0: self.offset_from_t0 + offset,
                microstep: MicroStep::ZERO
            }
            // Self::pure(t0, self.instant + offset)
        }
    }

    #[inline]
    pub(crate) fn next_microstep(&self) -> Self {
        Self {
            offset_from_t0: self.offset_from_t0,
            // instant: self.instant,
            microstep: self.microstep + 1,
        }
    }

    #[inline]
    pub(crate) fn now(t0: Instant) -> Self {
        Self {
            offset_from_t0: PhysicalInstant::now() - t0,
            microstep: MicroStep::ZERO,
        }
    }
}


/// A tagged event of the reactor program. Events are tagged
/// with the logical instant at which they must be processed.
/// They are queued and processed in order. See [self::EventQueue].
///
/// [self::PhysicalSchedulerLink] may only communicate with
/// the scheduler by sending events.
#[derive(Debug)]
pub(self) struct Event<'x> {
    /// The tag at which the reactions to this event must be executed.
    /// This is always > to the latest *processed* tag, by construction
    /// of the reactor application.
    pub(in self) tag: EventTag,
    /// The payload.
    pub payload: EventPayload<'x>,
}

impl<'x> Event<'x> {
    pub fn absorb(&mut self, other: Event<'x>) {
        use EventPayload::*;
        debug_assert_eq!(self.tag, other.tag);
        match (&mut self.payload, other.payload) {
            (Reactions(ref mut r1), Reactions(ref r2)) => {
                r1.to_mut().absorb(r2.as_ref())
            }
            _ => {
                // Terminate wins against reactions
                self.payload = Terminate
            }
        }
    }
}

/// Identifies different kind of events.
#[derive(Debug)]
pub(self) enum EventPayload<'x> {
    /// A set of reactions to execute.
    Reactions(Cow<'x, ExecutableReactions>),
    /// Means we should terminate the application at the tag
    /// of this event.
    Terminate,
}

pub(self) type ReactorBox<'a> = Box<dyn ReactorBehavior + Send + Sync + 'a>;
pub(self) type ReactorVec<'a> = IndexVec<ReactorId, ReactorBox<'a>>;

#[inline]
pub(self) fn display_tag_impl(initial_time: Instant, tag: EventTag) -> String {
    let elapsed = tag.duration_since_start(initial_time);
    format!("(T0 + {} ns = {} ms, {})", elapsed.as_nanos(), elapsed.as_millis(), tag.microstep())
}


