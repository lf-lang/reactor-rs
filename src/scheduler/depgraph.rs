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



use std::collections::{HashMap, HashSet};
use std::default::Default;

use index_vec::IdxSliceIndex;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::*;

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

/// Dependency graph representing "instantaneous" dependencies,
/// ie read- and write-dependencies of reactions to ports, and
/// their trigger dependencies. This is a DAG.
#[derive(Default)]
pub(in super) struct DepGraph {
    /// Edges are forward data flow
    /// Ie, a reaction R having a trigger dependency on a port P
    /// is represented as an edge P -> R.
    ///
    /// There are several kind of edges:
    /// - reaction n -> reaction m: means n has higher priority
    /// than m, only filled in for reactions of the same reactor.
    /// - reaction -> port/action: the reaction effects the port/action
    /// - port/action -> reaction: the port/action triggers the reaction
    ///
    ///
    graph: DiGraph<GraphNode, EdgeWeight, GlobalIdImpl>,

    /// Maps global IDs back to graph indices.
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

    pub fn action_triggers_reaction(&mut self, trigger: TriggerId, reaction: GlobalReactionId) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.0),
            self.get_ix(reaction.0),
            EdgeWeight::Default,
        );
    }

    pub fn port_triggers_reaction(&mut self, trigger: TriggerId, reaction: GlobalReactionId) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.0),
            self.get_ix(reaction.0),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_uses_port<T>(&mut self, trigger: &Port<T>, reaction: GlobalReactionId) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.get_id().0),
            self.get_ix(reaction.0),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_affects_port<T>(&mut self, reaction: GlobalReactionId, trigger: &Port<T>) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.get_id().0),
            self.get_ix(reaction.0),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_affects_action<K, T: Clone>(&mut self, reaction: GlobalReactionId, trigger: &Action<K, T>) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.get_id().0),
            self.get_ix(reaction.0),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_priority<K, T>(&mut self, n: GlobalReactionId, m: GlobalReactionId) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(n.0),
            self.get_ix(m.0),
            EdgeWeight::Default,
        );
    }

    pub fn port_bind<T>(&mut self, p1: &Port<T>, p2: &Port<T>) {
        // upstream (settable) -> downstream (bound)
        self.graph.add_edge(
            self.get_ix(p1.get_id().0),
            self.get_ix(p2.get_id().0),
            EdgeWeight::Default,
        );
    }

    pub fn triggers_reaction(&mut self, trigger: TriggerId, reaction: GlobalReactionId) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.0),
            self.get_ix(reaction.0),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_effects(&mut self, reaction: GlobalReactionId, trigger: TriggerId) {
        // reaction -> trigger
        self.graph.add_edge(
            self.get_ix(reaction.0),
            self.get_ix(trigger.0),
            EdgeWeight::Default
        );
    }

    pub fn reaction_uses(&mut self, reaction: GlobalReactionId, trigger: TriggerId) {
        // trigger -> reaction
        self.graph.add_edge(
            self.get_ix(trigger.0),
            self.get_ix(reaction.0),
            EdgeWeight::Use,
        );
    }

    fn get_ix(&self, id: GlobalId) -> GraphIx {
        *self.ix_by_id.get(&id).unwrap()
    }


    fn record(&mut self, id: GlobalId, kind: NodeKind) {
        let ix = self.graph.add_node(GraphNode { kind, id });
        self.ix_by_id.insert(id, ix);
    }
}

enum EdgeWeight {
    /// Default semantics for this edge (determined by the
    /// kind of source and target vertex). This only makes a
    /// difference for edges from a port/action to a reaction:
    /// if they're labeled `Default`, they're trigger dependencies,
    /// otherwise use dependencies.
    Default,
    ///
    Use,
}

/// Pre-calculated dependency information,
/// using the dependency graph
struct DependencyInfo {
    /// Maps each trigger to the set of reactions that need
    /// to be scheduled when it is triggered.
    trigger_to_plan: HashMap<TriggerId, ExecutableReactions>,

    /// The level of each reaction.
    layer_numbers: HashMap<GlobalReactionId, usize>,
}

impl DependencyInfo {
    fn new(DepGraph { graph: _, ix_by_id: _ }: DepGraph) -> Result<Self, AssemblyError> {
        let _trigger_to_plan = HashMap::<TriggerId, ExecutableReactions>::new();

        // We need to number reactions by layer.
        todo!()
    }

    /// Append a reaction to the given reaction collection
    fn augment(&self,
               ExecutableReactions(layers): &mut ExecutableReactions,
               reaction: GlobalReactionId,
    ) {
        let ix = self.layer_numbers.get(&reaction).copied().unwrap();

        if let Some(layer) = layers.get_mut(ix) {
            layer.insert(reaction);
        } else {
            debug_assert!(ix >= layers.len());
            let new_layer_count = ix - layers.len() + 1; // len 0, ix 0 => 1 new layer
            layers.reserve(new_layer_count);

            // add a bunch of empty layers to fill holes
            for _ in 1..new_layer_count { // new_layer_count - 1 iterations
                layers.push(Default::default());
            }
            let mut new_layer: Layer = Default::default();
            new_layer.insert(reaction);
            layers.push(new_layer);
        }
    }

    /// Merge the second set of reactions into the first.
    fn merge(&self,
             ExecutableReactions(dst): &mut ExecutableReactions,
             ExecutableReactions(mut src): ExecutableReactions) {
        let new_layers = src.len() - dst.len();
        if new_layers > 0 {
            dst.reserve(new_layers);
        }

        let dst_end = dst.len();

        for (i, src_layer) in src.drain(..).enumerate() {
            if i > dst_end {
                debug_assert_eq!(i, dst.len());
                dst.push(src_layer);
            } else {
                // merge into existing layer
                // note that we could probs replace get_mut(i).unwrap() with (unsafe) get_unchecked_mut(i)
                let dst_layer = dst.get_mut(i).unwrap();
                dst_layer.extend(src_layer);
            }
        }
    }
}


type Layer = HashSet<GlobalReactionId>;

/// A set of reactions ordered by relative dependency.
/// TODO this is relevant for parallel execution of reactions.
struct ExecutableReactions(
    /// An ordered list of layers to execute.
    ///
    /// It must by construction be the case that a reaction
    /// in layer `i` has no dependency on reactions in layers `j >= i`.
    /// This way, the execution of reactions in the same layer
    /// may be parallelized.
    Vec<Layer>
);

impl ExecutableReactions {
    /// Clear the individual layers, retains the allocation
    /// for the layer vector.
    pub fn clear(&mut self) {
        for layer in &mut self.0 {
            layer.clear()
        }
    }
}
