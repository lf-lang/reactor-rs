use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use crate::reactors::{IgnoredDefault, Port, PortKind, Scheduler};
use crate::reactors::action::ActionId;
use crate::reactors::BindStatus;
use crate::reactors::flowgraph::{FlowGraph, Schedulable};
use crate::reactors::id::{AssemblyId, GlobalId, Identified, ReactionId};
use crate::reactors::reaction::ClosedReaction;
use crate::reactors::Reactor;
use crate::reactors::util::{Enumerated, Named};
use crate::reactors::WorldReactor;

/// Base assembler trait.
pub trait AssemblerBase<'a, 'g, R> where R: Reactor {
    /// Binds the values of the given two ports. Every value set
    /// to the upstream port will be reflected in the downstream port.
    /// The downstream port cannot be set by a reaction thereafter.
    ///
    /// # Validity
    ///
    /// Either
    ///  1. upstream is an input port of this reactor, and either
    ///   1.i   downstream is an input port of a direct sub-reactor
    ///   1.ii  downstream is an output port of this reactor
    ///  2. upstream is an output port of a direct sub-reactor, and either
    ///   2.i  downstream is an input port of another direct sub-reactor
    ///   2.ii downstream is an output port of this reactor
    ///
    /// and all the following:
    /// - downstream is not already bound to another port
    /// - no reaction uses upstream todo why though? I found that in the C++ host
    /// - no reaction affects downstream
    ///
    fn bind_ports<T>(&mut self, upstream: &Port<T>, downstream: &Port<T>) -> Result<(), AssemblyError>;

    /// Assembles a subreactor.
    fn new_subreactor<'s, S>(&mut self, name: &'static str) -> Result<Rc<RunnableReactor<'s, S>>, AssemblyError>
        where S: Reactor + 's, 's : 'g;
}

/// Assembles a reactor.
pub struct Assembler<'a, 'g, R> where R: Reactor {
    /// Path from the root of the tree to this assembly,
   /// used to give global ids to each component
    id: Rc<AssemblyId>,

    /// Pool of currently known names
    local_names: HashSet<&'static str>,

    global: &'a mut GlobalAssembler<'g>,

    _phantom_r: PhantomData<&'g R>,
}


impl<'a, 'g, R> Assembler<'a, 'g, R> where R: Reactor {
    // this is the public impl block

    /*
     * These methods create new subcomponents, they're supposed
     * to be stored on the struct of the reactor.
     */

    pub fn new_output_port<T: IgnoredDefault + Copy>(&mut self, name: &'static str) -> Result<Port<T>, AssemblyError> {
        self.new_port(PortKind::Output, name)
    }

    pub fn new_input_port<T: IgnoredDefault + Copy>(&mut self, name: &'static str) -> Result<Port<T>, AssemblyError> {
        self.new_port(PortKind::Input, name)
    }

    pub fn new_action(&mut self, name: &'static str, min_delay: Option<Duration>, is_logical: bool) -> Result<ActionId, AssemblyError> {
        Ok(ActionId::new(min_delay, self.new_id(name)?, is_logical))
    }

    /*
     * These methods record dependencies between components.
     *
     * These 2 are trigger dependencies, they may be cyclic (but have delays)
     */

    /// Record that an action triggers the given reaction.
    ///
    /// # Validity
    ///
    /// - the action ID was created by this assembler
    pub fn action_triggers(&mut self, action: &ActionId, reaction_id: R::ReactionId) -> Result<(), AssemblyError> {
        // TODO
        let rid = ReactionId(self.existing_id(reaction_id));
        self.global.data_flow.add_trigger_dependency(rid, action, DependencyKind::Use)
    }


    /// Record that the given reaction may schedule the action for future execution.
    ///
    /// # Validity
    ///
    /// - the action ID was created by this assembler
    pub fn reaction_schedules(&mut self, reaction_id: R::ReactionId, action: &ActionId) -> Result<(), AssemblyError> {
        // TODO
        let rid = ReactionId(self.existing_id(reaction_id));
        self.global.data_flow.add_trigger_dependency(rid, action, DependencyKind::Affects)
    }

    /*
     * The remaining ones are data-flow dependencies, i.e. relevant for the priority graph, which is a DAG
     */


    /// Record that the reaction depends on the value of the given port
    ///
    /// # Validity
    ///
    /// Either
    ///  1. the port is an input port of this reactor
    ///  2. the port is an output port of a direct sub-reactor
    ///
    pub fn reaction_uses<T>(&mut self, reaction_id: R::ReactionId, port: &Port<T>) -> Result<(), AssemblyError> {
        let react_global_id = self.existing_id(reaction_id);
        let invalid = |cause: &'static str| -> AssemblyError {
            AssemblyError::InvalidDependency(cause, react_global_id.clone(), DependencyKind::Use, port.global_id().clone())
        };

        if port.is_input() && !port.is_in_reactor(&self.id) {
            return Err(invalid("Reaction can only use input ports of this reactor"));
        } else if port.is_output() && !port.is_in_direct_subreactor_of(&self.id) {
            return Err(invalid("Reaction can only use output ports of sub-reactors"));
        }

