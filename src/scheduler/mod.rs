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
use std::cell::Cell;
use std::sync::{Arc, Mutex};

pub use assembly::*;
pub use context::*;
pub(in self) use event_queue::*;
pub use scheduler_impl::*;

use crate::LogicalInstant;

use self::depgraph::ExecutableReactions;

/// The internal cell type used to store a thread-safe mutable logical time value.
type TimeCell = Arc<Mutex<Cell<LogicalInstant>>>;

/// A set of reactions to execute at a particular tag.
/// The key characteristic of instances is
/// 1. they may be merged together (by a [DataflowInfo]).
/// 2. merging two plans eliminates duplicates
#[derive(Debug)]
pub(in self) struct Event<'x> {
    pub(in self) reactions: Cow<'x, ExecutableReactions>,
    pub(in self) tag: LogicalInstant,
}

mod context;
mod scheduler_impl;
mod event_queue;
mod depgraph;
mod assembly;


#[inline]
pub(in self) fn display_tag_impl(initial_time: LogicalInstant, tag: LogicalInstant) -> String {
    let elapsed = tag.instant - initial_time.instant;
    format!("(T0 + {} ns = {} ms, {})", elapsed.as_nanos(), elapsed.as_millis(), tag.microstep)
}


