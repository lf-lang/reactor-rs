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




use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use crate::TriggerId;

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
    pub const ZERO: LocalReactionId = LocalReactionId::new_const(0);

    // a const fn to be able to use this in const context
    pub const fn new_const(u: ReactionIdImpl) -> Self {
        Self { _raw: u }
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
    pub const fn new_const(u: ReactorIdImpl) -> Self {
        Self { _raw: u }
    }
}

macro_rules! global_id_newtype {
    {$(#[$m:meta])* $id:ident} => {
        $(#[$m])*
        #[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Copy, Clone)]
        pub struct $id(pub(in crate) GlobalId);

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
#[derive(Eq, Ord, PartialOrd, PartialEq, Copy, Clone)]
pub(crate) struct GlobalId {
    _raw: GlobalIdImpl,
}


impl GlobalId {
    pub fn new(container: ReactorId, local: LocalReactionId) -> Self {
        let _raw: GlobalIdImpl = (container._raw as GlobalIdImpl) << ReactionIdImpl::BITS | (local._raw as GlobalIdImpl);
        Self { _raw }
    }

    pub(in crate) const fn container(&self) -> ReactorId {
        ReactorId::new_const((self._raw >> 16) as u16)
    }

    pub(in crate) const fn local(&self) -> LocalReactionId {
        LocalReactionId::new_const((self._raw & 0xffff) as u16)
    }
}

impl FromStr for GlobalId {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((container, local)) = s.split_once('/') {
            let container = container.parse::<ReactorIdImpl>().map_err(|_| "invalid reactor id")?;
            let local = local.parse::<ReactionIdImpl>().map_err(|_| "invalid local id")?;
            Ok(GlobalId::new(ReactorId::from_raw(container),
                             LocalReactionId::from_raw(local)))
        } else {
            Err("Expected format {int}/{int}")
        }
    }
}

// todo commit and remove
#[cfg(nightly)]
impl std::iter::Step for GlobalId {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        (end._raw as usize).checked_sub(start._raw as usize)
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        GlobalIdImpl::try_from(count).ok()
            .and_then(|c| start._raw.checked_add(c))
            .map(|_raw| Self { _raw })
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        GlobalIdImpl::try_from(count).ok()
            .and_then(|c| start._raw.checked_sub(c))
            .map(|_raw| Self { _raw })
    }
}

impl Hash for GlobalId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self._raw)
    }
}

impl Debug for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Display>::fmt(self, f)
    }
}

impl Display for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}/{}", self.container(), self.local())
    }
}


pub(crate) type PortId = TriggerId;
