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
use std::collections::hash_map::Entry as HEntry;
use std::default::Default;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
use std::sync::Arc;

use index_vec::{Idx, IndexVec};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use crate::*;
use crate::util::vecmap::{Entry as VEntry, VecMap};

use super::ReactionPlan;

type GraphIx = NodeIndex<u32>;

#[derive(Debug, Eq, PartialEq, Hash)]
enum NodeKind {
    /// startup/shutdown
    Special,
    MultiportUpstream,
    Port,
    Action,
    Timer,
    Reaction,
}

/// Weight of graph nodes.
struct GraphNode {
    kind: NodeKind,
    id: GraphId,
}

#[derive(Debug, Hash, Eq, PartialEq, Copy, Clone)]
enum GraphId {
    Trigger(TriggerId),
    Reaction(GlobalReactionId),
}

#[cfg(test)]
impl GraphId {
    const STARTUP: GraphId = GraphId::Trigger(TriggerId::STARTUP);
    const SHUTDOWN: GraphId = GraphId::Trigger(TriggerId::SHUTDOWN);
}

impl From<TriggerId> for GraphId {
    fn from(id: TriggerId) -> Self {
        GraphId::Trigger(id)
    }
}

impl From<GlobalReactionId> for GraphId {
    fn from(id: GlobalReactionId) -> Self {
        GraphId::Reaction(id)
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

    /// Map of multiport component ID -> multiport ID.
    /// todo data structure is bad.
    multiport_containment: HashMap<GraphId, TriggerId>,
    multiport_ranges: VecMap<TriggerId, Range<TriggerId>>,
}

impl Debug for GraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}({:?})", self.kind, self.id)
    }
}

impl DepGraph {
    pub fn new() -> Self {
        let mut ich = Self {
            dataflow: Default::default(),
            ix_by_id: Default::default(),
            multiport_containment: Default::default(),
            multiport_ranges: Default::default(),
        };
        ich.record_special(TriggerId::STARTUP);
        ich.record_special(TriggerId::SHUTDOWN);
        ich
    }

    /// Produce a dot representation of the graph.
    #[cfg(feature = "graph-dump")]
    pub fn format_dot(&self, id_registry: &DebugInfoRegistry) -> impl Display {
        use regex::{Regex, Captures};
        use petgraph::dot::{Config, Dot};

        let dot = Dot::with_config(&self.dataflow, &[Config::EdgeNoLabel]);

        let re = Regex::new(r"(Reaction|Action|Port|Timer)\(Id\((\d++)/(\d++)\)\)").unwrap();

        let formatted = format!("{:?}", dot);
        let replaced = re.replace_all(formatted.as_str(), |captures: &Captures| {
            let kind = &captures[1];
            let reactor_number = ReactorId::new_const(captures[2].parse().unwrap());
            let component_number = LocalReactionId::new_const(captures[3].parse().unwrap());

            let comp_id = GlobalId::new(reactor_number, component_number);
            if kind != "Reaction" {
                format!("{}({})", kind, id_registry.fmt_component(todo!()))
            } else {
                format!("{}({})", kind, id_registry.fmt_reaction(todo!()))
            };
        });

        replaced.into_owned()
    }

    pub(in super) fn record_port(&mut self, id: TriggerId) {
        self.record_port_impl(id);
    }


    fn record_port_impl(&mut self, id: TriggerId) -> GraphIx {
        self.record(GraphId::Trigger(id), NodeKind::Port)
    }

    /// Port banks have an ID which is a fake node, which effects all individual channels.
    /// It looks like a kind of tree:
    /// ```no_compile
    ///        BANK
    ///       / | \ \
    ///    b[0] ...  b[n]
    /// ```
    ///
    /// That way when someone declares a trigger on the bank,
    /// it's forwarded to individual channels in the graph.
    ///
    /// When X declares a trigger/uses on the entire
    /// bank, an edge is added from every channel to X.
    ///
    pub(in super) fn record_port_bank(&mut self, id: TriggerId, len: usize) -> Result<(), AssemblyError> {
        assert!(len > 0, "empty port bank");
        self.record(GraphId::Trigger(id), NodeKind::MultiportUpstream);

        for channel_id in id.next_range(len).map_err(|_| AssemblyError(AssemblyErrorImpl::IdOverflow))? {
            self.multiport_containment.insert(GraphId::Trigger(channel_id), id);

            // self.dataflow.add_edge(upstream_ix, channel_ix, EdgeWeight::Default);
        }
        self.multiport_ranges.insert(id, Range {
            start: TriggerId::new(id.index() + 1),
            end: TriggerId::new(id.index() + 1 + len),
        });
        Ok(())
    }

