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



use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::default::Default;
use std::fmt::{Debug, Display, Formatter};

use petgraph::Direction::{Incoming, Outgoing};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::*;

use super::ReactionPlan;

type GraphIx = NodeIndex<u32>;

#[derive(Debug, Eq, PartialEq, Hash)]
enum NodeKind {
    Special,
    // startup/shutdown
    Port,
    Action,
    Reaction,
}

/// Weight of graph nodes.
struct GraphNode {
    kind: NodeKind,
    id: GraphId,
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
enum GraphId { Startup, Shutdown, Id(GlobalId) }

impl From<TriggerId> for GraphId {
    fn from(id: TriggerId) -> Self {
        match id {
            TriggerId::Startup => Self::Startup,
            TriggerId::Shutdown => Self::Shutdown,
            TriggerId::Component(id) => Self::Id(id),
        }
    }
}

impl From<GraphId> for TriggerId {
    fn from(id: GraphId) -> Self {
        match id {
            GraphId::Startup => TriggerId::Startup,
            GraphId::Shutdown => TriggerId::Shutdown,
            GraphId::Id(id) => {
                // we don't assert that it's indeed a trigger and not a reaction...
                TriggerId::Component(id)
            },
        }
    }
}

impl From<GlobalReactionId> for GraphId {
    fn from(id: GlobalReactionId) -> Self {
        Self::Id(id.0)
    }
}

type DepGraphImpl = DiGraph<GraphNode, EdgeWeight, GlobalIdImpl>;

/// Dependency graph representing "instantaneous" dependencies,
/// ie read- and write-dependencies of reactions to ports, and
/// their trigger dependencies. This must be a DAG.
///
/// One global instance is built during the assembly process (see [RootAssembler]).
/// Initialization completes when that instance is turned into
/// a [DataflowInfo], which is the data structure used at runtime.
///
pub(in super) struct DepGraph {
    /// Instantaneous data flow. Must be acyclic. Edges from
    /// reactions to actions are not represented, as they are
    /// not actually a data dependency that could cause a
    /// scheduling conflict. Conveniently those edges are the
    /// only way control flow may be cyclic in the reactor model,
    /// so a stock check for cycles can be used.
    ///
    /// There are several kind of edges:
    /// - reaction -> port: the reaction effects the port/action
    /// - port/action -> reaction: the port/action triggers the reaction
    /// - port -> port: a binding of a port to another
    /// - reaction n -> reaction m: means n has higher priority
    /// than m, only filled in for reactions of the same reactor.
    dataflow: DepGraphImpl,

    /// Maps global IDs back to graph indices.
    ix_by_id: HashMap<GraphId, GraphIx>,
}

impl Debug for GraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}({:?})", self.kind, self.id)
    }
}

impl DepGraph {
    pub fn new() -> Self {
        let mut ich = Self { dataflow: Default::default(), ix_by_id: Default::default() };
        ich.record_special(false);
        ich.record_special(true);
        ich
    }

    /// Produce a dot representation of the graph.
    #[cfg(feature = "graph-dump")]
    pub fn format_dot(&self, id_registry: &IdRegistry) -> impl Display {
        use regex::{Regex, Captures};
        use petgraph::dot::{Config, Dot};
        use NodeKind::Reaction;

        let dot = Dot::with_config(&self.dataflow, &[Config::EdgeNoLabel]);

        let re = Regex::new(r"(Reaction|Action|Port)\((\d++)/(\d++)\)").unwrap();

        let formatted = format!("{:?}", dot);
        let replaced = re.replace_all(formatted.as_str(), |captures: &Captures| {
            let kind = &captures[1];
            let reactor_number = ReactorId::new_const(captures[2].parse().unwrap());
            let component_number = LocalReactionId::new_const(captures[3].parse().unwrap());

            let comp_id = GlobalId::new(reactor_number, component_number);
            let nice_str = if kind != "Reaction" {
                id_registry.fmt_component(comp_id)
            } else {
                id_registry.fmt_reaction(GlobalReactionId(comp_id))
            };

            format!("{}({})", kind, nice_str)
        });

        replaced.into_owned()
    }

