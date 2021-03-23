use std::fmt::*;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use super::{LogicalCtx, Named, ReactorDispatcher};
use std::cmp::Ordering;


#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Debug, Copy, Clone)]
pub(in super) struct GlobalId {
    container: u32,
    local: u32,
}

/// Wraps a reaction in an "erased" executable form.
/// This wraps a closures, that captures the reactor instance.
pub struct ReactionInvoker {
    body: Box<dyn Fn(&mut LogicalCtx) + Sync + Send>,
    id: GlobalId,
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

impl PartialOrd for ReactionInvoker {
    /// Reactions are comparable if they're declared in the same reactor.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.id.container == other.id.container {
            Some(self.id.local.cmp(&other.id.local))
        } else {
            None
        }
    }
}

impl ReactionInvoker {
    pub(in super) fn fire(&self, ctx: &mut LogicalCtx) {
        (self.body)(ctx)
    }

    pub(in super) fn id(&self) -> GlobalId {
        self.id
    }

    /// Create a new reaction, closing over its reactor instance.
    /// Note that this is only called from within the [new_reaction] macro.
    pub fn new<T: ReactorDispatcher + 'static>(reactor_id: u32,
                                               reaction_priority: u32,
                                               reactor: Arc<Mutex<T>>,
                                               rid: T::ReactionId) -> ReactionInvoker {
        let body = move |ctx: &mut LogicalCtx| {
            let mut ref_mut = reactor.lock().unwrap();
            let r1: &mut T = &mut *ref_mut;
            T::react(r1, ctx, rid)
        };
        ReactionInvoker {
            body: Box::new(body) as Box<dyn Fn(&mut LogicalCtx) + Sync + Send>,
            id: GlobalId { container: reactor_id, local: reaction_priority },
            name: rid.name(),
        }
    }
}


impl PartialEq for ReactionInvoker {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl Eq for ReactionInvoker {}

impl Hash for ReactionInvoker {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}


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
