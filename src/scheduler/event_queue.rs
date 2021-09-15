
use crate::{ReactorId, ReactionSet, LogicalInstant, LocalizedReactionSet};
use itertools::Itertools;
use std::cmp::Reverse;
use smallvec::SmallVec;


/// A set of reactions to execute at a particular tag.
pub(in crate) struct TagExecutionPlan {
    /// Tag at which this must be executed.
    pub tag: LogicalInstant,

    /// A sparse vector of [LocalizedReactionSet]. The
    /// [ReactorId] is implicit as the index in the vector.
    /// Reactors for which no reaction is scheduled are
    /// [None] in this vector.
    vec: Vec<Option<LocalizedReactionSet>>,

    /// Whether this set has reactions or not. This must be
    /// manually maintained when inserting/removing reactions.
    is_empty: bool
}


impl TagExecutionPlan {
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }

    /// Merge the new reactions into this plan.
    pub fn accept(&mut self, new_reactions: ReactionSet) {
        for (key, group) in &new_reactions.into_iter().group_by(|id| id.0.container()) {
            match self.vec.get_mut(key.index()) {
                None | Some(None) => {
                    // need to insert None
                    if key.index() >= self.vec.len() {
                        self.vec.resize_with(key.index() + 1, || None);
                    }

                    let new_bs: LocalizedReactionSet = group.map(|it| it.0.local()).collect();
                    self.is_empty &= new_bs.is_empty();
                    self.vec[key.index()] = Some(new_bs);
                }
                Some(Some(set)) => {
                    group.for_each(|g| {
                        set.insert(g.0.local());
                    });
                    self.is_empty &= set.is_empty();
                }
            }
        }
    }

    pub fn new_empty(tag: LogicalInstant) -> TagExecutionPlan {
        TagExecutionPlan {
            tag,
            vec: <_>::default(),
            is_empty: true
        }
    }

    fn new(tag: LogicalInstant, reactions: ReactionSet) -> TagExecutionPlan {
        let mut result = Self::new_empty(tag);
        result.accept(reactions);
        result
    }


    /// Returns an iterator that enumerates batches of
    /// reactions to process. This object is cleared of
    /// its contents even if the iterator is not consumed.
    ///
    /// TODO this doesn't use any topological information
    pub fn drain<'a>(&'a mut self) -> impl Iterator<Item=Batch> + 'a {
        self.is_empty = true;
        self.vec.drain(..)
            .enumerate()
            .filter_map(|(i, v)| v.map(|set| Batch(i.into(), set)))
    }
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

    pub fn insert(&mut self, tag: LogicalInstant, reactions: ReactionSet) {
        match self.value_list.binary_search_by_key(&Reverse(tag), |v| Reverse(v.tag)) {
            Ok(idx) => self.value_list[idx].accept(reactions),
            Err(idx) => {
                self.value_list.insert(idx, TagExecutionPlan::new(tag, reactions))
            }
        }
    }
}
