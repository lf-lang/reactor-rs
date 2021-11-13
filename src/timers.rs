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
use std::time::Instant;

use super::*;
use crate::assembly::{TriggerId, TriggerLike};

/// A timer is conceptually a logical action that may re-schedule
/// itself periodically.
///
/// For periodic timers, a reaction is synthesized which reschedules
/// the timer.
pub struct Timer {
    id: TriggerId,

    /// Minimal duration after the start of the program after
    /// which the timer starts to trigger.
    pub offset: Duration,

    /// Period between events emitted by this timer. A period
    /// of zero means that the timer will trigger exactly once
    /// after the specified offset.
    pub period: Duration,
}

impl Timer {
    pub(crate) fn new(id: TriggerId, offset: Duration, period: Duration) -> Self {
        Self { offset, period, id }
    }

    /// Whether the timer should repeat itself. A period of zero
    /// means that the timer will trigger exactly once after the
    /// specified offset.
    #[inline]
    pub fn is_periodic(&self) -> bool {
        !self.period.is_zero()
    }
}

impl TriggerLike for Timer {
    fn get_id(&self) -> TriggerId {
        self.id
    }
}

impl ReactionTrigger<()> for Timer {
    fn is_present(&self, now: &EventTag, _start: &Instant) -> bool {
        let elapsed = now.duration_since_start();
        if elapsed == self.offset {
            true
        } else if elapsed < self.offset || !self.is_periodic() {
            false
        } else {
            ((elapsed - self.offset).as_nanos() % self.period.as_nanos()) == 0
        }
    }

    #[inline]
    fn get_value(&self, now: &EventTag, start: &Instant) -> Option<()> {
        if self.is_present(now, start) {
            Some(())
        } else {
            None
        }
    }

    #[inline]
    fn use_value_ref<O>(&self, now: &EventTag, start: &Instant, action: impl FnOnce(Option<&()>) -> O) -> O {
        if self.is_present(now, start) {
            action(Some(&()))
        } else {
            action(None)
        }
    }
}
