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

use std::collections::HashMap;
use std::fmt::*;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use crate::*;
use crate::ActionPresence::NotPresent;

use super::{LogicalInstant};

#[doc(hidden)]
pub struct Logical;

#[doc(hidden)]
pub struct Physical;

pub type LogicalAction<T> = Action<Logical, T>;
pub type PhysicalAction<T> = Action<Physical, T>;

pub struct Action<Kind, T: Clone> {
    pub min_delay: Duration,
    id: GlobalId,
    is_logical: bool,
    _logical: PhantomData<Kind>,
    // This is only used for an action scheduled with zero delay 
    values: ValueMap<T>,
}

impl<K, T: Clone> Action<K, T> {

    /// Record a future value that can be queried at a future logical time.
    /// Note that we don't check that the given time is in the future. If it's
    /// in the past, the value will never be reclaimed.
    ///
    ///
    #[inline]
    pub(in crate) fn schedule_future_value(&mut self, time: LogicalInstant, value: Option<T>) {
        self.values.schedule(time, value)
    }

    #[inline]
    pub(in crate) fn get_value(&self, time: LogicalInstant) -> Option<T> {
        match self.values.get_value(time) {
            ActionPresence::NotPresent => None,
            ActionPresence::Present(value) => value
        }
    }

    #[inline]
    pub(in crate) fn is_present(&self, time: LogicalInstant) -> bool {
        match self.values.get_value(time) {
            ActionPresence::NotPresent => false,
            ActionPresence::Present(_) => true
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn forget_value(&mut self, time: LogicalInstant) {
        self.values.forget(time)
    }

    /// Compute the logical time at which an action must be scheduled
    ///
    ///
    pub fn make_eta(&self, now: LogicalInstant, additional_delay: Duration) -> LogicalInstant {
        let min_delay = self.min_delay + additional_delay;
        let mut instant = now.instant + min_delay;
        if !self.is_logical {
            // physical actions are adjusted to physical time if needed
            instant = Instant::max(instant, Instant::now());
        }

        let microstep = if instant == now.instant { now.microstep + 1 } else { MicroStep::ZERO };

        LogicalInstant {
            instant,
            microstep,
        }
    }

    fn new_impl(id: GlobalId,
                min_delay: Option<Duration>,
                is_logical: bool) -> Self {
        Action {
            min_delay: min_delay.unwrap_or(Duration::new(0, 0)),
            is_logical,
            id,
            _logical: PhantomData,
            values: Default::default(),
        }
    }
}

impl<T: Clone> LogicalAction<T> {
    pub fn new(id: GlobalId, min_delay: Option<Duration>) -> Self {
        Self::new_impl(id, min_delay, true)
    }
}

impl<T: Clone> PhysicalAction<T> {
    pub fn new(id: GlobalId, min_delay: Option<Duration>) -> Self {
        Self::new_impl(id, min_delay, false)
    }
}

impl<K, T: Clone> TriggerLike for Action<K, T> {
    fn get_id(&self) -> TriggerId {
        TriggerId(self.id)
    }
}


#[derive(Debug, Eq, PartialEq)]
pub(in crate) enum ActionPresence<T> {
    /// Action was not scheduled
    NotPresent,
    /// Action was scheduled, but value may be missing.
    Present(Option<T>),
}

impl<T: Clone> Clone for ActionPresence<T> {
    fn clone(&self) -> Self {
        match self {
            ActionPresence::NotPresent => ActionPresence::NotPresent,
            ActionPresence::Present(o) => ActionPresence::Present(o.clone())
        }
    }
}

/// Stores values of an action for future scheduled events.
/// We rely strongly on the fact that any value put in there by [Action.schedule_future_value]
/// will be cleaned up after that tag. Otherwise the map will
/// blow up.
pub(in crate) struct ValueMap<T: Clone> {
    // todo a simple linked list of entries should be simpler and sufficient
    // Most actions probably only need a single cell as a swap.
    map: HashMap<LogicalInstant, ActionPresence<T>>,
}

impl<T: Clone> ValueMap<T> {
    pub(in crate) fn get_value(&self, time: LogicalInstant) -> ActionPresence<T> {
        self.map.get(&time).cloned().unwrap_or(NotPresent)
    }

    pub(in crate) fn forget(&mut self, time: LogicalInstant) {
        self.map.remove(&time);
    }

    pub(in crate) fn schedule(&mut self, time: LogicalInstant, value: Option<T>) {
        self.map.insert(time, ActionPresence::Present(value));
        // todo log when overwriting value
    }
}

impl<T: Clone> Default for ValueMap<T> {
    fn default() -> Self {
        Self { map: Default::default() }
    }
}
