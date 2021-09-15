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



use std::collections::HashMap;
use std::default::Default;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::{GlobalId, GlobalIdImpl};

type GraphIx = NodeIndex<u32>;

enum NodeKind {
    Port,
    Action,
    Reaction,
}

/// Weight of graph nodes.
struct GraphNode {
    kind: NodeKind,
    id: GlobalId, // is this necessary? probs
}

#[derive(Default)]
pub(in super) struct DepGraph {
    // Edges are forward data flow
    graph: DiGraph<GraphNode, (), GlobalIdImpl>,

    ix_by_id: HashMap<GlobalId, GraphIx>,
}

pub struct ReactionIx(GraphIx);

/// Index of a port or action in the graph
pub struct ComponentIx(GraphIx);

impl DepGraph {
    pub(in super) fn record_port(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Port);
    }

    pub(in super) fn record_laction(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Action);
    }

    pub(in super) fn record_paction(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Action);
    }

    pub(in super) fn record_reaction(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Reaction);
    }

    fn record(&mut self, id: GlobalId, kind: NodeKind) {
        let ix = self.graph.add_node(GraphNode { kind, id });
        self.ix_by_id.insert(id, ix);
    }

    fn triggers_reaction<T>(&mut self, trigger: ComponentIx, reaction: ReactionIx) {
        self.graph.add_edge(trigger.0, reaction.0, ());
    }

    fn reaction_effects<T>(&mut self, reaction: ReactionIx, trigger: ComponentIx) {
        self.graph.add_edge(trigger.0, reaction.0, ());
    }
}
