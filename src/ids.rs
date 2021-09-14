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


use std::fmt::{Display, Formatter, Result, Debug};
use bit_set::BitSet;
use std::iter::FromIterator;

// private implementation types
type ReactionIdImpl = u16;
type ReactorIdImpl = u16;
pub(in crate) type GlobalIdImpl = u32;

define_index_type! {
    /// Type of a local reaction ID
    pub struct LocalReactionId = ReactionIdImpl;
    DISABLE_MAX_INDEX_CHECK = cfg!(not(debug_assertions));
    DISPLAY_FORMAT = "{}";
}

impl LocalReactionId {
    // a const fn to be able to use this in const context
    pub const fn new_const(u: ReactionIdImpl) -> Self {
        Self { _raw: u }
    }
}

/// A set of reactions all belonging to the same reactor.
/// The [ReactorId] is not stored within this struct.
pub(in crate) struct LocalizedReactionSet {
    set: BitSet,
}

impl LocalizedReactionSet {
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    pub fn insert(&mut self, id: LocalReactionId) -> bool {
        self.set.insert(id.index())
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item=LocalReactionId> + 'a {
        self.set.iter().map(Into::into)
    }
}

impl FromIterator<LocalReactionId> for LocalizedReactionSet {
    fn from_iter<T: IntoIterator<Item=LocalReactionId>>(iter: T) -> Self {
        let mut result = Self { set: BitSet::with_capacity(32) };
        for t in iter {
            result.insert(t);
        }
        result
    }
}


define_index_type! {
    /// The unique identifier of a reactor instance during
    /// execution.
    pub struct ReactorId = ReactorIdImpl;
    DISABLE_MAX_INDEX_CHECK = cfg!(not(debug_assertions));
    DISPLAY_FORMAT = "{}";
    DEFAULT = Self::new(0);
}

impl ReactorId {
    // a const fn to be able to use this in const context
    pub const fn new_const(u: ReactionIdImpl) -> Self {
        Self { _raw: u }
    }
}

// fixme this is a dup of GlobalId
/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Copy, Clone)]
pub struct GlobalReactionId {
    pub(in crate) container: ReactorId,
    pub(in crate) local: LocalReactionId,
}


impl GlobalReactionId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        Self { container, local }
    }
}

impl Debug for GlobalReactionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}/{}", self.container, self.local)
    }
}

impl Display for GlobalReactionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}

/// todo ensure it is toposorted
pub type ReactionSet = Vec<GlobalReactionId>;


/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Debug, Copy, Clone)]
pub struct GlobalId {
    pub(in crate) container: ReactorId,
    pub(in crate) local: LocalReactionId,
}


impl GlobalId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        Self { container, local }
    }

    pub(in crate) fn as_u32(&self) -> GlobalIdImpl {
        unsafe {
            std::mem::transmute_copy(&self)
        }
        //(self.container._raw as u32) << 16 | self.local._raw
    }
    #[cfg(test)]
    pub const fn next_id(&self) -> GlobalId {
        Self { container: self.container, local: LocalReactionId::new_const(self.local._raw + 1) }
    }
    #[cfg(test)]
    pub const fn first_id() -> GlobalId {
        GlobalId {
            container: ReactorId::new_const(0),
            local: LocalReactionId::new_const(0),
        }
    }
}

impl Display for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}/{}", self.container, self.local)
    }
}


pub(in crate) trait GloballyIdentified {
    fn get_id(&self) -> GlobalId;
}


pub type PortId = GlobalId;
