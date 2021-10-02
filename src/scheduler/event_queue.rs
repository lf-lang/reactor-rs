use std::cmp::Reverse;

use smallvec::SmallVec;

use crate::scheduler::depgraph::{ExecutableReactions};

use super::Event;
use std::borrow::Cow;

/// A queue of pending [TagExecutionPlan].
#[derive(Default)]
pub(in super) struct EventQueue<'x> {
    /// This is a list sorted by the tag of each TagExecutionPlan.
    /// The earliest tag is at the end.
    ///
    /// TODO using linked list could be nice in some cases
    value_list: SmallVec<[Event<'x>; 16]>,
}


impl<'x> EventQueue<'x> {
    /// Removes and returns the earliest tag
    pub fn take_earliest(&mut self) -> Option<Event<'x>> {
        self.value_list.pop()
    }

    pub(in super) fn insert(&mut self, evt: Event<'x>) {
        match self.value_list.binary_search_by_key(&Reverse(evt.tag), |e| Reverse(e.tag)) {
            Ok(idx) => self.value_list[idx].reactions.to_mut().absorb(evt.reactions.as_ref()),
            Err(idx) => self.value_list.insert(idx, evt),
        }
    }
}
