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

use super::LogicalInstant;

#[doc(hidden)]
pub struct Logical;

#[doc(hidden)]
pub struct Physical;

pub type LogicalAction<T> = Action<Logical, T>;
pub type PhysicalAction<T> = Action<Physical, T>;

pub struct Action<Kind, T> {
    pub min_delay: Duration,
    id: GlobalId,
    is_logical: bool,
    _logical: PhantomData<Kind>,

    /// Stores values of an action for future scheduled events.
    /// We rely strongly on the fact that any value put in there by [Action.schedule_future_value]
    /// will be cleaned up after that tag. Otherwise the map will
    /// blow up the heap.
    ///
    /// A logical instant is the thing about
    ///
    // todo a simple linked list of entries should be simpler and sufficient
    // Most actions probably only need a single cell as a swap.

    map: HashMap<LogicalInstant, Option<T>>,
}

impl<K, T> Action<K, T> {
    /// Record a future value that can be queried at a future logical time.
    /// Note that we don't check that the given time is in the future. If it's
    /// in the past, the value will never be reclaimed.
    ///
    ///
    #[inline]
    pub(in crate) fn schedule_future_value(&mut self, time: LogicalInstant, value: Option<T>) {
        self.map.insert(time, value);
        // todo log when overwriting value
    }


    #[inline]
    pub(in crate) fn forget_value(&mut self, time: &LogicalInstant) {
        self.map.remove(time);
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
            map: Default::default(),
        }
    }
}

impl<T, K> ReactionTrigger<T> for Action<K, T> {
    #[inline]
    fn is_present(&self, now: &LogicalInstant) -> bool {
        self.map.contains_key(now)
    }

    #[inline]
    fn get_value(&self, now: &LogicalInstant) -> Option<T> where T: Copy {
        self.map.get(&now).cloned().flatten()
    }

    #[inline]
    fn use_value_ref<O>(&self, now: &LogicalInstant, action: impl FnOnce(Option<&T>) -> O) -> O {
        let inmap: Option<&Option<T>> = self.map.get(now);
        let v = inmap.map(|i| i.as_ref()).flatten();
        action(v)
    }
}


impl<T> LogicalAction<T> {
    pub fn new(id: GlobalId, min_delay: Option<Duration>) -> Self {
        Self::new_impl(id, min_delay, true)
    }
}

impl<T> PhysicalAction<T> {
    pub fn new(id: GlobalId, min_delay: Option<Duration>) -> Self {
        Self::new_impl(id, min_delay, false)
    }
}

impl<K, T> TriggerLike for Action<K, T> {
    fn get_id(&self) -> TriggerId {
        TriggerId(self.id)
    }
}

/*#[cfg(test)] //fixme
mod test {
    use ActionPresence::{NotPresent, Present};

    use crate::*;

    #[test]
    fn a_value_map_should_be_able_to_store_a_value() {
        let mut vmap = Action::<i64>::default();
        let fut = LogicalInstant::now() + Duration::from_millis(500);
        assert_eq!(NotPresent, vmap.get_value(fut));
        vmap.schedule(fut, Some(2555));
        assert_eq!(Present(Some(2555)), vmap.get_value(fut));
        assert_eq!(Present(Some(2555)), vmap.get_value(fut)); // not deleted
        vmap.schedule(fut, Some(16));
        assert_eq!(Present(Some(16)), vmap.get_value(fut));
        vmap.schedule(fut, None);
        assert_eq!(Present(None), vmap.get_value(fut));
        vmap.forget(fut);
        assert_eq!(NotPresent, vmap.get_value(fut));
    }

    #[test]
    fn a_value_map_should_be_able_to_forget_a_value() {
        let mut vmap = ValueMap::<i64>::default();
        let fut = LogicalInstant::now() + Duration::from_millis(500);
        vmap.schedule(fut, Some(2555));
        assert_eq!(Present(Some(2555)), vmap.get_value(fut));
        vmap.forget(fut);
        assert_eq!(NotPresent, vmap.get_value(fut));
    }

    #[test]
    fn a_value_map_should_be_able_to_store_more_values() {
        let mut vmap = ValueMap::<i64>::default();
        let fut = LogicalInstant::now() + Duration::from_millis(500);
        let fut2 = LogicalInstant::now() + Duration::from_millis(540);
        let fut3 = LogicalInstant::now() + Duration::from_millis(560);

        vmap.schedule(fut, Some(1));
        // order is reversed on purpose
        vmap.schedule(fut3, Some(3));
        vmap.schedule(fut2, Some(2));

        assert_eq!(Present(Some(1)), vmap.get_value(fut));
        assert_eq!(Present(Some(2)), vmap.get_value(fut2));
        assert_eq!(Present(Some(3)), vmap.get_value(fut3));
    }
}
*/
