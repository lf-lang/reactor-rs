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

use std::fmt::*;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use super::{ToposortedReactions, LogicalInstant, Named};
use crate::{LogicalCtx, ReactionInvoker, ReactorId};
use std::sync::Arc;
use crate::Offset::After;

#[doc(hidden)]
pub struct Logical;

#[doc(hidden)]
pub struct Physical;

pub type LogicalAction = Action<Logical>;
pub type PhysicalAction = Action<Physical>;

pub struct Action<Logical> {
    delay: Duration,
    pub(in super) downstream: ToposortedReactions,
    name: &'static str,
    is_logical: bool,
    _logical: PhantomData<Logical>,
}

impl<T> Action<T> {
    pub fn set_downstream(&mut self, r: ToposortedReactions) {
        self.downstream = r
    }

    /// Compute the logical time at which an action must be scheduled
    ///
    ///
    pub fn make_eta(&self, now: LogicalInstant, additional_delay: Duration) -> LogicalInstant {
        let min_delay = self.delay + additional_delay;
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
            delay: min_delay.unwrap_or(Duration::new(0, 0)),
            is_logical,
            downstream: Default::default(),
            name,
            _logical: PhantomData,
        }
    }
}

impl LogicalAction {
    pub fn new(name: &'static str, min_delay: Option<Duration>) -> Self {
        Self::new_impl(min_delay, true, name)
    }
}

impl PhysicalAction {
    pub fn new(name: &'static str, min_delay: Option<Duration>) -> Self {
        Self::new_impl(min_delay, false, name)
    }
}

impl<T> Named for Action<T> {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl<T> Display for Action<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
    }
}

/// A timer is conceptually a logical action that re-schedules
/// itself periodically.
pub struct Timer {
    // A reaction that reschedules this
    reschedule: Option<Arc<ReactionInvoker>>,
    name: &'static str,
    offset: Duration,
    period: Duration,
}


impl Named for Timer {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl Timer {
    pub fn new(name: &'static str, offset: Duration, period: Duration) -> Self {
        Self {
            offset,
            period,
            name,
            reschedule: None,
        }
    }

    pub(in crate) fn make_reschedule_reaction(&mut self, rid: ReactorId) -> Arc<ReactionInvoker> {
        let mut action = LogicalAction::new(self.name, None);
        // action.set_downstream(r);
        let period = self.period.clone();
        let schedule_myself = move |ctx: &mut LogicalCtx| {
            ctx.schedule(&action, After(period))
        };
        return Arc::new(ReactionInvoker::new_from_closure(rid, 1000, schedule_myself));
    }
}