    pub(in super) fn record_port_bank_component(&mut self, bank_id: TriggerId, channel_id: TriggerId) {
        let channel_ix = self.record_port_impl(channel_id);
        self.dataflow.add_edge(
            self.get_ix(bank_id.into()),
            channel_ix,
            EdgeWeight::Default,
        );
    }

    pub(in super) fn record_laction(&mut self, id: TriggerId) {
        self.record(GraphId::Trigger(id), NodeKind::Action);
    }

    pub(in super) fn record_paction(&mut self, id: TriggerId) {
        self.record(GraphId::Trigger(id), NodeKind::Action);
    }

    pub(in super) fn record_timer(&mut self, id: TriggerId) {
        self.record(GraphId::Trigger(id), NodeKind::Timer);
    }

    pub(in super) fn record_reaction(&mut self, id: GlobalReactionId) {
        self.record(GraphId::Reaction(id), NodeKind::Reaction);
    }

    pub fn reaction_priority(&mut self, n: GlobalReactionId, m: GlobalReactionId) {
        self.dataflow.add_edge(
            self.get_ix(n.into()),
            self.get_ix(m.into()),
            EdgeWeight::Default,
        );
    }

    pub fn port_bind<T: Sync>(&mut self, p1: &Port<T>, p2: &Port<T>) {
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

    fn record(&mut self, id: GraphId, kind: NodeKind) -> GraphIx {
        match self.ix_by_id.entry(id) {
            HEntry::Occupied(_) => panic!("Duplicate id {:?}", id),
            HEntry::Vacant(v) => {
                let ix = self.dataflow.add_node(GraphNode { kind, id });
                v.insert(ix);
                ix
            }
        }
    }

    fn record_special(&mut self, trigger: TriggerId) {
        let id = GraphId::Trigger(trigger);
        let node = GraphNode { kind: NodeKind::Special, id };
        self.ix_by_id.insert(id, self.dataflow.add_node(node));
    }
}

impl DepGraph {
    pub(in self) fn number_reactions_by_layer(&self) -> HashMap<GlobalReactionId, LayerIx> {
        // note: this will infinitely recurse with a cyclic graph
        let mut layer_numbers = HashMap::<GlobalReactionId, LayerIx>::new();
        let mut todo = self.get_roots();
        let mut todo_next = Vec::new();
        let mut cur_layer: LayerIx = LayerIx(0);
        while !todo.is_empty() {
            for ix in todo.drain(..) {
                let node = self.dataflow.node_weight(ix).unwrap();

                if let GraphId::Reaction(id) = node.id {
                    match layer_numbers.entry(id) {
                        HEntry::Vacant(v) => {
                            v.insert(cur_layer);
                        }
                        HEntry::Occupied(mut e) => {
                            e.insert(cur_layer.max(*e.get()));
                        }
                    }
                }
                for out_edge in self.dataflow.edges_directed(ix, Outgoing) {
                    todo_next.push(out_edge.target())
                }
            }
            cur_layer = cur_layer.next();
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
    layer_numbers: HashMap<GlobalReactionId, LayerIx>,
}

impl ReactionLayerInfo {
    /// Append a reaction to the given reaction collection
    fn augment(&self, collection: &mut ExecutableReactions, reaction: GlobalReactionId) {
        let ix = self.layer_numbers.get(&reaction).copied().expect("reaction was not recorded in the graph");
        collection.insert(reaction, ix);
    }
}

/// Pre-calculated dependency information,
/// using the dependency graph
pub(in super) struct DataflowInfo {
    /// Maps each trigger to the set of reactions that need
    /// to be scheduled when it is triggered.

    trigger_to_plan: IndexVec<TriggerId, Arc<ExecutableReactions<'static>>>,

}

impl DataflowInfo {
    pub fn new(mut graph: DepGraph) -> Result<Self, AssemblyError> {
        if petgraph::algo::is_cyclic_directed(&graph.dataflow) {
            return Err(AssemblyError(AssemblyErrorImpl::CyclicDependencyGraph));
        }

        let layer_info = ReactionLayerInfo { layer_numbers: graph.number_reactions_by_layer() };
        let trigger_to_plan = Self::collect_trigger_to_plan(&mut graph, &layer_info);

        Ok(DataflowInfo { trigger_to_plan })
    }

    fn collect_trigger_to_plan(DepGraph { dataflow, multiport_containment, .. }: &mut DepGraph,
                               layer_info: &ReactionLayerInfo) -> IndexVec<TriggerId, Arc<ExecutableReactions<'static>>> {
        let mut result = IndexVec::with_capacity(dataflow.node_count() / 2);

        for trigger in dataflow.node_indices() {
            if let GraphId::Trigger(trigger_id) = dataflow[trigger].id {
                // if let Some(_multiport_id) = multiport_containment.get(&dataflow[trigger].id) {
                //     assert_eq!(dataflow[trigger].kind, NodeKind::Port);
                //     todo!("multiports")
                    // todo this is a multiport channel:
                    //  1. if someone has declared a dependency on this individual channel, collect dependencies into DEPS
                    //  2. else add trigger to DELAY goto 4
                    //  3. merge DEPS into dependencies ALL for the whole multiport
                    //  4. goto next iteration while some channels of the multiport remain to be processed
                    //  5. assign all triggers in DELAY the dependencies ALL
                    //
                    //  This requires all components of a given multiport to be processed consecutively.
                // }

                let mut reactions = ExecutableReactions::new();
                Self::collect_reactions_rec(&dataflow, trigger, layer_info, &mut reactions);
                result.insert(trigger_id, Arc::new(reactions));
            }
        }

        result
    }

    fn collect_reactions_rec(dataflow: &DepGraphImpl,
                             trigger: GraphIx,
                             layer_info: &ReactionLayerInfo,
                             reactions: &mut ExecutableReactions<'static>) {
        for downstream in dataflow.edges_directed(trigger, Outgoing) {
            let node = &dataflow[downstream.target()];
            match node.kind {
                NodeKind::Port => {
                    // this is necessarily a port->port binding
                    Self::collect_reactions_rec(dataflow, downstream.target(), layer_info, reactions)
                }
                NodeKind::Reaction => {
                    let rid = match node.id {
                        GraphId::Reaction(rid) => rid,
                        _ => unreachable!("this is a reaction")
                    };
                    // trigger->reaction
                    if downstream.weight() != &EdgeWeight::Use {
                        // so it's a trigger dependency
                        layer_info.augment(reactions, rid)
                    }
                }
                _ => {
                    // trigger->action? this is malformed
                    panic!("malformed dependency graph")
                }
            }
        }
    }


    /// Returns the set of reactions that needs to be scheduled
    /// when the given trigger is triggered.
    ///
    /// # Panics
    ///
    /// If the trigger id is not registered
    pub fn reactions_triggered_by(&self, trigger: &TriggerId) -> &ExecutableReactions<'static> {
        &self.trigger_to_plan[*trigger]
    }
}


type Layer = HashSet<GlobalReactionId>;

/// Type of the label of a layer. The max value is the maximum
/// depth of the dependency graph.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default)]
pub(crate) struct LayerIx(u32);

impl LayerIx {
    pub fn next(self) -> Self {
        LayerIx(self.0 + 1)
    }
}

impl Display for LayerIx {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A set of reactions ordered by relative dependency.
/// The key characteristic of instances is
/// 1. they may be merged together (by a [DataflowInfo]).
/// 2. merging two plans eliminates duplicates
#[derive(Clone, Debug, Default)]
pub(in crate) struct ExecutableReactions<'x> {
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
    ///
    /// Note also that the last layer in the list must be
    /// non-empty by construction.
    layers: VecMap<LayerIx, Cow<'x, Layer>>,
}

impl<'x> ExecutableReactions<'x> {
    pub fn new() -> Self {
        Self { layers: VecMap::new() }
    }

