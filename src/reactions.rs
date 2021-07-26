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

use std::cmp::Ordering;
use std::fmt::*;
use std::hash::{Hash, Hasher};


use crate::{GlobalReactionId, LocalReactionId, LogicalCtx, ReactorId};

/// Wraps a reaction in an "erased" executable form.
/// This wraps a closures, that captures the reactor instance.
pub struct ReactionInvoker {
    /// This is the invocable function.
    /// It needs to be boxed otherwise the struct has no known size.
    body: Box<dyn Fn(&mut LogicalCtx) + Sync + Send>,
    /// Global ID of the reaction, used to test equality of
    /// this reaction with other things.
    id: GlobalReactionId,
}

impl Display for ReactionInvoker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.id, f)
    }
}

impl Debug for ReactionInvoker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Debug>::fmt(&self.id, f)
    }
}

impl PartialOrd for ReactionInvoker {
    /// Reactions are comparable if they're declared in the same reactor.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.id.container == other.id.container {
            Some(self.id.local.cmp(&other.id.local))
        } else {
            None
        }
    }
}

impl ReactionInvoker {
    /// Execute the body of the reaction for the given logical context.
    pub(in super) fn fire(&self, ctx: &mut LogicalCtx) {
        (self.body)(ctx)
    }

    pub(in crate) fn id(&self) -> GlobalReactionId {
        self.id
    }

    /// Create a new reaction invoker that doesn't need a reactor,
    /// ie the invoked code can be arbitrary.
    /// This may be used to test the logic of the scheduler
    pub fn new_from_closure(reactor_id: ReactorId,
                            reaction_index: LocalReactionId,
                            action: impl Fn(&mut LogicalCtx) + Send + Sync + 'static) -> ReactionInvoker {
        ReactionInvoker {
            body: Box::new(action),
            id: GlobalReactionId { container: reactor_id, local: reaction_index },
        }
    }
}


impl PartialEq for ReactionInvoker {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl Eq for ReactionInvoker {}

impl Hash for ReactionInvoker {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

