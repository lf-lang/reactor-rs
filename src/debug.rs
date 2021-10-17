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


use core::any::type_name;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result};

use index_vec::IndexVec;

use crate::{GlobalId, GlobalReactionId, ReactorId, ReactorInitializer, TriggerId};

#[derive(Clone)]
pub(in crate) struct ReactorDebugInfo {
    /// Type name
    #[allow(unused)]
    pub type_name: &'static str,
    /// Simple name of the instantiation (last segment of the path)
    #[allow(unused)]
    pub inst_name: &'static str,
    /// Path to this instantiation (eg "/parent/child")
    inst_path: String,
}

impl ReactorDebugInfo {
    pub(in crate) fn root<R>() -> Self {
        Self {
            type_name: type_name::<R>(),
            inst_name: "/",
            inst_path: "/".into(),
        }
    }

    pub(in crate) fn derive<R: ReactorInitializer>(&self, inst_name: &'static str) -> Self {
        Self {
            type_name: type_name::<R::Wrapped>(),
            inst_name,
            inst_path: format!("{}{}/", self.inst_path, inst_name),
        }
    }
}

impl Display for ReactorDebugInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.inst_path)
    }
}


/// Stores a mapping from global Id to debug label
#[derive(Default)]
pub(crate) struct DebugInfoRegistry {
    reactor_infos: IndexVec<ReactorId, ReactorDebugInfo>,
    /// Map of ReactorId to number of components in that reactor
    reactor_sizes: IndexVec<ReactorId, usize>,

    // todo better data structures
    reaction_labels: HashMap<GlobalReactionId, Cow<'static, str>>,
    trigger_infos: HashMap<TriggerId, Cow<'static, str>>,
    trigger_to_global_id: HashMap<TriggerId, GlobalId>,
}

impl DebugInfoRegistry {
    pub fn get_debug_info(&self, id: ReactorId) -> &ReactorDebugInfo {
        &self.reactor_infos[id]
    }

    fn fmt_component_path<'a>(&'a self, id: GlobalId, label: Option<&'a Cow<'static, str>>) -> impl Display + 'a {
        struct Format<'a> {
            debug: &'a DebugInfoRegistry,
            id: GlobalId,
            label: Option<&'a Cow<'static, str>>,
        }
        use std::fmt::*;
        impl Display for Format<'_> {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                if let Some(label) = &self.label {
                    write!(f, "{}{}@{}", self.debug.get_debug_info(self.id.container()), self.id.local(), label)
                } else {
                    write!(f, "{}{}", self.debug.get_debug_info(self.id.container()), self.id.local())
                }
            }
        }

        Format { debug: self, id, label }
    }

    #[inline]
    pub(crate) fn fmt_component<'a>(&'a self, id: TriggerId) -> impl Display + 'a {
        self.fmt_component_path(*self.trigger_to_global_id.get(&id).expect("Id isn't registered!"),
                                self.trigger_infos.get(&id))
    }

    #[inline]
    pub fn fmt_reaction<'a>(&'a self, id: GlobalReactionId) -> impl Display + 'a {
        self.fmt_component_path(id.0, self.reaction_labels.get(&id))
    }

    pub(in super) fn record_trigger(&mut self, id: TriggerId, name: Cow<'static, str>) {
        let existing = self.trigger_infos.insert(id, name);
        debug_assert!(existing.is_none())
    }

    pub(in super) fn record_reaction(&mut self, id: GlobalReactionId, name: Cow<'static, str>) {
        let existing = self.reaction_labels.insert(id, name);
        debug_assert!(existing.is_none())
    }

    pub(in super) fn record_reactor(&mut self, id: ReactorId, debug: &ReactorDebugInfo) {
        let ix = self.reactor_infos.push(debug.clone());
        debug_assert_eq!(ix, id);
    }

    pub(in super) fn set_num_components(&mut self, id: ReactorId, num_components: usize) {
        let ix = self.reactor_sizes.push(num_components);
        debug_assert_eq!(ix, id);
    }
}
