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
use std::fmt::Display;

pub use context::*;
pub use events::*;
use index_vec::IndexVec;
pub use scheduler_impl::*;

use self::dependencies::ExecutableReactions;
use crate::*;

pub(crate) mod assembly_impl;
mod context;
pub(crate) mod debug;
mod dependencies;
mod events;
mod scheduler_impl;

#[cfg(feature = "public-internals")]
pub mod internals {
    pub use super::dependencies::{ExecutableReactions, Level, LevelIx, ReactionLevelInfo};
}

pub(self) type ReactionPlan<'x> = Option<Cow<'x, ExecutableReactions<'x>>>;
pub(self) type ReactorBox<'a> = Box<dyn ReactorBehavior + 'a>;
pub(self) type ReactorVec<'a> = IndexVec<ReactorId, ReactorBox<'a>>;

/// Can format stuff for trace messages.
#[derive(Clone)]
pub(self) struct DebugInfoProvider<'a> {
    id_registry: &'a DebugInfoRegistry,
}

impl DebugInfoProvider<'_> {
    pub(self) fn display_event(&self, evt: &Event) -> String {
        let Event { tag, reactions, terminate } = evt;
        let mut str = format!("at {}: run {}", tag, self.display_reactions(reactions));

        if *terminate {
            str += ", then terminate"
        }
        str
    }

    pub(self) fn display_reactions(&self, reactions: &ReactionPlan) -> String {
        use std::fmt::*;

        let mut str = "[".to_string();

        if let Some(reactions) = reactions {
            for (level_no, batch) in reactions.batches() {
                write!(str, "{}: ", level_no).unwrap();
                join_to!(&mut str, batch.iter(), ", ", "{", "}", |x| format!(
                    "{}",
                    self.display_reaction(x)
                ))
                .unwrap();
            }
        }

        str += "]";
        str
    }

    #[inline]
    pub(self) fn display_reaction(&self, id: GlobalReactionId) -> impl Display + '_ {
        self.id_registry.fmt_reaction(id)
    }
}
