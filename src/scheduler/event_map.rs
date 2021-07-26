use std::collections::{HashMap, BTreeSet};
use crate::{ReactorId, ReactionSet, LocalReactionId, LogicalInstant};
use itertools::Itertools;
use std::cmp::Reverse;
use bit_set::BitSet;
use std::iter::FromIterator;

pub struct LocalizedReactionSet {
    set: BitSet,
}

impl LocalizedReactionSet {
    pub fn contains(&self, id: LocalReactionId) -> bool {
        self.set.contains(id as usize)
    }

    pub fn insert(&mut self, id: LocalReactionId) -> bool {
        self.set.insert(id as usize)
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item=LocalReactionId> + 'a {
        self.set.iter().map(|u| u as LocalReactionId)
    }
}

impl FromIterator<LocalReactionId> for LocalizedReactionSet {
    fn from_iter<T: IntoIterator<Item=LocalReactionId>>(iter: T) -> Self {
        let mut result = Self { set: BitSet::with_capacity(32) };
        for t in iter {
            result.insert(t);
        }
        result
    }
}

pub struct TagExecutionPlan {
    pub tag: LogicalInstant,
    map: HashMap<ReactorId, LocalizedReactionSet>,
}


impl TagExecutionPlan {
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Merge the new reactions into this plan.
    pub fn accept(&mut self, new_reactions: ReactionSet) {
        for (key, group) in &new_reactions.into_iter().group_by(|id| id.container) {
            match self.map.get_mut(&key) {
                None => {
                    self.map.insert(key, group.map(|it| it.local).collect());
                },
                Some(set) => {
                    group.for_each(|g| {
                        set.insert(g.local);
                    })
                }
            }
        }
    }

    pub fn new_empty(tag: LogicalInstant) -> TagExecutionPlan {
        TagExecutionPlan {
            tag,
            map: <_>::default(),
        }
    }

    fn new(tag: LogicalInstant, reactions: ReactionSet) -> TagExecutionPlan {
        let mut map: HashMap<ReactorId, LocalizedReactionSet> = HashMap::new();
        for (key, group) in &reactions.into_iter().group_by(|id| id.container) {
            let locals = group.map(|it| it.local).collect();
            map.insert(key, locals);
        }

        TagExecutionPlan { tag, map }
    }


    pub fn iter(&mut self) -> impl Iterator<Item=Batch> {
        let mut map = HashMap::new();
        std::mem::swap(&mut self.map, &mut map);

        map.into_iter().map(|(k, v)| Batch(k, v))
    }
}

pub struct Batch(pub ReactorId, pub LocalizedReactionSet);

#[derive(Default)]
pub struct EventMap {
    /// This is a list sorted by the tag of each TagExecutionPlan.
    /// The earliest tag is at the end.
    /// TODO use linked list
    value_list: Vec<TagExecutionPlan>,
}


impl EventMap {
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
