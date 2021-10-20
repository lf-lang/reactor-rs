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
use std::ops::Range;

use index_vec::{Idx, IndexVec};

use crate::{GlobalReactionId, ReactorId, ReactorInitializer, assembly::TriggerId};


/// Maps IDs to debug information, stores all the debug info.
/// This is built during asembly.
pub(crate) struct DebugInfoRegistry {
    /// Maps reactor ids to their debug info.
    reactor_infos: IndexVec<ReactorId, ReactorDebugInfo>,

    /// Maps a ReactorId to the last TriggerId (exclusive) it occupies.
    /// This is used to get the ReactorId back from a TriggerId.
    reactor_bound: IndexVec<ReactorId, TriggerId>,

    /// Labels of each trigger, every trigger id in the program
    /// is registered here.
    trigger_infos: IndexVec<TriggerId, Cow<'static, str>>,

    // todo better data structures
    /// Labels of each reaction, only reactions that have one are in here.
    reaction_labels: HashMap<GlobalReactionId, Cow<'static, str>>,
}

/// The reactor ID, and the local index within the reactor.
/// We don't use GlobalId because the second component is not
/// a LocalReactionId, for trigger ids it may be as big as
/// usize, so we inflate LocalReactionId to usize.
type RawId = (ReactorId, usize);

impl DebugInfoRegistry {
    pub fn new() -> Self {
        let mut ich = Self {
            reactor_infos: Default::default(),
            reactor_bound: Default::default(),
            trigger_infos: Default::default(),
            reaction_labels: Default::default(),
        };

        assert_eq!(ich.trigger_infos.push(Cow::Borrowed("startup")), TriggerId::STARTUP);
        assert_eq!(ich.trigger_infos.push(Cow::Borrowed("shutdown")), TriggerId::SHUTDOWN);

        ich
    }
}

impl DebugInfoRegistry {
    pub fn get_debug_info(&self, id: ReactorId) -> &ReactorDebugInfo {
        &self.reactor_infos[id]
    }

    /// Format the id of a component.
    fn fmt_component_path<'a>(&'a self,
                              id: RawId,
                              label: Option<&'a Cow<'static, str>>,
                              always_display_idx: bool) -> impl Display + 'a {
        struct PathFmt<'a> {
            debug: &'a DebugInfoRegistry,
            id: RawId,
            label: Option<&'a Cow<'static, str>>,
            /// If true, the index is part of the output,
            /// even if the label is present.
            always_display_idx: bool,
        }
        use std::fmt::*;
        impl Display for PathFmt<'_> {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                write!(f, "{}", self.debug.get_debug_info(self.id.0))?;
                if let Some(label) = &self.label {
                    if self.always_display_idx {
                        write!(f, "{}@{}", self.id.1, label)
                    } else {
                        write!(f, "{}", label)
                    }
                } else {
                    write!(f, "{}", self.id.1)
                }
            }
        }

        PathFmt { debug: self, id, label, always_display_idx }
    }


    #[inline]
    pub fn fmt_reaction<'a>(&'a self, id: GlobalReactionId) -> impl Display + 'a {
        let raw = (id.0.container(), id.0.local().index());
        self.fmt_component_path(raw,
                                self.reaction_labels.get(&id),
                                true)
    }

    #[inline]
    pub(crate) fn fmt_component<'a>(&'a self, id: TriggerId) -> impl Display + 'a {
        self.fmt_component_path(self.raw_id_of_trigger(id),
                                Some(&self.trigger_infos[id]),
                                false)
    }

    fn raw_id_of_trigger(&self, id: TriggerId) -> RawId {
        match id {
            // Pretend startup and shutdown are in the last reactor.
            // For programs built with LFC, it's the main reactor.
            TriggerId::STARTUP => (self.reactor_infos.last_idx(), 0usize),
            TriggerId::SHUTDOWN => (self.reactor_infos.last_idx(), 0usize),

            id => {
                match self.reactor_bound.binary_search(&id) {
                    // we're the upper bound of some reactor `rid`,
                    // ie, we're the first component of the next reactor.
                    Ok(rid) => (rid.plus(1), 0usize),
                    // Here, rid is the reactor which contains the trigger.
                    // Eg if you have reactor_bound=[2, 4],
                    // that corresponds to two reactors [2..2, 2..4].
                    // If you ask for 2, it will take the branch Ok above.
                    // If you ask for 3, it will fail with Err(0), and reactor_bound[0]==2
                    // is actually the index of the reactor.
                    // todo test this
                    Err(rid) => {
                        (rid, id.index() - self.get_reactor_lower_bound(rid).index())
                    }
                }
            }
        }
    }

    fn get_reactor_lower_bound(&self, rid: ReactorId) -> TriggerId {
        rid.index().checked_sub(1)
            .map(|ix| self.reactor_bound[ix])
            .unwrap_or(TriggerId::FIRST_REGULAR)
    }


    pub(super) fn record_trigger(&mut self, id: TriggerId, name: Cow<'static, str>) {
        let ix = self.trigger_infos.push(name);
        debug_assert_eq!(ix, id);
    }

    pub(super) fn record_reaction(&mut self, id: GlobalReactionId, name: Cow<'static, str>) {
        let existing = self.reaction_labels.insert(id, name);
        debug_assert!(existing.is_none())
    }

    pub(super) fn record_reactor(&mut self, id: ReactorId, debug: ReactorDebugInfo) {
        let ix = self.reactor_infos.push(debug);
        debug_assert_eq!(ix, id);
    }

    pub(super) fn set_id_range(&mut self, id: ReactorId, range: Range<TriggerId>) {
        assert!(range.start <= range.end, "Malformed range {:?}", range);
        assert!(range.start >= TriggerId::FIRST_REGULAR, "Trigger IDs 0-1 are reserved");

        let ix = self.reactor_bound.push(range.end);
        assert_eq!(ix, id);
    }
}


/// Debug information for a single reactor.
pub(crate) struct ReactorDebugInfo {
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
    #[cfg(test)]
    pub(crate) fn test()-> Self {
        Self::root::<()>()
    }

    pub(crate) fn root<R>() -> Self {
        Self {
            type_name: type_name::<R>(),
            inst_name: "/",
            inst_path: "/".into(),
        }
    }

    pub(crate) fn derive<R: ReactorInitializer>(&self, inst_name: &'static str) -> Self {
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