    pub(in super) fn record_port(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Port);
    }

    pub(in super) fn record_laction(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Action);
    }

    pub(in super) fn record_paction(&mut self, id: GlobalId) {
        self.record(id, NodeKind::Action);
    }

    pub(in super) fn record_reaction(&mut self, id: GlobalReactionId) {
        self.record(id.0, NodeKind::Reaction);
    }

    pub fn reaction_priority(&mut self, n: GlobalReactionId, m: GlobalReactionId) {
        self.dataflow.add_edge(
            self.get_ix(n.into()),
            self.get_ix(m.into()),
            EdgeWeight::Default,
        );
    }

    pub fn port_bind<T: Send>(&mut self, p1: &Port<T>, p2: &Port<T>) {
        // upstream (settable) -> downstream (bound)
        self.dataflow.add_edge(
            self.get_ix(p1.get_id().into()),
            self.get_ix(p2.get_id().into()),
            EdgeWeight::Default,
        );
    }

    pub fn triggers_reaction(&mut self, trigger: TriggerId, reaction: GlobalReactionId) {
        // trigger -> reaction
        self.dataflow.add_edge(
            self.get_ix(trigger.into()),
            self.get_ix(reaction.into()),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_effects(&mut self, reaction: GlobalReactionId, trigger: TriggerId) {
        // reaction -> trigger
        self.dataflow.add_edge(
            self.get_ix(reaction.into()),
            self.get_ix(trigger.into()),
            EdgeWeight::Default,
        );
    }

    pub fn reaction_uses(&mut self, reaction: GlobalReactionId, trigger: TriggerId) {
        // trigger -> reaction
        self.dataflow.add_edge(
            self.get_ix(trigger.into()),
            self.get_ix(reaction.into()),
            EdgeWeight::Use,
        );
    }

    fn get_ix(&self, id: GraphId) -> GraphIx {
        self.ix_by_id[&id]
    }

    fn record(&mut self, id: GlobalId, kind: NodeKind) {
        let id = GraphId::Id(id);
        match self.ix_by_id.entry(id) {
            Entry::Occupied(_) => panic!("Duplicate id {:?}", id),
            Entry::Vacant(v) => {
                let ix = self.dataflow.add_node(GraphNode { kind, id });
                v.insert(ix);
            }
        }
    }
    fn record_special(&mut self, shutdown: bool) {
        let id = if shutdown { GraphId::Shutdown } else { GraphId::Startup };
        let node = GraphNode { kind: NodeKind::Special, id };
        self.ix_by_id.insert(id, self.dataflow.add_node(node));
    }
}

impl DepGraph {
    pub(in self) fn number_reactions_by_layer(&self) -> HashMap<GlobalReactionId, u32> {
        // note: this will infinitely recurse with a cyclic graph
        let mut layer_numbers = HashMap::<GlobalReactionId, u32>::new();
        let mut todo = self.get_roots();
        let mut todo_next = Vec::new();
        let mut cur_layer: u32 = 0;
        while !todo.is_empty() {
            for ix in todo.drain(..) {
                let node = self.dataflow.node_weight(ix).unwrap();

                if let GraphId::Id(id) = node.id {
                    match layer_numbers.entry(GlobalReactionId(id)) {
                        Entry::Vacant(v) => {
                            v.insert(cur_layer);
                        }
                        Entry::Occupied(mut e) => {
                            e.insert(cur_layer.max(*e.get()));
                        }
                    }
                }
                for out_edge in self.dataflow.edges_directed(ix, Outgoing) {
                    todo_next.push(out_edge.target())
                }
            }
            cur_layer += 1;
            std::mem::swap(&mut todo, &mut todo_next)
        }
        layer_numbers
    }

    /// Returns the roots of the graph
    pub(in self) fn get_roots(&self) -> Vec<GraphIx> {
        self.dataflow.node_indices()
            .filter(|node| self.dataflow.edges_directed(*node, Incoming).next().is_none())
            .collect()
    }
}

#[derive(Debug, Eq, PartialEq)]
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

struct ReactionLayerInfo {
    /// The level of each reaction.
    layer_numbers: HashMap<GlobalReactionId, u32>,
}

impl ReactionLayerInfo {
    /// Append a reaction to the given reaction collection
    fn augment(&self,
               ExecutableReactions(layers): &mut ExecutableReactions,
               reaction: GlobalReactionId,
    ) {
        let ix = self.layer_numbers.get(&reaction).copied().expect("reaction was not recorded in the graph") as usize;

        if let Some(layer) = layers.get_mut(ix) {
            layer.insert(reaction);
        } else {
            debug_assert!(ix >= layers.len());
            let new_layer_count = ix - layers.len() + 1; // len 0, ix 0 => 1 new layer
            layers.reserve(new_layer_count);

            // add a bunch of empty layers to fill holes
            for _ in 1..new_layer_count { // (new_layer_count - 1) iterations
                layers.push(Default::default());
            }
            let mut new_layer: Layer = HashSet::with_capacity(2);
            new_layer.insert(reaction);
            layers.push(new_layer);
        }
    }
}

/// Pre-calculated dependency information,
/// using the dependency graph
pub(in super) struct DataflowInfo {
    /// Maps each trigger to the set of reactions that need
    /// to be scheduled when it is triggered.
    trigger_to_plan: HashMap<TriggerId, ExecutableReactions>,

    layer_info: ReactionLayerInfo,
}

impl DataflowInfo {
    pub fn new(mut graph: DepGraph) -> Result<Self, AssemblyError> {
        if petgraph::algo::is_cyclic_directed(&graph.dataflow) {
            return Err(AssemblyError::CyclicDependencyGraph);
        }

        let layer_info = ReactionLayerInfo { layer_numbers: graph.number_reactions_by_layer() };
        let trigger_to_plan = Self::collect_trigger_to_plan(&mut graph, &layer_info);

        Ok(DataflowInfo { trigger_to_plan, layer_info })
    }

    fn collect_trigger_to_plan(DepGraph { dataflow, .. }: &mut DepGraph,
                               layer_info: &ReactionLayerInfo) -> HashMap<TriggerId, ExecutableReactions> {
        let mut h = HashMap::with_capacity(dataflow.node_count() / 2);

        let triggers: Vec<_> = dataflow.node_indices().filter(|ix| dataflow[*ix].kind != NodeKind::Reaction).collect();

        for trigger in triggers {
            let mut reactions = ExecutableReactions::new();
            Self::collect_reactions_rec(&dataflow, trigger, layer_info, &mut reactions);
            let graph_id = dataflow[trigger].id;
            h.insert(graph_id.into(), reactions);
        }

        h
    }

    fn collect_reactions_rec(dataflow: &DepGraphImpl,
                             trigger: GraphIx,
                             layer_info: &ReactionLayerInfo,
                             reactions: &mut ExecutableReactions) {
        for downstr in dataflow.edges_directed(trigger, Outgoing) {
            let node = &dataflow[downstr.target()];
            match node.kind {
                NodeKind::Port => {
                    // this is necessarily a port->port binding
                    Self::collect_reactions_rec(dataflow, downstr.target(), layer_info, reactions)
                }
                NodeKind::Reaction => {
                    let rid = match node.id {
                        GraphId::Id(rid) => GlobalReactionId(rid),
                        _ => unreachable!("this is a reaction")
                    };
                    // trigger->reaction
                    if downstr.weight() != &EdgeWeight::Use {
                        // so it's a trigger dependency
                        layer_info.augment(reactions, rid)
                    }
                }
                NodeKind::Action | NodeKind::Special => {
                    // trigger->action? this is malformed
                    panic!("malformed dependency graph")
                }
            }
        }
    }


    /// Append a reaction to the given reaction collection
    pub fn augment(&self, reactions: &mut ExecutableReactions, reaction: GlobalReactionId) {
        self.layer_info.augment(reactions, reaction)
    }

    pub fn layer_no(&self, reaction: GlobalReactionId) -> usize {
        self.layer_info.layer_numbers[&reaction] as usize
    }

    /// Returns the set of reactions that needs to be scheduled
    /// when the given trigger is triggered.
    ///
    /// # Panics
    ///
    /// If the trigger id is not registered
    pub fn reactions_triggered_by(&self, trigger: &TriggerId) -> &ExecutableReactions {
        self.trigger_to_plan.get(trigger).expect("trigger was not registered??")
    }
}


type Layer = HashSet<GlobalReactionId>;

/// A set of reactions ordered by relative dependency.
/// The key characteristic of instances is
/// 1. they may be merged together (by a [DataflowInfo]).
/// 2. merging two plans eliminates duplicates
#[derive(Clone, Debug, Default)]
pub(in crate) struct ExecutableReactions(
    /// An ordered list of layers to execute.
    ///
    /// It must by construction be the case that a reaction
    /// in layer `i` has no dependency(1) on reactions in layers `j >= i`.
    /// This way, the execution of reactions in the same layer
    /// may be parallelized.
    ///
    /// (1) a reaction n has a dependency on another m if m
    /// is in the predecessors of n in the dependency graph
    ///
    /// Note that by construction, no two reactions in the same
    /// layer may belong to the same reactor, as all of them
    /// are ordered by priority edges.
    Vec<Layer>
);

impl ExecutableReactions {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Returns an iterator which associates batches of reactions
    /// with their layer. Note that this does not mutate this collection
    /// (eg drain it), because that way we can use borrowed Cows
    /// and avoid more allocation.
    pub fn batches(&self) -> impl Iterator<Item=(usize, &HashSet<GlobalReactionId>)> {
        self.0.iter().enumerate().filter(|it| !it.1.is_empty())
    }

    pub fn next_batch(&self, min_layer: usize) -> Option<(usize, &HashSet<GlobalReactionId>)> {
        self.0.iter().enumerate().skip(min_layer).filter(|it| !it.1.is_empty()).next()
    }

    /// Merge the given set of reactions into this one.
    pub fn absorb(&mut self, ExecutableReactions(src): &ExecutableReactions) {
        let ExecutableReactions(dst) = self;
        if src.len() > dst.len() {
            dst.reserve(src.len() - dst.len());
        }

        let dst_end = dst.len();

        for (i, src_layer) in src.iter().enumerate() {
            if i >= dst_end {
                debug_assert_eq!(i, dst.len());
                dst.push(src_layer.clone());
            } else {
                // merge into existing layer
                // note that we could probs replace get_mut(i).unwrap() with (unsafe) get_unchecked_mut(i)
                let dst_layer = dst.get_mut(i).unwrap();
                dst_layer.extend(src_layer);
            }
        }
    }

    pub(super) fn merge_cows<'x>(x: ReactionPlan<'x>, y: ReactionPlan<'x>) -> ReactionPlan<'x> {
        match (x, y) {
            (None, None) => None,
            (Some(x), None) | (None, Some(x)) => Some(x),
            (Some(Cow::Owned(mut x)), Some(y)) | (Some(y), Some(Cow::Owned(mut x))) => {
                x.absorb(&y);
                Some(Cow::Owned(x))
            },
            (Some(mut x), Some(y)) => {
                x.to_mut().absorb(&y);
                Some(x)
            }
        }
    }
}

impl Display for ExecutableReactions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for layer in &self.0 {
            join_to!(f, layer.iter(), ", ", "{", "} ; ")?;
        }
        write!(f, "]")
    }
}


#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_roots() {
        let mut graph = DepGraph::new();
        let r1 = ReactorId::new(0);
        let n1 = GlobalReactionId::new(r1, LocalReactionId::new(0));
        let n2 = GlobalReactionId::new(r1, LocalReactionId::new(1));

        let p0 = GlobalId::new(r1, LocalReactionId::new(3));
        // let p0 = TriggerId::Component(p0);

        graph.record_reaction(n1);
        graph.record_reaction(n2);
        graph.record_port(p0);

        // n1 > n2
        graph.reaction_priority(n1, n2);

        graph.reaction_effects(n1, TriggerId::Component(p0));
        graph.triggers_reaction(TriggerId::Component(p0), n2);


        let roots = graph.get_roots();
        // graph.eprintln_dot(&IdRegistry::default());
        assert_eq!(roots, vec![graph.get_ix(GraphId::Startup),
                               graph.get_ix(GraphId::Shutdown),
                               graph.get_ix(n1.into())]);
    }
}