    /// Returns an iterator which associates batches of reactions
    /// with their layer. Note that this does not mutate this collection
    /// (eg drain it), because that way we can use borrowed Cows
    /// and avoid more allocation.
    pub fn batches(&self) -> impl Iterator<Item=&(LayerIx, Cow<'x, Layer>)> +'_{
        self.layers.iter_from(LayerIx(0))
    }

    #[inline]
    pub fn next_batch(&self, min_layer: LayerIx) -> Option<(LayerIx, &HashSet<GlobalReactionId>)> {
        self.layers.iter_from(min_layer).next().map(|(ix, cow)| (*ix, cow.as_ref()))
    }

    /// The greatest layer with non-empty value.
    pub fn max_layer(&self) -> LayerIx {
        self.layers.max_key().cloned().unwrap_or_default()
    }

    /// Merge the given set of reactions into this one.
    /// Ignore layers that come strictly before first_layer, may clear them if need be.
    pub fn absorb_after(&mut self, src: &ExecutableReactions<'x>, min_layer_inclusive: LayerIx) {
        let src = &src.layers;
        let dst = &mut self.layers;

        for (i, src_layer) in src.iter_from(min_layer_inclusive) {
            match dst.entry(*i) {
                VEntry::Vacant(e) => {
                    e.insert(src_layer.clone());
                },
                VEntry::Occupied(_, e) => {
                    if e.is_empty() {
                        *e = src_layer.clone();
                    } else {
                        // todo maybe set is not modified
                        e.to_mut().extend(src_layer.iter());
                    }
                }
            }
        }
    }

    /// Insert doesn't mutate the offset.
    fn insert(&mut self, reaction: GlobalReactionId, layer_ix: LayerIx) {
        match self.layers.entry(layer_ix) {
            VEntry::Vacant(e) => {
                let mut new_layer: Layer = HashSet::with_capacity(1);
                new_layer.insert(reaction);
                e.insert(Cow::Owned(new_layer));
            }
            VEntry::Occupied(_, e) => {
                e.to_mut().insert(reaction);
            },
        }
    }

    pub(super) fn merge_cows(x: ReactionPlan<'x>, y: ReactionPlan<'x>) -> ReactionPlan<'x> {
        Self::merge_cows_after(x, y, LayerIx(0))
    }

