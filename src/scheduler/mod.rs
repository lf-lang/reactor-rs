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

use crossbeam::atomic::AtomicCell;
use index_vec::IndexVec;

pub use assembly::*;
pub use context::*;
pub(in self) use event_queue::*;
pub use scheduler_impl::*;

use crate::{LogicalInstant, ReactorBehavior, ReactorId};

use self::depgraph::ExecutableReactions;

mod context;
mod scheduler_impl;
mod event_queue;
mod depgraph;
mod assembly;


/// The internal cell type used to store a thread-safe mutable logical time value.
type TimeCell = AtomicCell<LogicalInstant>;

/// A tagged event of the reactor program. Events are tagged
/// with the logical instant at which they must be processed.
/// They are queued and processed in order. See [self::EventQueue].
///
/// [self::PhysicalSchedulerLink] may only communicate with
/// the scheduler by sending events.
#[derive(Debug)]
pub(in self) struct Event<'x> {
    /// The tag at which the reactions to this event must be executed.
    /// This is always > to the latest *processed* tag, by construction
    /// of the reactor application.
    pub(in self) tag: LogicalInstant,
    /// The set of reactions to execute.
    pub(in self) reactions: Cow<'x, ExecutableReactions>,
}

pub(in self) type ReactorVec<'x> = IndexVec<ReactorId, Box<dyn ReactorBehavior + Send + Sync + 'x>>;

#[inline]
pub(in self) fn display_tag_impl(initial_time: LogicalInstant, tag: LogicalInstant) -> String {
    let elapsed = tag.instant - initial_time.instant;
    format!("(T0 + {} ns = {} ms, {})", elapsed.as_nanos(), elapsed.as_millis(), tag.microstep)
}


