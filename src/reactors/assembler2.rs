use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use std::time::Duration;

use petgraph::prelude::NodeIndex;

use super::action::ActionId;
use super::framework::{ActionId, Assembler, PortId, Reactor, RunnableReactor};
use super::id::{AssemblyId, GlobalId};
use super::ports::PortId;
use std::collections::HashSet;
use core::panicking::panic_fmt;
use crate::reactors::ports::PortKind;
use crate::reactors::ports::PortKind::Output;

type NodeIdRepr = u32;
type NodeId = NodeIndex<NodeIdRepr>;


struct AssemblerImpl<R: Reactor> {
    /// Path from the root of the tree to this assembly,
    /// used to give global ids to each component
    id: Rc<AssemblyId>,

    local_names: HashSet<&'static str>,
}


impl<R: Reactor> AssemblerImpl<R> {

    fn new_id(&mut self, name: &str) -> GlobalId {
        if !self.local_names.insert(name) {
            panic!("Name {} is already used in {}", name, self) // todo impl display
        }
        GlobalId::new(Rc::clone(&self.id), name)
    }

    fn new_port<T>(&mut self, kind: PortKind, name: &str) -> PortId<T> {
        PortId::<T>::new(kind, self.new_id(name))
    }
}


impl<R: Reactor> Assembler<R> for AssemblerImpl<R> {
    fn new_output_port<T>(&mut self, name: &str) -> PortId<T> {
        self.new_port(PortKind::Output, name)
    }

    fn new_input_port<T>(&mut self, name: &str) -> PortId<T> {
        self.new_port(PortKind::Input, name)
    }

    fn new_action(&mut self, name: &str, delay: Option<Duration>, is_logical: bool) -> ActionId {
        ActionId::new(delay, self.new_id(name), is_logical)
    }

    fn new_subreactor<S: Reactor>(&mut self, name: &str) -> RunnableReactor<S> {
        unimplemented!()
    }

    fn action_triggers(&mut self, port: ActionId, reaction_id: <R as Reactor>::ReactionId) {
        unimplemented!()
    }

    fn reaction_schedules(&mut self, reaction_id: <R as Reactor>::ReactionId, action: ActionId) {
        unimplemented!()
    }

    fn bind_ports<T>(&mut self, upstream: PortId<T>, downstream: PortId<T>) {
        unimplemented!()
    }

    fn reaction_uses<T>(&mut self, reaction_id: <R as Reactor>::ReactionId, port: PortId<T>) {
        unimplemented!()
    }

    fn reaction_affects<T>(&mut self, reaction_id: <R as Reactor>::ReactionId, port: PortId<T>) {
        unimplemented!()
    }
}
