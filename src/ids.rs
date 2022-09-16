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
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use impl_types::{GlobalIdImpl, ReactionIdImpl, ReactorIdImpl};
use index_vec::Idx;

macro_rules! simple_idx_type {
    ($(#[$($attrs:tt)*])* $id:ident($impl_t:ty)) => {

$(#[$($attrs)*])*
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct $id($impl_t);

impl $id {
    // a const fn to be able to use this in const context
    pub const fn new(u: $impl_t) -> Self {
        Self(u)
    }

    pub const fn raw(self) -> $impl_t {
        self.0
    }

    pub(crate) fn plus(&self, u: usize) -> Self {
        Self::from_usize(self.0 as usize + u)
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }

    #[allow(unused)]
    pub(crate) fn get_and_incr(&mut self) -> Self {
        let id = *self;
        *self = Self(self.0 + 1);
        id
    }
}

impl Idx for $id {
    fn from_usize(idx: usize) -> Self {
        debug_assert!(idx <= <$impl_t>::MAX as usize);
        Self(idx as $impl_t)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

impl Display for $id {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}
    };
}

simple_idx_type! {
    /// ID of a reaction local to its containing reactor.
    LocalReactionId(ReactionIdImpl)
}

simple_idx_type! {
    /// The unique identifier of a reactor instance during
    /// execution.
    ReactorId(ReactorIdImpl)
}

macro_rules! global_id_newtype {
    {$(#[$m:meta])* $id:ident} => {
        $(#[$m])*
        #[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Copy, Clone)]
        pub struct $id(pub(crate) GlobalId);

        impl $id {
            pub fn new(container: $crate::ReactorId, local: $crate::LocalReactionId) -> Self {
                Self($crate::GlobalId::new(container, local))
            }
        }

        impl Debug for $id {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }

        impl Display for $id {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

global_id_newtype! {
    /// Global identifier for a reaction.
    GlobalReactionId
}

/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Copy, Clone)]
pub(crate) struct GlobalId {
    container: ReactorId,
    local: LocalReactionId,
}

impl GlobalId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        Self { container, local }
    }

    #[allow(unused)]
    pub(crate) fn from_raw(u: GlobalIdImpl) -> Self {
        unsafe { std::mem::transmute(u) }
    }

    pub(crate) const fn container(&self) -> ReactorId {
        self.container
    }

    pub(crate) const fn local(&self) -> LocalReactionId {
        self.local
    }
}

impl FromStr for GlobalId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((container, local)) = s.split_once('/') {
            let container = container.parse::<ReactorIdImpl>().map_err(|_| "invalid reactor id")?;
            let local = local.parse::<ReactionIdImpl>().map_err(|_| "invalid local id")?;
            Ok(GlobalId::new(ReactorId::new(container), LocalReactionId::new(local)))
        } else {
            Err("Expected format {int}/{int}")
        }
    }
}

// Hashing global ids is a very hot operation in the framework,
// therefore we give it an optimal implementation.
// The implementation was verified to be faster than the default
// derive by a micro benchmark in this repo.
impl Hash for GlobalId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let as_impl: &GlobalIdImpl = unsafe { std::mem::transmute(self) };
        // this is written so that it works regardless of the concrete type of GlobalIdImpl
        Hash::hash(as_impl, state);
    }
}

// Since Hash was implemented explicitly, we have to do it for PartialEq as well.
impl PartialEq for GlobalId {
    fn eq(&self, other: &Self) -> bool {
        let self_impl: &GlobalIdImpl = unsafe { std::mem::transmute(self) };
        let other_impl: &GlobalIdImpl = unsafe { std::mem::transmute(other) };
        self_impl == other_impl
    }
}

// Same reasoning as for Hash, comparison is used to keep Level
// sets sorted, when feature `vec-id-sets` is enabled.
impl Ord for GlobalId {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_as_impl: &GlobalIdImpl = unsafe { std::mem::transmute(self) };
        let other_as_impl: &GlobalIdImpl = unsafe { std::mem::transmute(other) };
        self_as_impl.cmp(other_as_impl)
    }
}

impl PartialOrd for GlobalId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Debug for GlobalId {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Display>::fmt(self, f)
    }
}

impl Display for GlobalId {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}/{}", self.container(), self.local())
    }
}

/// private implementation types
pub(crate) mod impl_types {
    cfg_if! {
        if #[cfg(all(target_pointer_width = "64", feature = "wide-ids"))] {
            type MyUsize = usize;
            type HalfUsize = u32;
        } else {
            type MyUsize = u32;
            type HalfUsize = u16;
        }
    }

    pub type TriggerIdImpl = MyUsize;
    pub type ReactionIdImpl = HalfUsize;
    pub type ReactorIdImpl = HalfUsize;
    pub type GlobalIdImpl = MyUsize;
    assert_eq_size!(GlobalIdImpl, (ReactorIdImpl, ReactionIdImpl));
    assert_impl_all!(GlobalIdImpl: petgraph::graph::IndexType);
    assert_impl_all!(ReactorIdImpl: petgraph::graph::IndexType);
}
