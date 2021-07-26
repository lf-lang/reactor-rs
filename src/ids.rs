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


use std::fmt::{Display, Formatter, Result};

/// Type of a local reaction ID
pub type LocalReactionId = u16;

define_index_type! {
    pub struct ReactorId = u16;

    // We can also disable checking all-together if we are more concerned with perf
    // than any overflow problems, or even do so, but only for debug builds
    DISABLE_MAX_INDEX_CHECK = cfg!(not(debug_assertions));
}

impl Display for ReactorId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self._raw)
    }
}

impl Default for ReactorId {
    #[inline]
    fn default() -> Self {
        Self::new(0)
    }
}

/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Debug, Copy, Clone)]
pub struct GlobalReactionId {
    pub(in crate) container: ReactorId,
    pub(in crate) local: LocalReactionId,
}


impl GlobalReactionId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        Self { container, local }
    }
}

impl Display for GlobalReactionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}/{}", self.container, self.local)
    }
}

/// todo ensure it is toposorted
pub type ReactionSet = Vec<GlobalReactionId>;

