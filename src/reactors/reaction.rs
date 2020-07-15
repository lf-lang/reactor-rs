use std::cell::RefCell;
use std::rc::Rc;

use crate::reactors::assembler::RunnableReactor;
use crate::reactors::framework::{Reactor, Scheduler};
use crate::reactors::id::{GlobalId, Identified};

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
pub(in super) struct ClosedReaction {
    body: RefCell<Box<dyn FnMut(&mut Scheduler)>>,
    global_id: GlobalId,
}

impl ClosedReaction {
    pub(in super) fn fire(&self, scheduler: &mut Scheduler) {
        let mut cell = &mut *self.body.borrow_mut(); // note: may panic
        (cell)(scheduler)
    }

    /// Produce a closure for the reaction.
    pub(in super) fn new<R: Reactor + 'static>(reactor: &Rc<RunnableReactor<R>>,
                                               global_id: GlobalId,
                                               reaction_id: R::ReactionId) -> ClosedReaction {
        let reactor_ref: Rc<RunnableReactor<R>> = Rc::clone(reactor);
        let mut state_ref = reactor_ref.state();

        let closure = move |scheduler: &mut Scheduler| {
            match Rc::get_mut(&mut state_ref) {
                None => panic!("State of {:?} is already mutably borrowed", *reactor_ref.global_id()),
                Some(state_mut) => R::react(reactor_ref.as_ref(), state_mut.get_mut(), reaction_id, scheduler)
            }
        };


        ClosedReaction {
            body: RefCell::new(Box::new(closure)),
            global_id,
        }
    }
}


impl Identified for ClosedReaction {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}
