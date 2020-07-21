use std::borrow::Borrow;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::DerefMut;
use std::rc::Rc;

use crate::reactors::{ReactionCtx, Reactor, Scheduler};
use crate::reactors::assembler::RunnableReactor;
use crate::reactors::id::{GlobalId, Identified, ReactionId};

/// Reaction that is directly executable with a scheduler, instead
/// of with other data.
///
/// Once reactions are in the graph, we can't recover their
/// type information.
/// Eg, when we get a reaction from an ID in the scheduler, the compiler
/// cannot know the type of its Reactor, nor its Reactor::State, or
/// Reactor::ReactionId, which are necessary to call Reactor::react.
///
/// This struct captures this type information by capturing references.
///
/// This explains why:
/// - the state field of RunnableReactor is Rc<RefCell
/// - the Reaction::ReactionId is Copy (simplification, instead of carrying an Rc around)
/// - RunnableReactors have Rcs
///
/// todo we need to avoid references cycles, so probably, the
///  closures here should close over a Weak reference to the RunnableReactor
///
/// todo the error handling could be better
///
/// Note that the function is boxed otherwise this struct has
/// no known size.
///
pub(in super) struct ClosedReaction<'r> {
    body: RefCell<Box<dyn FnMut(&mut ReactionCtx) + 'r>>,
    global_id: GlobalId,
}

impl Debug for ClosedReaction<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClosedReaction {{ {} }}", self.global_id)
    }
}


impl<'g> ClosedReaction<'g> {
    pub(in super) fn fire(&self, ctx: &mut ReactionCtx) {
        let mut cell = &mut *self.body.borrow_mut(); // note: may panic
        (cell)(ctx)
    }

    /// Produce a closure for the reaction.
    pub(in super) fn new<'r : 'g, R: Reactor + 'r>(reactor: &Rc<RunnableReactor<'r, R>>,
                                                   state_ref: &Rc<RefCell<R::State>>,
                                                   global_id: GlobalId,
                                                   reaction_id: R::ReactionId) -> Self {

        let reactor_ref: Rc<RunnableReactor<'r, R>> = Rc::clone(reactor);
        let mut state_ref: Rc<RefCell<_>> = Rc::clone(state_ref);

        let closure = move |scheduler: &mut ReactionCtx| {
            let state: &RefCell<_> = Rc::borrow(&state_ref);
            let mut state_mut = state.borrow_mut();
            R::react(reactor_ref.as_ref(), state_mut.deref_mut(), reaction_id, scheduler)
        };


        ClosedReaction {
            body: RefCell::new(Box::new(closure)),
            global_id,
        }
    }
}


impl Identified for ClosedReaction<'_> {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}

impl Hash for ClosedReaction<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.global_id.hash(state)
    }
}

impl PartialEq for ClosedReaction<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.global_id == other.global_id
    }
}

impl Eq for ClosedReaction<'_> {}
