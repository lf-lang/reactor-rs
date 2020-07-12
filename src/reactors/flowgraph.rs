use petgraph::graph::{DefaultIx, DiGraph, NodeIndex};

use crate::reactors::id::{GlobalId, Identified};
use crate::reactors::framework::Reactor;

pub struct EdgeWeight;

pub type NodeId = NodeIndex<u32>;


pub struct FlowGraph {
    graph: DiGraph<GlobalId, EdgeWeight>
}


impl Default for FlowGraph {
    fn default() -> Self {
        FlowGraph {
            graph: Default::default()
        }
    }
}

enum FlowGraphElement {



}



pub struct GlobalReactionId<R: Reactor> {
    reaction: R::ReactionId,
    global_id: GlobalId,
}

impl<R> Identified for GlobalReactionId<R> where R: Reactor {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}

