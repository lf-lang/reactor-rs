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


use crate::{LogicalInstant, TriggerId};

/// Common trait for actions, ports, and timer objects handed
/// to reaction functions. This is meant to be used through the
/// API of [ReactionCtx] instead of directly.
pub trait ReactionTrigger<T> {
    /// Returns whether the trigger is present, given that
    /// the current logical time is the parameter.
    #[inline]
    fn is_present(&self, now: &LogicalInstant, start: &LogicalInstant) -> bool {
        self.use_value_ref(now, start, |opt| opt.is_some())
    }

    /// Copies the value out, if it is present. Whether a *value*
    /// is present is not in general the same thing as whether *this trigger*
    /// [Self::is_present]. See [ReactionCtx::get].
    fn get_value(&self, now: &LogicalInstant, start: &LogicalInstant) -> Option<T> where T: Copy;

    /// Execute an action using the current value of this trigger.
    /// The closure is called even if the value is absent (with a [None]
    /// argument).
    fn use_value_ref<O>(&self, now: &LogicalInstant, start: &LogicalInstant, action: impl FnOnce(Option<&T>) -> O) -> O;
}

/// Something on which we can declare a trigger dependency
/// in the dependency graph.
#[doc(hidden)]
pub trait TriggerLike {
    fn get_id(&self) -> TriggerId;
}

