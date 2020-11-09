use std::cell::{Cell, Ref};
use std::cell::RefCell;
use std::fmt::*;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use crate::reactors::Named;
use crate::runtime::{Ctx, ReactorDispatcher};

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
    body: Box<dyn Fn(&mut Ctx)>,
    id: i32,
    name: &'static str,
}

unsafe impl Sync for ReactionInvoker {}

unsafe impl Send for ReactionInvoker {}

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
    pub(in super) fn fire(&self, ctx: &mut Ctx) {
        (self.body)(ctx)
    }

    pub fn new<T: ReactorDispatcher + 'static>(id: i32,
                                               //todo should be sync
                                               reactor: Rc<RefCell<T>>,
                                               rid: T::ReactionId) -> ReactionInvoker {
        let body = move |ctx: &mut Ctx| {
            let mut ref_mut = reactor.deref().borrow_mut();
            let r1: &mut T = &mut *ref_mut;
            T::react(r1, ctx, rid)
        };
        ReactionInvoker {
            body: Box::new(body),
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
