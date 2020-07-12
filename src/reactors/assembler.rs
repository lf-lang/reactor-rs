use std::cell::RefCell;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use petgraph::graph::DiGraph;

use crate::reactors::action::ActionId;
use crate::reactors::flowgraph::FlowGraph;
use crate::reactors::framework::Reactor;
use crate::reactors::id::{AssemblyId, GlobalId, Identified};
use crate::reactors::ports::{PortId, PortKind};

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

    pub fn new_output_port<T>(&mut self, name: &'static str) -> PortId<T> {
        self.new_port(PortKind::Output, name)
    }

    pub fn new_input_port<T>(&mut self, name: &'static str) -> PortId<T> {
        self.new_port(PortKind::Input, name)
    }

    pub fn new_action(&mut self, name: &'static str, min_delay: Option<Duration>, is_logical: bool) -> ActionId {
        ActionId::new(min_delay, self.new_id(name), is_logical)
    }

    /// Assembles a subreactor. After this, the ports of the subreactor
    /// may be used in some connections, see [reaction_uses], [reaction_affects].
    pub fn new_subreactor<S: Reactor>(&mut self, name: &'static str) -> RunnableReactor<S> {
        let id = self.new_id(name);

        let mut sub_assembler = Assembler::<S>::new(&self.id);

        let sub_reactor = S::assemble(&mut sub_assembler);

        RunnableReactor {
            me: sub_reactor,
            global_id: id,
            state: RefCell::new(S::initial_state())
        }
    }

    /*
     * These methods record dependencies between components.
     *
     * These 2 are trigger dependencies, they may be cyclic (but have delays)
     */

    /// Record that an action triggers the given reaction
    ///
    /// Validity: the action ID was created by this assembler
    pub fn action_triggers(&mut self, port: ActionId, reaction_id: R::ReactionId) {
        unimplemented!()
    }


    /// Record that the given reaction may schedule the action for (future)? execution
    ///
    /// Validity: the action ID was created by this assembler
    pub fn reaction_schedules(&mut self, reaction_id: R::ReactionId, action: ActionId) {
        unimplemented!()
    }

    /*
     * The remaining ones are data-flow dependencies, i.e. relevant for the priority graph, which is a DAG
     */

    /// Binds the values of the given two ports. Every value set
    /// to the upstream port will be reflected in the downstream port.
    ///
    /// # Validity
    ///
    /// Either
    ///  1. upstream is an input port of this reactor, and either
    ///   1.i   downstream is an input port of a direct sub-reactor
    ///   1.ii  downstream is an output port of this reactor
    ///  2. upstream is an output port of a direct sub-reactor, and either
    ///   2.i  downstream is an input port of another sub-reactor
    ///   2.ii downstream is an output port of this reactor
    ///
    /// and all the following:
    /// - downstream is not already bound to another port
    /// - no reaction uses upstream
    /// - no reaction affects downstream
    ///
    pub fn bind_ports<T>(&mut self, upstream: PortId<T>, downstream: PortId<T>) {
        unimplemented!()
    }

    /// Record that the reaction depends on the value of the given port
    ///
    /// Validity: either
    ///  1. the port is an input port of this reactor
    ///  2. the port is an output port of a direct sub-reactor
    pub fn reaction_uses<T>(&mut self, reaction_id: R::ReactionId, port: PortId<T>) {
        unimplemented!()
    }


    /// Record that the given reaction may set the value of the port
    ///
    ///  1. the port is an output port of this reactor
    ///  2. the port is an input port of a direct sub-reactor
    pub fn reaction_affects<T>(&mut self, reaction_id: R::ReactionId, port: PortId<T>) {
        unimplemented!()
    }

    pub fn root() -> Self {
        Self::new(&Rc::new(AssemblyId::Root))
    }
}


impl<R> Assembler<R> where R: Reactor { // this is the private impl block

    fn new_id(&mut self, name: &'static str) -> GlobalId {
        if !self.local_names.insert(name) {
            panic!("Name {} is already used in {}", name, self.id.deref()) // todo impl display
        }
        GlobalId::new(Rc::clone(&self.id), name)
    }

    fn new_port<T>(&mut self, kind: PortKind, name: &'static str) -> PortId<T> {
        PortId::<T>::new(kind, self.new_id(name))
    }


    fn new(id: &Rc<AssemblyId>) -> Self {
        Assembler {
            id: Rc::clone(id),
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
    state: RefCell<R::State>,
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


