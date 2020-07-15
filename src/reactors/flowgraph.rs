use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

use petgraph::algo::Cycle;
use petgraph::graph::{DefaultIx, DiGraph, NodeIndex};
use petgraph::visit::GetAdjacencyMatrix;

use crate::reactors::{Assembler, AssemblyError, DependencyKind};
use crate::reactors::action::ActionId;
use crate::reactors::assembler::RunnableReactor;
use crate::reactors::AssemblyError::CyclicDependency;
use crate::reactors::flowgraph::FlowGraphElement::{Port, Reaction};
use crate::reactors::framework::Reactor;
use crate::reactors::id::{GlobalId, Identified};
use crate::reactors::ports::PortId;
use crate::reactors::reaction::ClosedReaction;
use std::rc::Rc;

/*
    TODO like with ClosedReaction, we must erase the external
    generic param on ports.

    Pre-binding everything while the type information is available could be possible
    (the previous prototype did that). But we lose some possibilities
    w.r.t. error handling.

 */


pub type GraphId = NodeIndex<u32>;

pub(in super) struct FlowGraph {
    graph: DiGraph<FlowGraphElement, ()>,
    graph_ids: HashMap<GlobalId, GraphId>,
}

impl FlowGraph {
    fn get_node(&mut self, elt: FlowGraphElement) -> GraphId {
        let id = elt.global_id().clone();
        if let Some(gid) = self.graph_ids.get(&id) {
            gid.clone()
        } else {
            let gid: GraphId = self.graph.add_node(elt);
            self.graph_ids.insert(id, gid);
            gid
        }
    }

    pub fn add_port_dependency<T>(&mut self, upstream: &PortId<T>, downstream: &PortId<T>) -> Result<(), AssemblyError> {
        let up_id = self.get_node(Port(upstream.global_id().clone()));
        let down_id = self.get_node(Port(downstream.global_id().clone()));

        self.graph.add_edge(up_id, down_id, ());

        Ok(())
    }

    pub fn add_data_dependency<T>(&mut self, reaction: GlobalId, data: &PortId<T>, kind: DependencyKind) -> Result<(), AssemblyError> {
        assert!(self.graph_ids.contains_key(&reaction));
        // todo MM looks like we have to add ports too?
        // assert!(self.graph_ids.contains_key(data.global_id()));

        let rid = self.get_node(Reaction(reaction));
        let pid = self.get_node(Port(data.global_id().clone()));

        match kind {
            DependencyKind::Use => self.graph.add_edge(rid, pid, ()),
            DependencyKind::Affects => self.graph.add_edge(pid, rid, ()),
        };

        Ok(())
    }

    pub fn add_reactions(&mut self, reactions: Vec<GlobalId>) {
        let mut ids = Vec::<GraphId>::with_capacity(reactions.len());
        for r in reactions {
            ids.push(self.get_node(Reaction(r)));
        }

        // Add priority links between reactions
        for (a, b) in ids.iter().zip(ids.iter().skip(1)) {
            self.graph.add_edge(*a, *b, ());
        }
    }

    /// Note that since this only cares about ports that are in
    /// the graph, the result excludes dangling ports
    pub fn reactions_by_port_set(
        &mut self,
        reaction_by_id: HashMap<GlobalId, Rc<ClosedReaction>>,
    ) -> Result<HashMap<GlobalId, Vec<Rc<ClosedReaction>>>, AssemblyError> {
        let sorted: Vec<GraphId> = match petgraph::algo::toposort(&self.graph, None) {
            Ok(sorted) => sorted,
            Err(cycle) => {
                let id = self.graph.node_weight(cycle.node_id()).unwrap().global_id();
                return Err(CyclicDependency(format!("Dependency cycle containing {}", id)));
            }
        };

        let mut result: HashMap<GlobalId, Vec<Rc<ClosedReaction>>> = <_>::default();

        // not the best algorithm but whatever, this is only done on startup anyway (and we can improve later)
        for port_idx in &sorted {
            // todo unconnected ports will throw on unwrap -> add them when creating
            if let Some(Port(port)) = self.graph.node_weight(*port_idx) {
                let mut port_descendants = Vec::<Rc<ClosedReaction>>::new();

                for follower in sorted[port_idx.index()..].iter() {
                    if let Reaction(id) = self.graph.node_weight(*follower).unwrap() {
                        if petgraph::algo::has_path_connecting(&self.graph, *port_idx, *follower, None) {
                            let reaction = reaction_by_id.get(id).unwrap();
                            port_descendants.push(Rc::clone(reaction));
                        }
                    }
                };

                result.insert(port.clone(), port_descendants);
            }
        };

        Ok(result)
    }
}


// the flow graph is transparent to reactors (they're all flattened)
enum FlowGraphElement {
    Reaction(GlobalId),
    Port(GlobalId),
    Action(ActionId),
}

impl Identified for FlowGraphElement {
    fn global_id(&self) -> &GlobalId {
        match self {
            FlowGraphElement::Reaction(id) => id,
            FlowGraphElement::Port(id) => id,
            FlowGraphElement::Action(a) => a.global_id(),
        }
    }
}

impl Default for FlowGraph {
    fn default() -> Self {
        FlowGraph {
            graph: <_>::default(),
            graph_ids: <_>::default(),
        }
    }
}
