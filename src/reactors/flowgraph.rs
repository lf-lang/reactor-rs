use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::reactors::{AssemblyError, DependencyKind, ReactionCtx, Port};
use crate::reactors::action::ActionId;
use crate::reactors::AssemblyError::CyclicDependency;
use crate::reactors::flowgraph::FlowGraphElement::{ActionElt, PortElt, ReactionElt};
use crate::reactors::id::{GlobalId, Identified, PortId, ReactionId};
use crate::reactors::reaction::ClosedReaction;

pub type GraphId = NodeIndex<u32>;

pub(in super) struct FlowGraph {
    graph: DiGraph<FlowGraphElement, ()>,
    graph_ids: HashMap<GlobalId, GraphId>,

    closed_reactions: HashMap<ReactionId, Rc<ClosedReaction>>,
}

impl FlowGraph {
    fn get_node(&mut self, elt: FlowGraphElement) -> GraphId {
        let id = elt.global_id().clone();
        if let Some(gid) = self.graph_ids.get(&id) {
            gid.clone()
        } else {
            let gid: GraphId = self.graph.add_node(elt);
            self.graph_ids.insert(id, gid);
            gid
        }
    }

    pub fn add_port_dependency<T>(&mut self, upstream: &Port<T>, downstream: &Port<T>) -> Result<(), AssemblyError> {
        let up_id = self.get_node(PortElt(upstream.port_id().clone()));
        let down_id = self.get_node(PortElt(downstream.port_id().clone()));

        self.graph.add_edge(up_id, down_id, ());

        Ok(())
    }

    pub fn add_data_dependency<T>(&mut self, reaction: ReactionId, data: &Port<T>, kind: DependencyKind) -> Result<(), AssemblyError> {
        assert!(self.graph_ids.contains_key(&reaction.global_id()));
        // todo MM do we have to add ports too?
        // assert!(self.graph_ids.contains_key(data.global_id()));

        let rid = self.get_node(ReactionElt(reaction));
        let pid = self.get_node(PortElt(data.port_id().clone()));

        match kind {
            DependencyKind::Use => self.graph.add_edge(rid, pid, ()),
            DependencyKind::Affects => self.graph.add_edge(pid, rid, ()),
        };

        Ok(())
    }

    pub fn add_reactions(&mut self, reactions: Vec<ReactionId>) {
        let mut ids = Vec::<GraphId>::with_capacity(reactions.len());
        for r in reactions {
            ids.push(self.get_node(ReactionElt(r)));
        }

        // Add priority links between reactions
        for (a, b) in ids.iter().zip(ids.iter().skip(1)) {
            self.graph.add_edge(*a, *b, ());
        }
    }

    pub fn register_reaction(&mut self, reaction: ClosedReaction) {
        self.closed_reactions.insert(ReactionId(reaction.global_id().clone()), Rc::new(reaction));
    }

    pub(in super) fn consume_to_schedulable(mut self) -> Result<Schedulable, AssemblyError> {
        let sorted: Vec<GraphId> = match petgraph::algo::toposort(&self.graph, None) {
            Ok(sorted) => sorted,
            Err(cycle) => {
                let id = self.graph.node_weight(cycle.node_id()).unwrap().global_id();
                return Err(CyclicDependency(format!("Dependency cycle containing {}", id)));
            }
        };

        let mut reactions_by_port_id: HashMap<PortId, Vec<Rc<ClosedReaction>>> = <_>::default();

        let mut reaction_uses_port: HashMap<ReactionId, Vec<PortId>> = <_>::default();
        let mut reaction_affects_port: HashMap<ReactionId, Vec<PortId>> = <_>::default();

        let mut reaction_schedules_action: HashMap<ReactionId, Vec<ActionId>> = <_>::default();
        let mut action_triggers_reaction: HashMap<ActionId, Vec<ReactionId>> = <_>::default();

        // not the best algorithm but whatever, this is only done on startup anyway (and we can improve later)
        for idx in &sorted {
            let weight = self.graph.node_weight(*idx);
            match weight {
                Some(PortElt(port)) => {
                    let mut port_descendants = Vec::<Rc<ClosedReaction>>::new();

                    for follower in sorted[idx.index()..].iter() {
                        if let ReactionElt(id) = self.graph.node_weight(*follower).unwrap() {
                            if petgraph::algo::has_path_connecting(&self.graph, *idx, *follower, None) {
                                let reaction = self.closed_reactions.get(id).unwrap();
                                port_descendants.push(Rc::clone(reaction));
                            }
                        }
                    };

                    reactions_by_port_id.insert(port.clone(), port_descendants);
                }
                Some(ActionElt(action_id)) => {
                    let mut is_triggered = Vec::<ReactionId>::new();

                    for antidep in self.graph.neighbors_directed(*idx, Direction::Outgoing) {
                        match self.graph.node_weight(antidep).unwrap() {
                            ReactionElt(reaction_id) => {
                                is_triggered.push(reaction_id.clone());
                            }
                            _ => {}
                        }
                    }

                    action_triggers_reaction.insert(action_id.clone(), is_triggered);
                }
                Some(ReactionElt(reaction_id)) => {
                    let mut uses = Vec::<PortId>::new();
                    let mut affects = Vec::<PortId>::new();

                    let mut schedules = Vec::<ActionId>::new();

                    for antidep in self.graph.neighbors_directed(*idx, Direction::Outgoing) {
                        match self.graph.node_weight(antidep).unwrap() {
                            PortElt(port_id) => {
                                affects.push(port_id.clone());
                            }
                            ActionElt(action_id) => {
                                schedules.push(action_id.clone());
                            }
                            _ => {}
                        }
                    }

                    for dep in self.graph.neighbors_directed(*idx, Direction::Incoming) {
                        match self.graph.node_weight(dep).unwrap() {
                            PortElt(port_id) => {
                                uses.push(port_id.clone());
                            }
                            _ => {}
                        }
                    }

                    reaction_affects_port.insert(reaction_id.clone(), affects);
                    reaction_uses_port.insert(reaction_id.clone(), uses);
                    reaction_schedules_action.insert(reaction_id.clone(), schedules);
                }
                _ => {}
            }
        };

        Ok(Schedulable {
            reactions_by_port_id,
            reaction_schedules_action,
            reaction_uses_port,
            reaction_affects_port,
            action_triggers_reaction,
        })
    }
}

// the flow graph is transparent to reactors (they're all flattened)
#[derive(Debug, Eq, PartialEq, Clone)]
enum FlowGraphElement {
    ReactionElt(ReactionId),
    PortElt(PortId),
    ActionElt(ActionId),
}

impl Identified for FlowGraphElement {
    fn global_id(&self) -> &GlobalId {
        match self {
            FlowGraphElement::PortElt(id) => id.global_id(),
            FlowGraphElement::ReactionElt(id) => id.global_id(),
            FlowGraphElement::ActionElt(a) => a.global_id(),
        }
    }
}

impl Default for FlowGraph {
    fn default() -> Self {
        FlowGraph {
            graph: <_>::default(),
            graph_ids: <_>::default(),
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
    action_triggers_reaction: HashMap<ActionId, Vec<ReactionId>>,
}

const EMPTY_VEC: [Rc<ClosedReaction>; 0] = [];

impl Schedulable {
    pub fn get_downstream_reactions(&self, port_id: &PortId) -> &[Rc<ClosedReaction>] {
        self.reactions_by_port_id.get(port_id).map_or_else(|| &EMPTY_VEC[..],
                                                           |it| it.as_slice())
    }
}

