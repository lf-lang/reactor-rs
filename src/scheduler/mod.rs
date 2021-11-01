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
use std::time::Instant;

use index_vec::IndexVec;

pub use context::*;
pub use events::*;
pub use scheduler_impl::*;

use crate::*;

use self::dependencies::ExecutableReactions;

pub(crate) mod assembly_impl;
mod context;
mod dependencies;
mod events;
mod scheduler_impl;

pub(self) type ReactionPlan<'x> = Option<Cow<'x, ExecutableReactions<'x>>>;
pub(self) type ReactorBox<'a> = Box<dyn ReactorBehavior + 'a + Send>;
pub(self) type ReactorVec<'a> = IndexVec<ReactorId, ReactorBox<'a>>;

/// Can format stuff for trace messages.
#[derive(Clone)]
pub(self) struct DebugInfoProvider<'a> {
    id_registry: &'a DebugInfoRegistry,
    initial_time: Instant,
}

impl DebugInfoProvider<'_> {
    pub(self) fn display_event(&self, evt: &Event) -> String {
        use std::fmt::*;

        match evt {
            Event {
                tag,
                reactions,
                terminate,
            } => {
                let mut str = format!("at {}: run [", tag);

                if let Some(reactions) = reactions {
                    for (layer_no, batch) in reactions.batches() {
                        write!(str, "{}: ", layer_no).unwrap();
                        join_to!(&mut str, batch.iter(), ", ", "{", "}", |x| format!(
                            "{}",
                            self.display_reaction(*x)
                        ))
                        .unwrap();
                    }
                }

                str += "]";
                if *terminate {
                    str += ", then terminate"
                }
                str += "";
                str
            }
        }
    }

    #[inline]
    pub(self) fn display_reaction<'a>(&'a self, id: GlobalReactionId) -> impl Display + 'a {
        self.id_registry.fmt_reaction(id)
    }
}
