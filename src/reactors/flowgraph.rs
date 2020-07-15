use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::reactors::{AssemblyError, DependencyKind, ReactionCtx, Port};
use crate::reactors::action::ActionId;
use crate::reactors::AssemblyError::CyclicDependency;
use crate::reactors::flowgraph::FlowGraphElement::{PortElt, ReactionElt};
use crate::reactors::id::{GlobalId, Identified, PortId, ReactionId};
use crate::reactors::reaction::ClosedReaction;
use crate::reactors::flowgraph::TriggerGraphElement::ActionElt;
use petgraph::Direction::{Incoming, Outgoing};

pub type GraphId = NodeIndex<u32>;


struct GraphWrapper<V: Identified + Clone> {
    graph: DiGraph<V, ()>,
    graph_ids: HashMap<GlobalId, GraphId>,
}

impl<V: Clone + Identified> Default for GraphWrapper<V> {
    fn default() -> Self {
        Self {
            graph: Default::default(),
            graph_ids: Default::default(),
        }
    }
}

impl<V: Clone + Identified> GraphWrapper<V> {
    fn get_node(&mut self, elt: &V) -> GraphId {
        let id = elt.global_id().clone();
        if let Some(gid) = self.graph_ids.get(&id) {
            gid.clone()
        } else {
            let gid: GraphId = self.graph.add_node(elt.clone());
            self.graph_ids.insert(id, gid);
            gid
        }
    }


    pub fn add_dependency(&mut self, from: V, to: V, kind: DependencyKind) -> Result<(), AssemblyError> {
        let rid = self.get_node(&from);
        let pid = self.get_node(&to);

        match kind {
            DependencyKind::Use => self.graph.add_edge(rid, pid, ()),
            DependencyKind::Affects => self.graph.add_edge(pid, rid, ()),
        };

        Ok(())
    }

    pub fn toposorted(&self) -> Result<Vec<GraphId>, AssemblyError> {
        match petgraph::algo::toposort(&self.graph, None) {
            Err(cycle) => {
                let id = self.graph.node_weight(cycle.node_id()).unwrap().global_id();
                Err(CyclicDependency(format!("Dependency cycle containing {}", id)))
            }
            Ok(vec) => Ok(vec),
        }
    }

    pub fn iter_neighbors<'a>(&'a self, elt: &V, direction: Direction) -> impl Iterator<Item=V> + 'a {
        let gid = self.graph_ids.get(elt.global_id()).unwrap();
        self.graph.neighbors_directed(*gid, direction).map(move |gid| self.to_elt(gid))
    }

    pub fn iter_nodes<'a>(&'a self) -> impl Iterator<Item=V> + 'a {
        self.graph.node_indices().map(move |gid| self.to_elt(gid))
    }

    fn to_elt(&self, gid: GraphId) -> V {
        self.graph.node_weight(gid).unwrap().clone()
    }
}

pub(in super) struct FlowGraph {
    dataflow: GraphWrapper<FlowGraphElement>,
    triggers: GraphWrapper<TriggerGraphElement>,

    closed_reactions: HashMap<ReactionId, Rc<ClosedReaction>>,
}

impl FlowGraph {
    pub fn add_port_dependency<T>(&mut self, upstream: &Port<T>, downstream: &Port<T>) -> Result<(), AssemblyError> {
        let up_id = self.dataflow.get_node(&FlowGraphElement::PortElt(upstream.port_id().clone()));
        let down_id = self.dataflow.get_node(&FlowGraphElement::PortElt(downstream.port_id().clone()));

        self.dataflow.graph.add_edge(up_id, down_id, ());

        Ok(())
    }

    pub fn add_data_dependency<T>(&mut self, reaction: ReactionId, data: &Port<T>, kind: DependencyKind) -> Result<(), AssemblyError> {
        self.dataflow.add_dependency(
            FlowGraphElement::ReactionElt(reaction),
            FlowGraphElement::PortElt(data.port_id().clone()),
            kind,
        )
    }