    /// todo would be nice to simplify this, it's hot
    pub(super) fn merge_cows_after(x: ReactionPlan<'x>, y: ReactionPlan<'x>, min_layer: LayerIx) -> ReactionPlan<'x> {
        match (x, y) {
            (x, None) | (None, x) => x,
            (Some(x), y) | (y, Some(x)) if x.max_layer() < min_layer  => y,
            (Some(Cow::Owned(mut x)), Some(y)) | (Some(y), Some(Cow::Owned(mut x))) => {
                x.absorb_after(&y, min_layer);
                Some(Cow::Owned(x))
            },
            (Some(mut x), Some(mut y)) => {
                if x.max_layer() > y.max_layer() {
                    std::mem::swap(&mut x, &mut y);
                }
                // x is the largest one here
                x.to_mut().absorb_after(&y, min_layer);
                Some(x)
            }
        }
    }
}

impl Display for ExecutableReactions<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (_, layer) in self.layers.iter_from(LayerIx(0)) {
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

        let p0 = TriggerId::new(12);
        // let p0 = TriggerId::Component(p0);

        graph.record_reaction(n1);
        graph.record_reaction(n2);
        graph.record_port(p0);

        // n1 > n2
        graph.reaction_priority(n1, n2);

        graph.reaction_effects(n1, p0);
        graph.triggers_reaction(p0, n2);


        let roots = graph.get_roots();
        // graph.eprintln_dot(&IdRegistry::default());
        assert_eq!(roots, vec![graph.get_ix(GraphId::STARTUP),
                               graph.get_ix(GraphId::SHUTDOWN),
                               graph.get_ix(n1.into())]);
    }
}
