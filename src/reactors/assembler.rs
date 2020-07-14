use std::borrow::BorrowMut;
use std::cell::{RefCell, RefMut};
use std::collections::HashSet;
use std::env::set_current_dir;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::time::Duration;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::reactors::action::ActionId;
use crate::reactors::flowgraph::{FlowGraph, NodeId};
use crate::reactors::framework::{Reactor, Scheduler};
use crate::reactors::id::{AssemblyId, GlobalId, Identified};
use crate::reactors::ports::{PortId, PortKind, IgnoredDefault};
use crate::reactors::util::{Named, Enumerated};
use crate::reactors::world::WorldReactor;
use std::fmt::{Debug, Formatter};

/// Assembles a reactor.
pub struct Assembler<R: Reactor> {
    /// Path from the root of the tree to this assembly,
   /// used to give global ids to each component
    id: Rc<AssemblyId>,

    /// Pool of currently known names
    local_names: HashSet<&'static str>,

    data_flow: FlowGraph,

    _phantom_r: PhantomData<R>,
}


impl<R> Assembler<R> where R: Reactor {
    // this is the public impl block

    /*
     * These methods create new subcomponents, they're supposed
     * to be stored on the struct of the reactor.
     */

    pub fn new_output_port<T: IgnoredDefault>(&mut self, name: &'static str) -> Result<PortId<T>, AssemblyError> {
        self.new_port(PortKind::Output, name)
    }

    pub fn new_input_port<T: IgnoredDefault>(&mut self, name: &'static str) -> Result<PortId<T>, AssemblyError> {
        self.new_port(PortKind::Input, name)
    }

    pub fn new_action(&mut self, name: &'static str, min_delay: Option<Duration>, is_logical: bool) -> Result<ActionId, AssemblyError> {
        Ok(ActionId::new(min_delay, self.new_id(name)?, is_logical))
    }

    /// Assembles a subreactor. After this, the ports of the subreactor
    /// may be used in some connections, see [reaction_uses], [reaction_affects].
    pub fn new_subreactor<S: Reactor>(&mut self, name: &'static str) -> Result<RunnableReactor<S>, AssemblyError> {
        let id = self.new_id(name)?;

        let new_index = NodeIndex::new(0); // TODO
        let mut sub_assembler = Assembler::<S>::new(Rc::new(self.sub_id_for::<S>(new_index, name)));

        let sub_reactor = S::assemble(&mut sub_assembler)?;

        // todo compute flow graph
        //  close reactions

        Ok(RunnableReactor::<S>::new(sub_reactor, id))
    }

    /*
     * These methods record dependencies between components.
     *
     * These 2 are trigger dependencies, they may be cyclic (but have delays)
     */

    /// Record that an action triggers the given reaction.
    ///
    /// Validity: the action ID was created by this assembler
    pub fn action_triggers(&mut self, port: ActionId, reaction_id: R::ReactionId) {
        // TODO
    }


    /// Record that the given reaction may schedule the action for future execution.
    ///
    /// Validity: the action ID was created by this assembler
    pub fn reaction_schedules(&mut self, reaction_id: R::ReactionId, action: ActionId) {
        // TODO
    }

    /*
     * The remaining ones are data-flow dependencies, i.e. relevant for the priority graph, which is a DAG
     */

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
    pub fn bind_ports<T>(&mut self, upstream: &PortId<T>, downstream: &PortId<T>) -> Result<(), AssemblyError> {
        let invalid = |cause: &'static str| -> AssemblyError {
            AssemblyError::InvalidBinding(cause, upstream.global_id().clone(), downstream.global_id().clone())
        };

