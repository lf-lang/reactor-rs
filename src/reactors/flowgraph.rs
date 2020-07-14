use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

use petgraph::graph::{DefaultIx, DiGraph, NodeIndex};

use crate::reactors::action::ActionId;
use crate::reactors::assembler::RunnableReactor;
use crate::reactors::flowgraph::FlowGraphElement::Reaction;
use crate::reactors::framework::Reactor;
use crate::reactors::id::{GlobalId, Identified};
use crate::reactors::ports::PortId;
use crate::reactors::reaction::ClosedReaction;
use crate::reactors::{AssemblyError, DependencyKind};

/*
    TODO like with ClosedReaction, we must erase the external
    generic param on ports.

    Pre-binding everything while the type information is available could be possible
    (the previous prototype did that). But we lose some possibilities
    w.r.t. error handling.

 */


pub type GraphId = NodeIndex<u32>;

pub(in super) struct FlowGraph {
    graph: DiGraph<GlobalId, EdgeWeight>,
    graph_ids: HashMap<GlobalId, GraphId>,
}

impl FlowGraph {
    fn get_node(&mut self, id: &GlobalId) -> GraphId {
        if let Some(gid) = self.graph_ids.get(id) {
            gid.clone()
        } else {
            let gid: GraphId = self.graph.add_node(id.clone());
            self.graph_ids.insert(id.clone(), gid);
            gid
        }
    }

    pub fn add_port_dependency<T>(&mut self, upstream: &PortId<T>, downstream: &PortId<T>) -> Result<(), AssemblyError> {

        let up_id = self.get_node(upstream.global_id());
        let down_id = self.get_node(downstream.global_id());

        self.graph.add_edge(up_id, down_id, EdgeWeight::Dataflow);

        Ok(())
    }

    pub fn add_data_dependency<T>(&mut self, reaction: GlobalId, data: &PortId<T>, kind: DependencyKind) -> Result<(), AssemblyError> {
        assert!(self.graph_ids.contains_key(&reaction));
        // todo MM looks like we have to add ports too?
        // assert!(self.graph_ids.contains_key(data.global_id()));

        let rid = self.get_node(&reaction);
        let pid = self.get_node(data.global_id());

        match kind {
            DependencyKind::Use => self.graph.add_edge(rid, pid, EdgeWeight::Dataflow),
            DependencyKind::Affects => self.graph.add_edge(pid, rid, EdgeWeight::Dataflow),
        };

        Ok(())
    }

    pub fn add_reactions(&mut self, reactions: Vec<GlobalId>) {
        let mut ids = Vec::<GraphId>::with_capacity(reactions.len());
        for r in reactions {
            ids.push(self.get_node(&r));
        }

        // Add priority links between reactions
        for (a, b) in ids.iter().zip(ids.iter().skip(1)) {
            self.graph.add_edge(*a, *b, EdgeWeight::Dataflow);
        }
    }
}


pub enum EdgeWeight {
    Dataflow,
    Trigger { delay: Duration },
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
            graph: Default::default(),
            graph_ids: Default::default(),
        }
    }
}