        self.global.data_flow.add_data_dependency(ReactionId(react_global_id), port, DependencyKind::Use)
    }


    /// Record that the given reaction may set the value of the port
    ///
    /// # Validity
    ///
    /// Either
    ///  1. the port is an output port of this reactor
    ///  2. the port is an input port of a direct sub-reactor
    ///
    /// And
    /// - the port is not bound to an upstream port
    pub fn reaction_affects<T>(&mut self, reaction_id: R::ReactionId, port: &Port<T>) -> Result<(), AssemblyError> {
        let react_global_id = self.existing_id(reaction_id);
        let invalid = |cause: &'static str| -> AssemblyError {
            AssemblyError::InvalidDependency(cause, react_global_id.clone(), DependencyKind::Use, port.global_id().clone())
        };

        if port.is_output() && !port.is_in_reactor(&self.id) {
            return Err(invalid("Reaction can only affect output ports of this reactor"));
        } else if port.is_input() && !port.is_in_direct_subreactor_of(&self.id) {
            return Err(invalid("Reaction can only affect input ports of sub-reactors"));
        } else if port.bind_status() != BindStatus::Unbound {
            return Err(invalid("Port is already bound"));
        }

        self.global.data_flow.add_data_dependency(ReactionId(react_global_id), port, DependencyKind::Affects)
    }
}

impl<'a, 'g, R> AssemblerBase<'a, 'g, R> for Assembler<'a, 'g, R> where R: Reactor {
    fn bind_ports<T>(&mut self, upstream: &Port<T>, downstream: &Port<T>) -> Result<(), AssemblyError> {
        let invalid = |cause: &'static str| -> AssemblyError {
            AssemblyError::InvalidBinding(String::from(cause), upstream.global_id().clone(), downstream.global_id().clone())
        };

        match upstream.kind() {
            PortKind::Input => {
                if !upstream.is_in_reactor(&self.id) {
                    return Err(invalid("1. Upstream port must be an input port of this reactor"));
                } else if downstream.is_input() && !upstream.is_in_direct_subreactor_of(&self.id) {
                    return Err(invalid("1.i. Downstream port should be declared in a direct sub-reactor"));
                } else if downstream.is_output() && !upstream.is_in_reactor(&self.id) {
                    return Err(invalid("1.ii. Downstream port should be declared in this reactor"));
                }
            }
            PortKind::Output => {
                if !upstream.is_in_direct_subreactor_of(&self.id) {
                    return Err(invalid("2. Upstream port must be an input port of this reactor"));
                } else if downstream.is_input()
                    && (!upstream.is_in_direct_subreactor_of(&self.id) || downstream.global_id() == upstream.global_id()) {
                    return Err(invalid("2.i. Downstream port should be declared in a different direct sub-reactor"));
                } else if downstream.is_output() && !upstream.is_in_reactor(&self.id) {
                    return Err(invalid("2.ii. Downstream port should be declared in this reactor"));
                }
            }
        }

        upstream.forward_to(downstream)?;
        self.global.data_flow.add_port_dependency(upstream, downstream)
    }

    /// Assembles a subreactor. After this, the ports of the subreactor
    /// may be used in some connections, see [`reaction_uses`](Self::reaction_uses),
    /// [`reaction_affects`](Self::reaction_affects).
    fn new_subreactor<'s, S>(&mut self, name: &'static str) -> Result<Rc<RunnableReactor<'s, S>>, AssemblyError>
        where S: Reactor + 's, 's : 'g {
        let id = self.new_id(name)?;

        let mut sub_assembler = Assembler::<S>::new(self.global, Rc::new(self.sub_id_for(name)));

        let reactions = sub_assembler.make_reaction_global_ids()?;
        sub_assembler.global.data_flow.add_reactions(reactions);

        match S::assemble(&mut sub_assembler) {
            #[cold] Err(sub_error) => Err(AssemblyError::InContext(id, Box::new(sub_error))),
            Ok(sub_reactor) => {
                let state = Rc::new(RefCell::new(S::initial_state()));
                let reactor = RunnableReactor::<S>::new(sub_reactor, id, &state);
                let rc = Rc::new(reactor);

                sub_assembler.register_closed_reactions(&rc, &state);

                Ok(rc)
            }
        }
    }
}


impl<'a, 'g, R> Assembler<'a, 'g, R> where R: Reactor { // this is the private impl block

    fn new_id(&mut self, name: &'static str) -> Result<GlobalId, AssemblyError> {
        if !self.local_names.insert(name) {
            Err(AssemblyError::DuplicateName(name))
        } else {
            Ok(GlobalId::new(Rc::clone(&self.id), name))
        }
    }

    fn existing_id(&self, reaction_id: R::ReactionId) -> GlobalId {
        let name = reaction_id.name();
        assert!(self.local_names.contains(name), "Should have contained name {}", name);
        GlobalId::new(Rc::clone(&self.id), name)
    }

    fn new_port<T: IgnoredDefault + Copy>(&mut self, kind: PortKind, name: &'static str) -> Result<Port<T>, AssemblyError> {
        Ok(Port::<T>::new(kind, self.new_id(name)?))
    }

