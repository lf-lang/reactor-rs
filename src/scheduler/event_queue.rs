use std::borrow::Cow;
use std::cmp::Reverse;

use smallvec::SmallVec;

use crate::LogicalInstant;
use crate::scheduler::depgraph::{DependencyInfo, ExecutableReactions};

/// A set of reactions to execute at a particular tag.
/// The key characteristic of instances is
/// 1. they may be merged together.
/// 2. merging two plans eliminates duplicates
pub(in crate) struct TagExecutionPlan<'x> {
    /// Tag at which this must be executed.
    pub tag: LogicalInstant,
    pub reactions: Cow<'x, ExecutableReactions>,
}


/// A queue of pending [TagExecutionPlan].
#[derive(Default)]
pub(in super) struct EventQueue<'x> {
    /// This is a list sorted by the tag of each TagExecutionPlan.
    /// The earliest tag is at the end.
    ///
    /// TODO using linked list could be nice in some cases
    value_list: SmallVec<[TagExecutionPlan<'x>; 16]>,
}


impl<'x> EventQueue<'x> {
    /// Removes and returns the earliest tag
    pub fn take_earliest(&mut self) -> Option<TagExecutionPlan<'x>> {
        self.value_list.pop()
    }

    pub(in super) fn insert(&mut self, tag: LogicalInstant, dataflow: &'x DependencyInfo, reactions: Cow<'x, ExecutableReactions>) {
        match self.value_list.binary_search_by_key(&Reverse(tag), |v| Reverse(v.tag)) {
            Ok(idx) => dataflow.merge(self.value_list[idx].reactions.to_mut(), reactions.as_ref()),
            Err(idx) => self.value_list.insert(idx, TagExecutionPlan { tag, reactions }),
        }
    }
}