    pub fn add_trigger_dependency(&mut self, reaction: ReactionId, action: &ActionId, kind: DependencyKind) -> Result<(), AssemblyError> {
        self.triggers.add_dependency(
            TriggerGraphElement::ReactionElt(reaction),
            TriggerGraphElement::ActionElt(action.clone()),
            kind,
        )
    }


    pub fn add_reactions(&mut self, reactions: Vec<ReactionId>) {
        let mut ids = Vec::<FlowGraphElement>::with_capacity(reactions.len());
        for r in reactions {
            ids.push(ReactionElt(r));
        }

        // Add priority links between reactions
        for (a, b) in ids.iter().zip(ids.iter().skip(1)) {
            self.dataflow.add_dependency(a.clone(), b.clone(), DependencyKind::Use);
        }
    }

    pub fn register_reaction(&mut self, reaction: ClosedReaction) {
        self.closed_reactions.insert(ReactionId(reaction.global_id().clone()), Rc::new(reaction));
    }

    pub(in super) fn consume_to_schedulable(mut self) -> Result<Schedulable, AssemblyError> {

        // berk berk berk

        let mut reactions_by_port_id: HashMap<PortId, Vec<Rc<ClosedReaction>>> = <_>::default();
        let mut action_triggers_reaction: HashMap<ActionId, Vec<Rc<ClosedReaction>>> = <_>::default();

        let mut reaction_uses_port: HashMap<ReactionId, Vec<PortId>> = <_>::default();
        let mut reaction_affects_port: HashMap<ReactionId, Vec<PortId>> = <_>::default();

        let mut reaction_schedules_action: HashMap<ReactionId, Vec<ActionId>> = <_>::default();


        let sorted: Vec<GraphId> = self.dataflow.toposorted()?;
        // not the best algorithm but whatever, this is only done on startup anyway (and we can improve later)
        for idx in &sorted {
            let weight = self.dataflow.graph.node_weight(*idx);
            match weight {
                Some(PortElt(port)) => {
                    let mut port_descendants = Vec::<Rc<ClosedReaction>>::new();

                    for follower in sorted[idx.index()..].iter() {
                        if let ReactionElt(id) = self.dataflow.graph.node_weight(*follower).unwrap() {
                            if petgraph::algo::has_path_connecting(&self.dataflow.graph, *idx, *follower, None) {
                                let reaction = self.closed_reactions.get(&id).unwrap();
                                port_descendants.push(Rc::clone(reaction));
                            }
                        }
                    };

                    reactions_by_port_id.insert(port.clone(), port_descendants);
                }
                Some(ReactionElt(reaction_id)) => {
                    let mut uses = Vec::<PortId>::new();
                    let mut affects = Vec::<PortId>::new();

                    self.acc_port_dependencies(idx, &mut affects, Direction::Outgoing);
                    self.acc_port_dependencies(idx, &mut uses, Direction::Incoming);

                    reaction_affects_port.insert(reaction_id.clone(), affects);
                    reaction_uses_port.insert(reaction_id.clone(), uses);
                }
                _ => {}
            }
        };

        for weight in self.triggers.iter_nodes() {
            match &weight {
                TriggerGraphElement::ActionElt(action_id) => {
                    let is_triggered =
                        self.triggers.iter_neighbors(&weight, Incoming)
                            .filter_map(
                                |antidep|
                                    match &antidep {
                                        TriggerGraphElement::ReactionElt(reaction_id) => {
                                            Some(self.closed_reactions.get(reaction_id).unwrap().clone())
                                        }
                                        _ => None
                                    }
                            ).collect::<Vec<_>>();

                    action_triggers_reaction.insert(action_id.clone(), is_triggered);
                }

                TriggerGraphElement::ReactionElt(reaction_id) => {
                    let schedules =
                        self.triggers.iter_neighbors(&weight, Outgoing)
                            .filter_map(
                                |dep|
                                    match &dep {
                                        TriggerGraphElement::ActionElt(action_id) => {
                                            Some(action_id.clone())
                                        }
                                        _ => None
                                    }
                            ).collect::<Vec<_>>();

                    reaction_schedules_action.insert(reaction_id.clone(), schedules);
                }
            }
        }

        Ok(Schedulable {
            reactions_by_port_id,
            reaction_schedules_action,
            reaction_uses_port,
            reaction_affects_port,
            action_triggers_reaction,
        })
    }