    fn sub_id_for(&self, name: &'static str) -> AssemblyId {
        AssemblyId::Nested {
            parent: Rc::clone(&self.id),
            user_name: name,
        }
    }

    fn make_reaction_global_ids(&mut self) -> Result<Vec<ReactionId>, AssemblyError> {
        let ids: Vec<R::ReactionId> = R::ReactionId::list();
        let mut globals: Vec<ReactionId> = Vec::with_capacity(ids.len());

        for id in ids {
            globals.push(ReactionId(self.new_id(id.name())?))
        }
        Ok(globals)
    }

    fn register_closed_reactions<'r>(
        &mut self,
        runnable_r: &Rc<RunnableReactor<'r, R>>,
        state_ref: &Rc<RefCell<R::State>>,
    ) where R: Reactor + 'r, 'r : 'g {
        for typed_id in R::ReactionId::list() {
            let global_id = self.existing_id(typed_id);
            let closed: ClosedReaction<'g> = ClosedReaction::<'g>::new(runnable_r, state_ref, global_id.clone(), typed_id);
            self.global.data_flow.register_reaction(closed);
        }
    }

    pub(in super) fn new(world: &'a mut GlobalAssembler<'g>, id: Rc<AssemblyId>) -> Self {
        Assembler {
            id,
            local_names: Default::default(),
            global: world,
            _phantom_r: PhantomData,
        }
    }
}

/// The output of the assembly of a reactor.
pub struct RunnableReactor<'r, R: Reactor + 'r> {
    me: R,
    global_id: GlobalId,
    // needs to be refcell for transparent mutability
    state: Rc<RefCell<R::State>>,
    phantom: PhantomData<&'r R>,
}

impl<'r, R> RunnableReactor<'r, R> where R: Reactor + 'r {
    pub(in super) fn state(&self) -> Rc<RefCell<R::State>> {
        Rc::clone(&self.state)
    }

    fn new(reactor: R, global_id: GlobalId, state: &Rc<RefCell<R::State>>) -> Self {
        RunnableReactor {
            me: reactor,
            global_id,
            state: Rc::clone(state),
            phantom: PhantomData,
        }
    }
}

// makes it so, that we can use the members of R on a RunnableReactor<R>
impl<'r, R> Deref for RunnableReactor<'r, R> where R: Reactor + 'r {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.me
    }
}

impl<'r, R> Identified for RunnableReactor<'r, R> where R: Reactor + 'r {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}


/// The direction of a dependency. Forward dependencies "use"
/// data, backwards dependencies "affect" data.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DependencyKind { Use, Affects }

impl Display for DependencyKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::Use => write!(f, "uses"),
            DependencyKind::Affects => write!(f, "affects"),
        }
    }
}


/// Assembly-time error. Caused by invalid structure of reactors,
/// eg cyclic dependencies.
pub enum AssemblyError {
    InvalidBinding(String, GlobalId, GlobalId),
    InvalidDependency(&'static str, GlobalId, DependencyKind, GlobalId),
    DuplicateName(&'static str),
    CyclicDependency(String),
    InContext(GlobalId, Box<AssemblyError>),
}

impl Debug for AssemblyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AssemblyError::InvalidBinding(cause, upstream, downstream) => {
                write!(f, "Invalid binding: {} (while binding '{}' to '{}')", cause, upstream, downstream)
            }
            AssemblyError::InvalidDependency(cause, reaction, kind, downstream) => {
                write!(f, "Invalid dependency: {} (for dependency '{}' {} '{}')", cause, reaction, kind, downstream)
            }
            AssemblyError::DuplicateName(name) => {
                write!(f, "Duplicate name '{}'", name)
            }
            AssemblyError::CyclicDependency(msg) => {
                write!(f, "Cyclic dependency: {}", msg)
            }
            AssemblyError::InContext(ctx_id, err) => {
                write!(f, "While assembling {}: ", ctx_id)?;
                Debug::fmt(err, f)
            }
        }
    }
}

/// Global state of the assembly, shared by sub-assemblers
pub(in super) struct GlobalAssembler<'g> {
    data_flow: FlowGraph<'g>,
}


impl<'g> GlobalAssembler<'g> {
    pub fn new() -> Self {
        GlobalAssembler {
            data_flow: Default::default(),
        }
    }
}


/// Build a toplevel reactor
pub fn make_world<'g, R>() -> Result<(RunnableReactor<'g, R>, Scheduler<'g>), AssemblyError> where R: WorldReactor + 'g {
    let mut world = GlobalAssembler::new();
    let mut root_assembler = Assembler::<R>::new(&mut world, Rc::new(AssemblyId::Root));
    let r = <R as Reactor>::assemble(&mut root_assembler)?;
    let state = Rc::new(RefCell::new(R::initial_state()));
    let toplevel_reactor = RunnableReactor::<R>::new(r, root_assembler.new_id(":root:")?, &state);

    let scheduler = Scheduler::new(world.data_flow.consume_to_schedulable()?);

    Ok((toplevel_reactor, scheduler))
}
