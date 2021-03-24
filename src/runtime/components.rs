use std::fmt::*;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use super::{LogicalCtx, Named, ReactorDispatcher};
use std::cmp::Ordering;

/// Type of the global ID of a reactor.
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Debug, Copy, Clone)]
pub struct ReactorId { value: usize }

impl ReactorId {
    pub fn first() -> Self {
        Self { value: 0 }
    }

    pub fn get_and_increment(&mut self) -> Self {
        let this = *self;
        *self = Self { value: this.value + 1 };
        this
    }
}

/// Identifies a component of a reactor using the ID of its container
/// and a local component ID.
#[derive(Eq, Ord, PartialOrd, PartialEq, Hash, Debug, Copy, Clone)]
pub(in super) struct GlobalReactionId {
    container: ReactorId,
    local: u32,
}

/// Wraps a reaction in an "erased" executable form.
/// This wraps a closures, that captures the reactor instance.
pub struct ReactionInvoker {
    /// This is the invocable function.
    /// It needs to be boxed otherwise the struct has no known size.
    body: Box<dyn Fn(&mut LogicalCtx) + Sync + Send>,
    /// Global ID of the reaction, used to test equality of
    /// this reaction with other things.
    id: GlobalReactionId,
}

impl Display for ReactionInvoker {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
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
    /// Execute the body of the reaction for the given logical context.
    pub(in super) fn fire(&self, ctx: &mut LogicalCtx) {
        (self.body)(ctx)
    }

    pub(in super) fn id(&self) -> GlobalReactionId {
        self.id
    }

    /// Create a new reaction, closing over its reactor instance.
    /// Note that this is only called from within the [new_reaction] macro.
    ///
    /// The reaction_priority orders the reaction relative to
    /// the other reactions of the same reactor. The `reactor_id`
    /// is global.
    ///
    pub fn new<T: ReactorDispatcher + 'static>(reactor_id: ReactorId,
                                               reaction_priority: u32,
                                               reactor: Arc<Mutex<T>>,
                                               rid: T::ReactionId) -> ReactionInvoker {
        Self::new_from_closure(reactor_id, reaction_priority, move |ctx: &mut LogicalCtx| {
            let mut ref_mut = reactor.lock().unwrap();
            let r1: &mut T = &mut *ref_mut;
            T::react(r1, ctx, rid)
        })
    }

    /// Create a new reaction invoker that doesn't need a reactor,
    /// ie the invoked code can be arbitrary.
    /// This may be used to test the logic of the scheduler
    pub(in crate) fn new_from_closure(reactor_id: ReactorId,
                                      reaction_priority: u32,
                                      action: impl Fn(&mut LogicalCtx) + Send + Sync + 'static) -> ReactionInvoker {
        ReactionInvoker {
            body: Box::new(action),
            id: GlobalReactionId { container: reactor_id, local: reaction_priority },
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

/// todo ensure it is toposorted
pub type ToposortedReactions = Vec<Arc<ReactionInvoker>>;
