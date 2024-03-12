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

use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::ops::Range;
use std::time::Instant;

use index_vec::Idx;

use crate::impl_types::TriggerIdImpl;
use crate::EventTag;

/// Common trait for actions, ports, and timer objects handed
/// to reaction functions. This is meant to be used through the
/// API of [ReactionCtx](crate::ReactionCtx) instead of directly.
pub trait ReactionTrigger<T> {
    /// Returns whether the trigger is present, given that
    /// the current logical time is the parameter.
    #[inline]
    fn is_present(&self, now: &EventTag, start: &Instant) -> bool {
        self.use_value_ref(now, start, |opt| opt.is_some())
    }

    /// Copies the value out, if it is present. Whether a *value*
    /// is present is not in general the same thing as whether *this trigger*
    /// [Self::is_present]. See [crate::ReactionCtx::get].
    fn get_value(&self, now: &EventTag, start: &Instant) -> Option<T>
    where
        T: Copy;

    /// Execute an action using the current value of this trigger.
    /// The closure is called even if the value is absent (with a [None]
    /// argument).
    fn use_value_ref<O>(&self, now: &EventTag, start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O;
}

#[cfg(not(feature = "no-unsafe"))]
pub trait ReactionTriggerWithRefAccess<T> {
    /// Returns a reference to the value, if it is present. Whether a *value*
    /// is present is not in general the same thing as whether *this trigger*
    /// [Self::is_present]. See [crate::ReactionCtx::get_ref].
    ///
    /// This does not require the value to be Copy, however, the implementation
    /// of this method currently may require unsafe code. The method is therefore
    /// not offered when compiling with the `no-unsafe` feature.
    fn get_value_ref(&self, now: &EventTag, start: &Instant) -> Option<&T>;
}

/// Something on which we can declare a trigger dependency
/// in the dependency graph.
#[doc(hidden)]
pub trait TriggerLike {
    fn get_id(&self) -> TriggerId;
}

/// The ID of a trigger component.
#[derive(Eq, PartialEq, Copy, Clone, Hash, Ord, PartialOrd)]
pub struct TriggerId(TriggerIdImpl);

// Historical note: in the past, TriggerId was a newtype over a GlobalId.
// The structure of GlobalId was nice, as it allows us to print nice debug
// info. But it also forces us to use inefficient data structures to get a Map<TriggerId, ...>,
// because ids were not allocated consecutively. We were previously using
// hashmaps, now we use IndexVec.
// Also the structure of GlobalId used to set relatively low
// ceilings on the number of components and reactions of each
// reactor. Previously, we could have max 2^16 (reactions+components)
// per reactor. Now we can have 2^16 reactions per reactor,
// and range(usize) total components.

impl TriggerId {
    pub const STARTUP: TriggerId = TriggerId(0);
    pub const SHUTDOWN: TriggerId = TriggerId(1);

    pub(crate) const FIRST_REGULAR: TriggerId = TriggerId(2);

    #[allow(unused)]
    pub(crate) fn new(id: TriggerIdImpl) -> Self {
        assert!(id > 1, "0-1 are reserved for startup & shutdown!");
        TriggerId(id)
    }

    #[allow(unused)]
    pub(crate) fn get_and_incr(&mut self) -> Result<Self, ()> {
        let id = *self;
        *self = id.next()?;
        Ok(id)
    }

    pub(crate) fn next(&self) -> Result<Self, ()> {
        self.0.checked_add(1).map(TriggerId).ok_or(())
    }

    /// Returns an iterator that iterates over the range `(self+1)..(self+1+len)`.
    /// Returns `Err` on overflow.
    pub(crate) fn iter_next_range(&self, len: usize) -> Result<impl Iterator<Item = Self>, ()> {
        if let Some(upper) = self.0.checked_add(1 + (len as TriggerIdImpl)) {
            Ok(((self.0 + 1)..upper).map(TriggerId))
        } else {
            Err(())
        }
    }

    pub(crate) fn next_range(&self, len: usize) -> Result<Range<Self>, ()> {
        if let Some(upper) = self.0.checked_add(1 + (len as TriggerIdImpl)) {
            Ok(Range { start: self.next()?, end: Self::new(upper) })
        } else {
            Err(())
        }
    }

    pub(crate) fn iter_range(range: &Range<TriggerId>) -> impl Iterator<Item = TriggerId> {
        (range.start.0..range.end.0).map(TriggerId)
    }
}

impl Idx for TriggerId {
    fn from_usize(idx: usize) -> Self {
        // note that this is basically an unchecked call to the ctor
        // when Self::new checks
        TriggerId(idx as TriggerIdImpl)
    }

    #[allow(clippy::unnecessary_cast)]
    fn index(self) -> usize {
        // The cast may be unnecessary if TriggerIdImpl resolves
        // to usize, but that depends on compile-time features.
        self.0 as usize
    }
}

impl Debug for TriggerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            TriggerId::STARTUP => write!(f, "startup"),
            TriggerId::SHUTDOWN => write!(f, "shutdown"),
            TriggerId(id) => write!(f, "{}", id),
        }
    }
}
