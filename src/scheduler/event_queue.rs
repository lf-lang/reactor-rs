use std::cmp::{Reverse, Ordering};

use itertools::Itertools;
use smallvec::SmallVec;

use crate::{LocalizedReactionSet, LogicalInstant, ReactionSet, ReactorId};
use crate::scheduler::depgraph::{ExecutableReactions, DependencyInfo};

/// A set of reactions to execute at a particular tag.
/// The key characteristic of instances is
/// 1. they may be merged together.
/// 2. merging two plans eliminates duplicates
pub(in crate) struct TagExecutionPlan {
    /// Tag at which this must be executed.
    pub tag: LogicalInstant,
    pub reactions: ExecutableReactions,
}

pub(in crate) struct Batch(pub ReactorId, pub LocalizedReactionSet);

/// A queue of pending [TagExecutionPlan].
#[derive(Default)]
pub(in crate) struct EventQueue {
    /// This is a list sorted by the tag of each TagExecutionPlan.
    /// The earliest tag is at the end.
    ///
    /// TODO using linked list could be nice in some cases
    value_list: SmallVec<[TagExecutionPlan; 16]>,
}


impl EventQueue {

    /// Removes and returns the earliest tag
    pub fn take_earliest(&mut self) -> Option<TagExecutionPlan> {
        self.value_list.pop()
    }

    pub fn insert(&mut self, tag: LogicalInstant, dataflow: &DependencyInfo, reactions: &ExecutableReactions) {
        match self.value_list.binary_search_by_key(&Reverse(tag), |v| Reverse(v.tag)) {
            Ok(idx) => dataflow.merge(&mut self.value_list[idx].reactions, reactions),
            Err(idx) => self.value_list.insert(idx, TagExecutionPlan { tag, reactions: reactions.clone() }),
        }
    }
}