    fn acc_port_dependencies(&self, idx: &NodeIndex, output: &mut Vec<PortId>, direction: Direction) {
        for antidep in self.dataflow.graph.neighbors_directed(*idx, direction) {
            match self.dataflow.graph.node_weight(antidep).unwrap() {
                FlowGraphElement::PortElt(port_id) => {
                    output.push(port_id.clone());
                }
                _ => {}
            }
        }
    }
}

// the flow graph is transparent to reactors (they're all flattened)
#[derive(Debug, Eq, PartialEq, Clone)]
enum FlowGraphElement {
    ReactionElt(ReactionId),
    PortElt(PortId),
}

impl Identified for FlowGraphElement {
    fn global_id(&self) -> &GlobalId {
        match self {
            FlowGraphElement::PortElt(id) => id.global_id(),
            FlowGraphElement::ReactionElt(id) => id.global_id(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
enum TriggerGraphElement {
    ReactionElt(ReactionId),
    ActionElt(ActionId),
}

impl Identified for TriggerGraphElement {
    fn global_id(&self) -> &GlobalId {
        match self {
            TriggerGraphElement::ReactionElt(id) => id.global_id(),
            TriggerGraphElement::ActionElt(a) => a.global_id(),
        }
    }
}

impl Default for FlowGraph {
    fn default() -> Self {
        FlowGraph {
            dataflow: <_>::default(),
            triggers: <_>::default(),
            closed_reactions: <_>::default(),
        }
    }
}


pub(in super) struct Schedulable {
    /// Maps port ids to a list of reactions that must be scheduled
    /// each time the port is set in a reaction.
    reactions_by_port_id: HashMap<PortId, Vec<Rc<ClosedReaction>>>,

    reaction_uses_port: HashMap<ReactionId, Vec<PortId>>,
    reaction_affects_port: HashMap<ReactionId, Vec<PortId>>,
    reaction_schedules_action: HashMap<ReactionId, Vec<ActionId>>,

    action_triggers_reaction: HashMap<ActionId, Vec<Rc<ClosedReaction>>>,
}


macro_rules! empty_vec {
    ($name:ident : $t:ty) => {
        const $name: [$t ; 0 ] = [];
    };
}

empty_vec!(NO_REACTIONS : Rc<ClosedReaction>);
empty_vec!(NO_PORTS : PortId);
empty_vec!(NO_ACTIONS : ActionId);

impl Schedulable {
    pub fn get_downstream_reactions(&self, port_id: &PortId) -> &[Rc<ClosedReaction>] {
        self.reactions_by_port_id.get(port_id)
            .map_or_else(|| &NO_REACTIONS[..], |it| it.as_slice())
    }
    pub fn get_triggered_reactions(&self, action_id: &ActionId) -> &[Rc<ClosedReaction>] {
        self.action_triggers_reaction.get(action_id)
            .map_or_else(|| &NO_REACTIONS[..], |it| it.as_slice())
    }

    pub fn get_allowed_reads(&self, reaction_id: &ReactionId) -> &[PortId] {
        self.reaction_uses_port.get(reaction_id)
            .map_or_else(|| &NO_PORTS[..], |it| it.as_slice())
    }

    pub fn get_allowed_writes(&self, reaction_id: &ReactionId) -> &[PortId] {
        self.reaction_affects_port.get(reaction_id)
            .map_or_else(|| &NO_PORTS[..], |it| it.as_slice())
    }

    pub fn get_allowed_schedules(&self, reaction_id: &ReactionId) -> &[ActionId] {
        self.reaction_schedules_action.get(reaction_id)
            .map_or_else(|| &NO_ACTIONS[..], |it| it.as_slice())
    }
}

