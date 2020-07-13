use std::any::Any;
use std::time::Duration;

use petgraph::graph::{DefaultIx, DiGraph, NodeIndex};

use crate::reactors::action::ActionId;
use crate::reactors::assembler::RunnableReactor;
use crate::reactors::framework::Reactor;
use crate::reactors::id::{GlobalId, Identified};
use crate::reactors::ports::PortId;
use crate::reactors::reaction::ClosedReaction;

/*
    TODO like with ClosedReaction, we must erase the external
    generic param on ports.

    Pre-binding everything while the type information is available could be possible
    (the previous prototype did that). But we lose some possibilities
    w.r.t. error handling.

 */


pub type NodeId = NodeIndex<u32>;

pub struct FlowGraph {
    graph: DiGraph<GlobalId, EdgeWeight>
}

pub enum EdgeWeight {
    Dataflow,
    Trigger { delay: Duration },
}

// the flow graph is transparent to reactors (they're all flattened)
enum FlowGraphElement {
    Reaction(ClosedReaction),
    Port(GlobalId), // TODO
    Action(ActionId),
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

impl Default for FlowGraph {
    fn default() -> Self {
        FlowGraph {
            graph: Default::default()
        }
    }
}