        match upstream.kind() {
            PortKind::Input => {
                // 1.
                if !upstream.is_in_reactor(&self.id) {
                    return Err(invalid("Upstream port must be an input port of this reactor"));
                }
                match downstream.kind() {
                    PortKind::Input => {
                        if !upstream.is_in_direct_subreactor_of(&self.id) {
                            return Err(invalid("1.i. Downstream port should be declared in a direct sub-reactor"));
                        }
                    }
                    PortKind::Output => {
                        if !upstream.is_in_reactor(&self.id) {
                            return Err(invalid("1.ii. Downstream port should be declared in this reactor"));
                        }
                    }
                }
            }
            PortKind::Output => {
                // 2.
                if !upstream.is_in_direct_subreactor_of(&self.id) {
                    return Err(invalid("Upstream port must be an input port of this reactor"));
                }
                match downstream.kind() {
                    PortKind::Input => {
                        if !upstream.is_in_direct_subreactor_of(&self.id) || downstream.global_id() == upstream.global_id() {
                            return Err(invalid("2.i. Downstream port should be declared in a different direct sub-reactor"));
                        }
                    }
                    PortKind::Output => {
                        if !upstream.is_in_reactor(&self.id) {
                            return Err(invalid("2.ii. Downstream port should be declared in this reactor"));
                        }
                    }
                }
            }
        }

        upstream.forward_to(downstream)
    }

    /// Record that the reaction depends on the value of the given port
    ///
    /// Validity: either
    ///  1. the port is an input port of this reactor
    ///  2. the port is an output port of a direct sub-reactor
    pub fn reaction_uses<T>(&mut self, reaction_id: R::ReactionId, port: &PortId<T>) {
        // TODO
    }


    /// Record that the given reaction may set the value of the port
    ///
    ///  1. the port is an output port of this reactor
    ///  2. the port is an input port of a direct sub-reactor
    pub fn reaction_affects<T>(&mut self, reaction_id: R::ReactionId, port: &PortId<T>) {
        // TODO
    }

    pub fn make_world() -> Result<RunnableReactor<R>, AssemblyError> where R: WorldReactor {
        let mut root_assembler = Self::new(Rc::new(AssemblyId::Root));
        let r = <R as Reactor>::assemble(&mut root_assembler)?;
        Ok(RunnableReactor::new(r, root_assembler.new_id(":root:")?))
    }
}


impl<R> Assembler<R> where R: Reactor { // this is the private impl block

    fn new_id(&mut self, name: &'static str) -> Result<GlobalId, AssemblyError> {
        if !self.local_names.insert(name) {
            Err(AssemblyError::DuplicateName(name))
        } else {
            Ok(GlobalId::new(Rc::clone(&self.id), name))
        }
    }

    fn new_port<T: IgnoredDefault>(&mut self, kind: PortKind, name: &'static str) -> Result<PortId<T>, AssemblyError> {
        Ok(PortId::<T>::new(kind, self.new_id(name)?))
    }

    fn sub_id_for<T>(&self, id: NodeId, name: &'static str) -> AssemblyId {
        AssemblyId::Nested {
            parent: Rc::clone(&self.id),
            ext_id: id,
            user_name: name,
            typename: std::any::type_name::<T>(),
        }
    }


    fn new(id: Rc<AssemblyId>) -> Self {
        Assembler {
            id,
            local_names: Default::default(),
            data_flow: Default::default(),
            _phantom_r: PhantomData,
        }
    }
}

/// The output of the assembly of a reactor.
pub struct RunnableReactor<R: Reactor> {
    me: R,
    global_id: GlobalId,
    // needs to be refcell for transparent mutability
    state: Rc<RefCell<R::State>>,
}

impl<R: Reactor> RunnableReactor<R> {
    pub(in super) fn state(&self) -> Rc<RefCell<R::State>> {
        Rc::clone(&self.state)
    }

    fn new(reactor: R, global_id: GlobalId) -> Self {
        RunnableReactor {
            me: reactor,
            global_id,
            state: Rc::new(RefCell::new(R::initial_state())),
        }
    }
}

// makes it so, that we can use the members of R on a RunnableReactor<R>
impl<R> Deref for RunnableReactor<R> where R: Reactor {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.me
    }
}

impl<R> Identified for RunnableReactor<R> where R: Reactor {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}

pub enum AssemblyError {
    InvalidBinding(&'static str, GlobalId, GlobalId),
    DuplicateName(&'static str),
}

impl Debug for AssemblyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AssemblyError::InvalidBinding(cause, upstream, downstream) => {
                write!(f, "Invalid binding: {} {} {}", cause, upstream, downstream)
            }

            AssemblyError::DuplicateName(name) => {
                write!(f, "Duplicate name {}", name)
            }
        }
    }
}
