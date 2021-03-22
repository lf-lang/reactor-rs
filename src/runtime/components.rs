

use std::fmt::*;
use std::hash::{Hash, Hasher};



use std::sync::{Arc, Mutex};

use crate::reactors::Named;
use crate::runtime::{ReactorDispatcher, LogicalCtx};

#[derive(Clone)]
pub struct Dependencies {
    pub(in super) reactions: Vec<Arc<ReactionInvoker>>
}

impl Default for Dependencies {
    fn default() -> Self {
        Self { reactions: Vec::new() }
    }
}

impl Dependencies {
    pub fn append(&mut self, other: &mut Dependencies) {
        self.reactions.append(&mut other.reactions)
    }
}

impl From<Vec<Arc<ReactionInvoker>>> for Dependencies {
    fn from(reactions: Vec<Arc<ReactionInvoker>>) -> Self {
        Self { reactions }
    }
}

pub struct ReactionInvoker {
    body: Box<dyn Fn(&mut LogicalCtx) + Sync + Send>,
    id: u32,
    /// name used for debug
    name: &'static str,
}

impl Named for ReactionInvoker {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl Display for ReactionInvoker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
    }
}

impl ReactionInvoker {
    pub(in super) fn fire(&self, ctx: &mut LogicalCtx) {
        (self.body)(ctx)
    }

    pub fn id(&self) -> u32 { self.id }

    pub fn new<T: ReactorDispatcher + 'static>(id: u32,
                                               reactor: Arc<Mutex<T>>,
                                               rid: T::ReactionId) -> ReactionInvoker {
        let body = move |ctx: &mut LogicalCtx| {
            let mut ref_mut = reactor.lock().unwrap();
            let r1: &mut T = &mut *ref_mut;
            T::react(r1, ctx, rid)
        };
        ReactionInvoker {
            body: Box::new(body) as Box<dyn Fn(&mut LogicalCtx) + Sync + Send>,
            id,
            name: rid.name(),
        }
    }
}


impl PartialEq for ReactionInvoker {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ReactionInvoker {}

impl Hash for ReactionInvoker {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}
