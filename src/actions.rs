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

use std::cmp::Reverse;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::assembly::{TriggerId, TriggerLike};
use crate::*;

use vecmap::{Entry, VecMap};

/// A logical action.
pub struct LogicalAction<T: Sync>(pub(crate) Action<Logical, T>);

/// A physical action. Physical actions may only be used with
/// the API of [AsyncCtx](crate::AsyncCtx).
/// See [ReactionCtx::spawn_physical_thread](crate::ReactionCtx::spawn_physical_thread).
pub struct PhysicalAction<T: Sync>(pub(crate) Action<Physical, T>);

pub(crate) struct Logical;
pub(crate) struct Physical;

pub(crate) struct Action<Kind, T: Sync> {
    pub(crate) min_delay: Duration,
    id: TriggerId,
    // is_logical: bool,
    _logical: PhantomData<Kind>,

    /// Stores values of an action for future scheduled events.
    /// We rely strongly on the fact that any value put in there by [Action.schedule_future_value]
    /// will be cleaned up after that tag. Otherwise the map will
    /// blow up the heap.
    map: VecMap<Reverse<EventTag>, Option<T>>,
}

impl<K, T: Sync> Action<K, T> {
    /// Record a future value that can be queried at a future logical time.
    /// Note that we don't check that the given time is in the future. If it's
    /// in the past, the value will never be reclaimed.
    ///
    ///
    #[inline]
    pub(crate) fn schedule_future_value(&mut self, time: EventTag, value: Option<T>) {
        match self.map.entry(Reverse(time)) {
            Entry::Vacant(e) => e.insert(value),
            Entry::Occupied(ref mut e) => {
                trace!("Value overwritten in an action for tag {}", time);
                trace!("This means an action was scheduled several times for the same tag.");
                e.replace(value)
            }
        }
    }

    #[inline]
    pub(crate) fn forget_value(&mut self, time: &EventTag) -> Option<T> {
        self.map.remove(&Reverse(*time)).flatten()
    }

    fn new_impl(id: TriggerId, min_delay: Option<Duration>, _is_logical: bool) -> Self {
        Action {
            min_delay: min_delay.unwrap_or(Duration::ZERO),
            // is_logical,
            id,
            _logical: PhantomData,
            map: VecMap::new(),
        }
    }
}

impl<T: Sync, K> ReactionTrigger<T> for Action<K, T> {
    #[inline]
    fn is_present(&self, now: &EventTag, _start: &Instant) -> bool {
        self.map.contains_key(&Reverse(*now))
    }

    #[inline]
    fn get_value(&self, now: &EventTag, _start: &Instant) -> Option<T>
    where
        T: Copy,
    {
        self.map.get(&Reverse(*now)).cloned().flatten()
    }

    #[inline]
    fn use_value_ref<O>(&self, now: &EventTag, _start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O {
        let inmap: Option<&Option<T>> = self.map.get(&Reverse(*now));
        let v = inmap.and_then(|i| i.as_ref());
        action(v)
    }
}

#[cfg(not(feature = "no-unsafe"))]
impl<T: Sync, K> triggers::ReactionTriggerWithRefAccess<T> for Action<K, T> {
    fn get_value_ref(&self, now: &EventTag, _start: &Instant) -> Option<&T> {
        self.map.get(&Reverse(*now)).map(|a| a.as_ref()).flatten()
    }
}

impl<T: Sync> ReactionTrigger<T> for LogicalAction<T> {
    #[inline]
    fn is_present(&self, now: &EventTag, start: &Instant) -> bool {
        self.0.is_present(now, start)
    }

    #[inline]
    fn get_value(&self, now: &EventTag, start: &Instant) -> Option<T>
    where
        T: Copy,
    {
        self.0.get_value(now, start)
    }

    #[inline]
    fn use_value_ref<O>(&self, now: &EventTag, start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O {
        self.0.use_value_ref(now, start, action)
    }
}

#[cfg(not(feature = "no-unsafe"))]
impl<T: Sync> triggers::ReactionTriggerWithRefAccess<T> for LogicalAction<T> {
    fn get_value_ref(&self, now: &EventTag, start: &Instant) -> Option<&T> {
        self.0.get_value_ref(now, start)
    }
}

impl<T: Sync> LogicalAction<T> {
    pub(crate) fn new(id: TriggerId, min_delay: Option<Duration>) -> Self {
        Self(Action::new_impl(id, min_delay, true))
    }
}

impl<T: Sync> PhysicalAction<T> {
    fn new(id: TriggerId, min_delay: Option<Duration>) -> Self {
        Self(Action::new_impl(id, min_delay, false))
    }
}

impl<T: Sync> TriggerLike for PhysicalAction<T> {
    fn get_id(&self) -> TriggerId {
        self.0.id
    }
}

impl<T: Sync> TriggerLike for LogicalAction<T> {
    fn get_id(&self) -> TriggerId {
        self.0.id
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

/// A reference to a physical action. This thing is cloneable
/// and can be sent to async threads. The contained action
/// reference is unique and protected by a lock. All operations
/// on the action are
///
/// See [crate::ReactionCtx::spawn_physical_thread].
#[derive(Clone)]
pub struct PhysicalActionRef<T: Sync>(Arc<Mutex<PhysicalAction<T>>>);

impl<T: Sync> PhysicalActionRef<T> {
    pub(crate) fn new(id: TriggerId, min_delay: Option<Duration>) -> Self {
        Self(Arc::new(Mutex::new(PhysicalAction::new(id, min_delay))))
    }

    pub(crate) fn use_mut<O>(&self, f: impl FnOnce(&mut PhysicalAction<T>) -> O) -> Result<O, ()> {
        let mut refmut = self.0.deref().lock().map_err(|_| ())?;

        Ok(f(refmut.deref_mut()))
    }

    pub(crate) fn use_mut_p<O, P>(&self, p: P, f: impl FnOnce(&mut PhysicalAction<T>, P) -> O) -> Result<O, P> {
        match self.0.deref().lock() {
            Ok(mut refmut) => Ok(f(refmut.deref_mut(), p)),
            Err(_) => Err(p),
        }
    }

    pub(crate) fn use_value<O>(&self, f: impl FnOnce(&PhysicalAction<T>) -> O) -> Result<O, ()> {
        let r#ref = self.0.deref().lock().map_err(|_| ())?;

        Ok(f(r#ref.deref()))
    }
}

impl<T: Sync> TriggerLike for PhysicalActionRef<T> {
    fn get_id(&self) -> TriggerId {
        self.use_value(|a| a.get_id()).unwrap()
    }
}

impl<T: Sync> ReactionTrigger<T> for PhysicalActionRef<T> {
    fn is_present(&self, now: &EventTag, start: &Instant) -> bool {
        self.use_value(|a| a.0.is_present(now, start)).unwrap()
    }

    fn get_value(&self, now: &EventTag, start: &Instant) -> Option<T>
    where
        T: Copy,
    {
        self.use_value(|a| a.0.get_value(now, start)).unwrap()
    }

    fn use_value_ref<O>(&self, now: &EventTag, start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O {
        self.use_value(|a| a.0.use_value_ref(now, start, action)).unwrap()
    }
}
