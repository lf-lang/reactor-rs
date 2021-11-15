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
use std::collections::hash_map::Entry as HEntry;
use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
use std::sync::Arc;

use index_vec::{Idx, IndexVec};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction::{Incoming, Outgoing};

use super::ReactionPlan;
use crate::assembly::*;
use crate::impl_types::GlobalIdImpl;
use crate::scheduler::dependencies::NodeKind::MultiportUpstream;
use crate::util::vecmap::{Entry as VEntry, VecMap};
use crate::*;

type GraphIx = NodeIndex<GlobalIdImpl>;

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
pub(super) struct DepGraph {
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
    /// Map of multiport ID -> range of IDs for its channels
    multiport_ranges: VecMap<TriggerId, Range<TriggerId>>,
}

impl Debug for GraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphNode { id: GraphId::Reaction(id), .. } => write!(f, "Reaction({:?})", id),
            GraphNode { id: GraphId::Trigger(TriggerId::STARTUP), .. } => write!(f, "startup"),
            GraphNode { id: GraphId::Trigger(TriggerId::SHUTDOWN), .. } => write!(f, "shutdown"),
            GraphNode { id: GraphId::Trigger(id), kind } => write!(f, "{:?}({:?})", kind, id.index()),
        }
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
        use petgraph::dot::{Config, Dot};
        use regex::{Captures, Regex};

        let dot = Dot::with_config(&self.dataflow, &[Config::EdgeNoLabel]);

        let re = Regex::new(r"(\w+)\(([^)]++)\)").unwrap();

        let formatted = format!("{:?}", dot);
        let replaced = re.replace_all(formatted.as_str(), |captures: &Captures| {
            let kind = &captures[1];
            let id = &captures[2];
            if kind == "Reaction" {
                let global = id.parse::<GlobalId>().unwrap();
                format!("{}({})", kind, id_registry.fmt_reaction(GlobalReactionId(global)))
            } else {
                let trigger_id = TriggerId::new(id.parse().unwrap());
                format!("{}({})", kind, id_registry.fmt_component(trigger_id))
            }
        });

        replaced.into_owned()
    }

    pub(super) fn record_port(&mut self, id: TriggerId) {
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
    pub(super) fn record_port_bank(&mut self, id: TriggerId, len: usize) -> Result<(), AssemblyError> {
        assert!(len > 0, "empty port bank");
        self.record(GraphId::Trigger(id), NodeKind::MultiportUpstream);

        for channel_id in id.next_range(len).map_err(|_| AssemblyError(AssemblyErrorImpl::IdOverflow))? {
            self.multiport_containment.insert(GraphId::Trigger(channel_id), id);

            // self.dataflow.add_edge(upstream_ix, channel_ix, EdgeWeight::Default);
        }
        self.multiport_ranges.insert(
            id,
            Range {
                start: TriggerId::new(id.index() + 1),
                end: TriggerId::new(id.index() + 1 + len),
            },
        );
        Ok(())
    }

    pub(super) fn record_port_bank_component(&mut self, bank_id: TriggerId, channel_id: TriggerId) {
        let channel_ix = self.record_port_impl(channel_id);
        self.dataflow
            .add_edge(self.get_ix(bank_id.into()), channel_ix, EdgeWeight::Default);
    }

    pub(super) fn record_laction(&mut self, id: TriggerId) {
        self.record(GraphId::Trigger(id), NodeKind::Action);
    }

    pub(super) fn record_paction(&mut self, id: TriggerId) {
        self.record(GraphId::Trigger(id), NodeKind::Action);
    }

    pub(super) fn record_timer(&mut self, id: TriggerId) {
        self.record(GraphId::Trigger(id), NodeKind::Timer);
    }

    pub(super) fn record_reaction(&mut self, id: GlobalReactionId) {
        self.record(GraphId::Reaction(id), NodeKind::Reaction);
    }

    /// Records that n > m, ie it will execute always before m.
    pub fn reaction_priority(&mut self, n: GlobalReactionId, m: GlobalReactionId) {
        self.dataflow
            .add_edge(self.get_ix(n.into()), self.get_ix(m.into()), EdgeWeight::Default);
    }

    pub fn port_bind<T: Sync>(&mut self, p1: &Port<T>, p2: &Port<T>) {
        // upstream (settable) -> downstream (bound)
        self.dataflow.add_edge(
            self.get_ix(p1.get_id().into()),
            self.get_ix(p2.get_id().into()),
            EdgeWeight::Default,
        );
    }

    #[cfg(test)]
    pub fn port_bind_untyped(&mut self, p1: TriggerId, p2: TriggerId) {
        // upstream (settable) -> downstream (bound)
        self.dataflow
            .add_edge(self.get_ix(p1.into()), self.get_ix(p2.into()), EdgeWeight::Default);
    }

    pub fn triggers_reaction(&mut self, trigger: TriggerId, reaction: GlobalReactionId) {
        let trigger_ix = self.get_ix(trigger.into());
        if self.dataflow[trigger_ix].kind == MultiportUpstream {
            for channel_id in TriggerId::iter_range(self.multiport_ranges.get(&trigger).unwrap()) {
                self.triggers_reaction(channel_id, reaction);
            }
            return;
        }

        // trigger -> reaction
        self.dataflow
            .add_edge(trigger_ix, self.get_ix(reaction.into()), EdgeWeight::Default);
    }

    pub fn reaction_effects(&mut self, reaction: GlobalReactionId, trigger: TriggerId) {
        // reaction -> trigger
        self.dataflow
            .add_edge(self.get_ix(reaction.into()), self.get_ix(trigger.into()), EdgeWeight::Default);
    }

    pub fn reaction_uses(&mut self, reaction: GlobalReactionId, trigger: TriggerId) {
        // trigger -> reaction
        self.dataflow
            .add_edge(self.get_ix(trigger.into()), self.get_ix(reaction.into()), EdgeWeight::Use);
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
    pub(self) fn number_reactions_by_level(&self) -> HashMap<GlobalReactionId, LevelIx> {
        // note: this will infinitely recurse with a cyclic graph
        let mut level_numbers = HashMap::<GlobalReactionId, LevelIx>::new();
        let mut todo = self.get_roots();
        let mut todo_next = Vec::new();
        // Todo this implementation explores all paths of the graph.
        //  Even small programs may have prohibitively many paths.
        //  Real world example: RadixSort has a chain of 60 reactors,
        //  each reactor is connected to the next and its internal dep graph is a diamond.
        //  So you have 0<>1<>2<>...<>60, so there is 2^60 paths in the graph.
        //  This example is fixed for now, as the diamonds are only of depth 1, and we now dedup the todo queue.
        //  But diamonds of size > 1 will reproduce the problem.

        // There is an easy algorithm that is linear, but destructive.
        // If we use that we have to copy the graph. Is this needed?
        let mut cur_level: LevelIx = LevelIx(0);
        while !todo.is_empty() {
            for ix in todo.drain(..) {
                let node = self.dataflow.node_weight(ix).unwrap();

                if let GraphId::Reaction(id) = node.id {
                    let current = level_numbers.entry(id).or_insert(cur_level);
                    *current = cur_level.max(*current);
                }

                let successors = self.dataflow.edges_directed(ix, Outgoing).map(|e| e.target());
                todo_next.extend(successors);
            }
            cur_level = cur_level.next();
            std::mem::swap(&mut todo, &mut todo_next);
            todo.sort();
            todo.dedup();
        }
        level_numbers
    }

    /// Returns the roots of the graph
    pub(self) fn get_roots(&self) -> Vec<GraphIx> {
        self.dataflow
            .node_indices()
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

/// Stores the level of each reaction. This is transient info
/// that is used to build a [DataflowInfo] and discarded.
///
struct ReactionLevelInfo {
    /// The level of each reaction.
    level_numbers: HashMap<GlobalReactionId, LevelIx>,
}

impl ReactionLevelInfo {
    /// Append a reaction to the given reaction collection
    fn augment(&self, collection: &mut ExecutableReactions, reaction: GlobalReactionId) {
        let ix = self
            .level_numbers
            .get(&reaction)
            .copied()
            .expect("reaction was not recorded in the graph");
        collection.insert(reaction, ix);
    }
}

/// Pre-calculated dependency information,
/// using the dependency graph
pub(super) struct DataflowInfo {
    /// Maps each trigger to the set of reactions that need
    /// to be scheduled when it is triggered.
    trigger_to_plan: IndexVec<TriggerId, Arc<ExecutableReactions<'static>>>,
}

impl DataflowInfo {
    pub fn new(mut graph: DepGraph) -> Result<Self, AssemblyError> {
        if petgraph::algo::is_cyclic_directed(&graph.dataflow) {
            return Err(AssemblyError(AssemblyErrorImpl::CyclicDependencyGraph));
        }

        let level_info = ReactionLevelInfo { level_numbers: graph.number_reactions_by_level() };
        let trigger_to_plan = Self::collect_trigger_to_plan(&mut graph, &level_info);

        Ok(DataflowInfo { trigger_to_plan })
    }

    fn collect_trigger_to_plan(
        DepGraph { dataflow, .. }: &mut DepGraph,
        level_info: &ReactionLevelInfo,
    ) -> IndexVec<TriggerId, Arc<ExecutableReactions<'static>>> {
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
                Self::collect_reactions_rec(&dataflow, trigger, level_info, &mut reactions);
                result.insert(trigger_id, Arc::new(reactions));
            }
        }

        result
    }

    fn collect_reactions_rec(
        dataflow: &DepGraphImpl,
        trigger: GraphIx,
        level_info: &ReactionLevelInfo,
        reactions: &mut ExecutableReactions<'static>,
    ) {
        for downstream in dataflow.edges_directed(trigger, Outgoing) {
            let node = &dataflow[downstream.target()];
            match node.kind {
                NodeKind::Port => {
                    // this is necessarily a port->port binding
                    Self::collect_reactions_rec(dataflow, downstream.target(), level_info, reactions)
                }
                NodeKind::Reaction => {
                    let rid = match node.id {
                        GraphId::Reaction(rid) => rid,
                        _ => unreachable!("this is a reaction"),
                    };
                    // trigger->reaction
                    if downstream.weight() != &EdgeWeight::Use {
                        // so it's a trigger dependency
                        level_info.augment(reactions, rid)
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

type Level = HashSet<GlobalReactionId>;

/// Type of the label of a level. The max value is the maximum
/// depth of the dependency graph.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default)]
pub(crate) struct LevelIx(u32);

impl LevelIx {
    pub fn next(self) -> Self {
        LevelIx(self.0 + 1)
    }
}

impl Display for LevelIx {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A set of reactions ordered by relative dependency.
/// The key characteristic of instances is
/// 1. they may be merged together (by a [DataflowInfo]).
/// 2. merging two plans eliminates duplicates
#[derive(Clone, Debug, Default)]
pub(crate) struct ExecutableReactions<'x> {
    /// An ordered list of levels to execute.
    ///
    /// It must by construction be the case that a reaction
    /// in level `i` has no dependency(1) on reactions in levels `j >= i`.
    /// This way, the execution of reactions in the same level
    /// may be parallelized.
    ///
    /// (1) a reaction n has a dependency on another m if m
    /// is in the predecessors of n in the dependency graph
    ///
    /// Note that by construction, no two reactions in the same
    /// level may belong to the same reactor, as all of them
    /// are ordered by priority edges.
    ///
    /// Note also that the last level in the list must be
    /// non-empty by construction.
    levels: VecMap<LevelIx, Cow<'x, Level>>,
}

impl<'x> ExecutableReactions<'x> {
    pub fn new() -> Self {
        Self { levels: VecMap::new() }
    }

    /// Returns an iterator which associates batches of reactions
    /// with their level. Note that this does not mutate this collection
    /// (eg drain it), because that way we can use borrowed Cows
    /// and avoid more allocation.
    pub fn batches(&self) -> impl Iterator<Item = &(LevelIx, Cow<'x, Level>)> + '_ {
        self.levels.iter_from(LevelIx(0))
    }

    #[inline]
    pub fn next_batch(&self, min_level: LevelIx) -> Option<(LevelIx, &HashSet<GlobalReactionId>)> {
        self.levels.iter_from(min_level).next().map(|(ix, cow)| (*ix, cow.as_ref()))
    }

    /// The greatest level with non-empty value.
    pub fn max_level(&self) -> LevelIx {
        self.levels.max_key().cloned().unwrap_or_default()
    }

    /// Merge the given set of reactions into this one.
    /// Ignore levels that come strictly before `min_level_inclusive`, may even clear them.
    pub fn absorb_after(&mut self, src: &ExecutableReactions<'x>, min_level_inclusive: LevelIx) {
        let src = &src.levels;
        let dst = &mut self.levels;

        for (i, src_level) in src.iter_from(min_level_inclusive) {
            match dst.entry(*i) {
                VEntry::Vacant(e) => {
                    e.insert(src_level.clone());
                }
                VEntry::Occupied(_, e) => {
                    if e.is_empty() {
                        *e = src_level.clone();
                    } else {
                        // todo maybe set is not modified
                        e.to_mut().extend(src_level.iter());
                    }
                }
            }
        }
    }

    /// Insert doesn't mutate the offset.
    fn insert(&mut self, reaction: GlobalReactionId, level_ix: LevelIx) {
        match self.levels.entry(level_ix) {
            VEntry::Vacant(e) => {
                let mut new_level = Level::with_capacity(1);
                new_level.insert(reaction);
                e.insert(Cow::Owned(new_level));
            }
            VEntry::Occupied(_, e) => {
                e.to_mut().insert(reaction);
            }
        }
    }

    pub(super) fn merge_cows(x: ReactionPlan<'x>, y: ReactionPlan<'x>) -> ReactionPlan<'x> {
        Self::merge_plans_after(x, y, LevelIx(0))
    }

    // todo would be nice to simplify this, it's hot
    /// Produce the set union of two reaction plans.
    /// Levels below the `min_level` are not merged, and the caller
    /// shouldn't query them. For all levels >= `min_level`,
    /// the produced reaction plan has all the reactions of
    /// `x` and `y` for that level.
    pub(super) fn merge_plans_after(x: ReactionPlan<'x>, y: ReactionPlan<'x>, min_level: LevelIx) -> ReactionPlan<'x> {
        match (x, y) {
            (x, None) | (None, x) => x,
            (Some(x), y) | (y, Some(x)) if x.max_level() < min_level => y,
            (Some(Cow::Owned(mut x)), Some(y)) | (Some(y), Some(Cow::Owned(mut x))) => {
                x.absorb_after(&y, min_level);
                Some(Cow::Owned(x))
            }
            (Some(mut x), Some(mut y)) => {
                if x.max_level() > y.max_level() {
                    std::mem::swap(&mut x, &mut y);
                }
                // x is the largest one here
                x.to_mut().absorb_after(&y, min_level);
                Some(x)
            }
        }
    }
}

impl Display for ExecutableReactions<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (_, level) in self.levels.iter_from(LevelIx(0)) {
            join_to!(f, level.iter(), ", ", "{", "} ; ")?;
        }
        write!(f, "]")
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    struct TestGraphFixture {
        graph: DepGraph,
        next_trigger_id: TriggerId,
        next_reactor_id: ReactorId,
        debug_info: DebugInfoRegistry,
    }

    impl TestGraphFixture {
        fn new() -> Self {
            Self {
                graph: DepGraph::new(),
                next_trigger_id: TriggerId::FIRST_REGULAR,
                debug_info: DebugInfoRegistry::new(),
                next_reactor_id: ReactorId::new(0),
            }
        }

        fn new_reactor(&mut self, name: impl Into<String>) -> TestAssembler {
            let reactor_id = self.next_reactor_id.get_and_incr();
            self.debug_info.record_reactor(reactor_id, ReactorDebugInfo::test_named(name));
            TestAssembler {
                reactor_id,
                first_trigger_id: self.next_trigger_id,
                fixture: self,
            }
        }

        #[cfg(feature = "graph-dump")]
        #[allow(unused)]
        fn eprintln_graph(&self) {
            eprintln!("{}", self.graph.format_dot(&self.debug_info));
        }
    }

    struct TestAssembler<'a> {
        fixture: &'a mut TestGraphFixture,
        reactor_id: ReactorId,
        first_trigger_id: TriggerId,
    }

    impl TestAssembler<'_> {
        fn new_reactions<const N: usize>(&mut self) -> [GlobalReactionId; N] {
            let result = array![i => GlobalReactionId::new(self.reactor_id, LocalReactionId::from_usize(i)); N];
            let mut last = None;
            for n in &result {
                self.fixture.graph.record_reaction(*n);
                if let Some(last) = last {
                    self.fixture.graph.reaction_priority(last, *n);
                }
                last = Some(*n);
            }
            result
        }

        fn new_ports<const N: usize>(&mut self, names: [&'static str; N]) -> [TriggerId; N] {
            let result = array![_ => self.fixture.next_trigger_id.get_and_incr().unwrap(); N];
            for (i, p) in (&result).into_iter().enumerate() {
                self.fixture.graph.record_port(*p);
                self.fixture.debug_info.record_trigger(*p, Cow::Borrowed(names[i]));
            }
            result
        }
    }

    impl Drop for TestAssembler<'_> {
        fn drop(&mut self) {
            let range = self.first_trigger_id..self.fixture.next_trigger_id;
            self.fixture.debug_info.set_id_range(self.reactor_id, range)
        }
    }

    #[test]
    fn test_roots() {
        let mut test = TestGraphFixture::new();
        let mut builder = test.new_reactor("main");
        let [n1, n2] = builder.new_reactions();
        let [p0] = builder.new_ports(["p0"]);
        drop(builder);

        test.graph.reaction_effects(n1, p0);
        test.graph.triggers_reaction(p0, n2);

        let roots = test.graph.get_roots();
        assert_eq!(
            roots,
            vec![
                test.graph.get_ix(GraphId::STARTUP),
                test.graph.get_ix(GraphId::SHUTDOWN),
                test.graph.get_ix(n1.into()),
            ]
        );
    }

    #[test]
    fn test_level_assignment_simple() {
        let mut test = TestGraphFixture::new();

        let mut builder = test.new_reactor("main");
        let [n1, n2] = builder.new_reactions();
        let [p0] = builder.new_ports(["p0"]);
        drop(builder);

        test.graph.reaction_effects(n1, p0);
        test.graph.triggers_reaction(p0, n2);

        let levels = test.graph.number_reactions_by_level();
        assert!(levels[&n1] < levels[&n2]);
    }

    #[test]
    fn test_level_assignment_diamond_1() {
        let mut test = TestGraphFixture::new();

        let mut builder = test.new_reactor("main");
        let [n1, n2] = builder.new_reactions();
        let [p0, p1] = builder.new_ports(["p0", "p1"]);
        drop(builder);

        test.graph.reaction_effects(n1, p0);
        test.graph.reaction_effects(n1, p1);
        test.graph.triggers_reaction(p0, n2);
        test.graph.triggers_reaction(p1, n2);

        let levels = test.graph.number_reactions_by_level();
        assert!(levels[&n1] < levels[&n2]);
    }

    // this is a stress test that ensures our level assignment algo is not exponential
    #[test]
    fn test_level_assignment_diamond_1_exponential() {
        let mut test = TestGraphFixture::new();

        let mut builder = test.new_reactor("top");
        let [mut prev_in] = builder.new_ports(["in"]);
        drop(builder);

        // the number of paths in the graph is exponential
        // in this upper bound, here 3^60.
        for reactor_id in 0..60 {
            let mut builder = test.new_reactor(format!("r[{}]", reactor_id));
            let [n1, n2] = builder.new_reactions();
            let [p0, p1, out] = builder.new_ports(["p0", "p1", "out"]);
            drop(builder);

            // make a diamond
            test.graph.reaction_effects(n1, p0);
            test.graph.reaction_effects(n1, p1);
            test.graph.triggers_reaction(p0, n2);
            test.graph.triggers_reaction(p1, n2);

            // connect to prev_in
            test.graph.triggers_reaction(prev_in, n1);
            // replace prev_in with out
            test.graph.reaction_effects(n2, out);
            prev_in = out;
        }

        // to debug this lower the graph size
        // test.eprintln_graph();
        let levels = test.graph.number_reactions_by_level();

        assert_eq!(levels.len(), 120);
    }

    #[test]
    fn test_level_assignment_diamond_depth2_exponential() {
        let mut test = TestGraphFixture::new();

        let mut builder = test.new_reactor("top");
        let [mut prev_in] = builder.new_ports(["in"]);
        drop(builder);

        // the number of paths in the graph is exponential
        // in this upper bound, here 3^60.
        for reactor_id in 0..60 {
            let mut builder = test.new_reactor(format!("r[{}]", reactor_id));
            let [n1, n2] = builder.new_reactions();
            let [p0, p01, p1, p11, out] = builder.new_ports(["p0", "p01", "p1", "p11", "out"]);
            drop(builder);

            // make a diamond OF DEPTH > 1

            test.graph.port_bind_untyped(p0, p01);
            test.graph.port_bind_untyped(p1, p11);

            test.graph.reaction_effects(n1, p0);
            test.graph.reaction_effects(n1, p1);
            test.graph.triggers_reaction(p01, n2);
            test.graph.triggers_reaction(p11, n2);

            // connect to prev_in
            test.graph.triggers_reaction(prev_in, n1);
            // replace prev_in with out
            test.graph.reaction_effects(n2, out);
            prev_in = out;
        }

        // to debug this lower the graph size
        // test.eprintln_graph();
        let levels = test.graph.number_reactions_by_level();

        assert_eq!(levels.len(), 120);
    }
}
