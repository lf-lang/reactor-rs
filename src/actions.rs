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

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::*;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use super::{LogicalInstant, Named, ToposortedReactions};

#[doc(hidden)]
pub struct Logical;

#[doc(hidden)]
pub struct Physical;

pub type LogicalAction<T> = Action<Logical, T>;
pub type PhysicalAction<T> = Action<Physical, T>;

pub struct Action<Kind, T: Clone> {
    pub min_delay: Duration,
    pub(in super) downstream: ToposortedReactions,
    name: &'static str,
    is_logical: bool,
    _logical: PhantomData<Kind>,
    // This is only used for an action scheduled with zero delay 
    cell: RefCell<ValueMap<T>>,
}

impl<K, T: Clone> Action<K, T> {
    pub fn set_downstream(&mut self, r: ToposortedReactions) {
        self.downstream = r
    }

    /// Record a future value that can be queried at a future logical time.
    /// Note that we don't check that the given time is in the future. If it's
    /// in the past, the value will never be reclaimed.
    pub(in crate) fn schedule_future_value(&self, time: LogicalInstant, value: Option<T>) {
        self.cell.borrow_mut().schedule(time, value)
    }

    pub(in crate) fn get_value(&self, time: LogicalInstant) -> Option<T> {
        self.cell.borrow().get_value(time).cloned()
    }

    pub(in crate) fn forget_value(&self, time: LogicalInstant) {
        self.cell.borrow_mut().forget(time)
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

        LogicalInstant {
            instant,
            microstep: now.microstep + 1,
        }
    }

    fn new_impl(
        min_delay: Option<Duration>,
        is_logical: bool,
        name: &'static str) -> Self {
        Action {
            min_delay: min_delay.unwrap_or(Duration::new(0, 0)),
            is_logical,
            downstream: Default::default(),
            name,
            _logical: PhantomData,
            cell: RefCell::default(),
        }
    }
}

impl<T: Clone> LogicalAction<T> {
    pub fn new(name: &'static str, min_delay: Option<Duration>) -> Self {
        Self::new_impl(min_delay, true, name)
    }
}

impl<T: Clone> PhysicalAction<T> {
    pub fn new(name: &'static str, min_delay: Option<Duration>) -> Self {
        Self::new_impl(min_delay, false, name)
    }
}

impl<K, T: Clone> Named for Action<K, T> {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl<K, T: Clone> Display for Action<K, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
    }
}

/// Stores values of an action for future scheduled events.
/// We rely strongly on the fact that any value put in there by [Action.schedule_future_value]
/// will be cleaned up after that tag. Otherwise the map will
/// blow up.
pub(in crate) struct ValueMap<T> {
    // todo a simple linked list of entries should be simpler and sufficient
    // Most actions probably only need a single cell as a swap.
    map: HashMap<LogicalInstant, T>,
}

impl<T> ValueMap<T> {
    pub(in crate) fn get_value(&self, time: LogicalInstant) -> Option<&T> {
        self.map.get(&time)
    }

    pub(in crate) fn forget(&mut self, time: LogicalInstant) {
        self.map.remove(&time);
    }

    pub(in crate) fn schedule(&mut self, time: LogicalInstant, value: Option<T>) {
        match value {
            None => self.map.remove(&time),
            Some(value) => self.map.insert(time, value)
        };
    }
}

impl<T> Default for ValueMap<T> {
    fn default() -> Self {
        Self { map: Default::default() }
    }
}
