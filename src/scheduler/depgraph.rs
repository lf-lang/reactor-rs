/*
 * Copyright (c) 2021, TU Dresden.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
 * THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
 * THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */



use petgraph::graph::{DiGraph, NodeIndex};

use crate::{GlobalId, GlobalIdImpl, GloballyIdentified, LogicalAction, PhysicalAction, Port};

type GraphIx = NodeIndex<u32>;

enum GraphNode {
    Port(GlobalId),
    Action(GlobalId),
    Reaction(GlobalId),
}

struct DepGraph {
    // Edges are forward data flow
    graph: DiGraph<GraphNode, (), GlobalIdImpl>,
}

pub struct ReactionIx(GraphIx);
/// Index of a port or action in the graph
pub struct ComponentIx(GraphIx);

impl DepGraph {
    fn record_port<T>(&mut self, item: Port<T>) -> ComponentIx {
        let ix = self.graph.add_node(GraphNode::Port(item.get_id()));
        ComponentIx(ix)
    }

    fn record_laction<T: Clone>(&mut self, item: LogicalAction<T>) -> ComponentIx {
        let ix = self.graph.add_node(GraphNode::Action(item.get_id()));
        ComponentIx(ix)
    }

    fn record_paction<T: Clone>(&mut self, item: PhysicalAction<T>) -> ComponentIx {
        let ix = self.graph.add_node(GraphNode::Action(item.get_id()));
        ComponentIx(ix)
    }

    fn record_reaction<T>(&mut self, id: GlobalId) -> ReactionIx {
        let ix = self.graph.add_node(GraphNode::Reaction(id));
        ReactionIx(ix)
    }

    fn triggers_reaction<T>(&mut self, trigger: ComponentIx, reaction: ReactionIx) {
        self.graph.add_edge(trigger.0, reaction.0, ());
    }

    fn reaction_effects<T>(&mut self, reaction: ReactionIx, trigger: ComponentIx) {
        self.graph.add_edge(trigger.0, reaction.0, ());
    }

    fn new() -> Self {
        Self {
            graph: DiGraph::new()
        }
    }
}
