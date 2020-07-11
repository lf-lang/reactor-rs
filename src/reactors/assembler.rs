//! Assembler logic
//!
//! This contains the logic to build a reactor graph, upon
//! initialization of the program.


use std::fmt::{Debug, Formatter};
use std::rc::Rc;

use petgraph::graph::{DiGraph, Neighbors};
use petgraph::graph::NodeIndex;

use super::port::{InPort, OutPort};
use super::reactor::Reactor;
use super::reaction::Reaction;
use std::ops::Deref;
use std::pin::Pin;
use petgraph::Direction;
use crate::reactors::assembler::AssemblyId::Nested;
use std::process::id;
use std::cmp::Ordering;
use std::borrow::Borrow;
use std::marker::PhantomData;

type NodeIdRepr = u32;
type NodeId = NodeIndex<NodeIdRepr>;


pub trait GraphElement {
    fn kind(&self) -> NodeKind;
}

enum EdgeTag {
    // TODO this is messy

    /*
        O: OutPort -> I: InPort
        means I is bound to O
     */
    PortConnection,

    /*
        O: (Output | Action) -> N: Reaction
        means N depends on the action/output port
     */
    ReactionDep,
    /*
        N: Reaction -> I: (Input | Action)
        means I depends on N
     */
    ReactionAntiDep,
}


type NodeData = Rc<dyn GraphElement>;

/// The dependency graph between structures
type DepGraph = DiGraph<NodeData, EdgeTag, NodeIdRepr>;


/// Identifies an assembly uniquely in the tree
/// This is just a path built from the root down.
#[derive(Eq, PartialEq, Debug)]
enum AssemblyId {
    Root,
    Nested {
        // This is the node id used in the parent
        ext_id: NodeId,
        // the id of the parent
        parent: Rc<AssemblyId>,
    },
}

impl Clone for AssemblyId {
    fn clone(&self) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Nested { ext_id, parent } =>
                Self::Nested { ext_id: *ext_id, parent: Rc::clone(parent) }
        }
    }
}

impl AssemblyId {
    fn parent(&self) -> Option<&AssemblyId> {
        match self {
            Self::Root => None,
            Self::Nested { parent, .. } => Some(Rc::borrow(parent)),
        }
    }
}


/// Zips an element with its ID relative to all the other elements
/// in the graph.
pub struct Linked<T> {
    /// Id of the assembly
    assembly_id: Rc<AssemblyId>,

    /// Id in the containing reactor
    local_id: NodeId,

    /// Value
    /// TODO is pin necessary?
    data: Rc<T>,
}

impl<T> Deref for Linked<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}

impl<T> Debug for Linked<T>
    where T: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}


#[derive(Debug, Eq, PartialEq)]
pub enum NodeKind {
    Input,
    Output,
    Reaction,
    Reactor,
    Action,
    // TODO
}

/// Manages the construction phase.
///
/// The topology has two parallel aspects:
/// - hierarchical relations (containment) forms a tree,
/// eg a reactor, port, etc are contained by a single reactor
/// - dependency relations form a graph, eg an output port may
/// be connected to several input ports of other reactors
///
/// Dependency relations are constructed explicitly by `connect`ing
/// elements. An element must receive an ID in the dependency
/// graph of its reactor, which is what the assembler controls.
///
/// Hierarchy relations are encoded into a path object, the `AssemblyId`,
/// that is carried around during construction. This information
/// is only used at assembly-time, to validate the connections
/// between elements (eg you can only connect ports that are
/// on the same level of the tree).
///
/// The output of assembly is a [RunnableReactor].
///
/// Basic assembly procedure:
///     - assemble all subreactors (this is recursive)
///     - label all subcomponents (using [Linked]). A subreactor
///     is a black box, we ignore its own topology. It still has
///     an ID in the graph of its parent.
///     - connect components using their [Linked] wrapper.
///     This records connections into 2 graphs:
///       1. data dependencies: connections between ports & reactions (not actions).
///       This must be a DAG that orders reactions by priority (toposort).
///       2. trigger dependencies: these are timely dependencies,
///       which may be cyclic (provided they're delayed)
///     - put this all together into a RunnableReactor & we're done.
///
///
pub struct Assembler<T: Reactor> {
    /// The ID of this assembler. This is the path from the
    /// root reactor to this assembly, used for equality
    id: Rc<AssemblyId>,

    dataflow: DepGraph,

    /// This order defines priority of reactions
    reactions: Vec<NodeId>,

    _t_phantom: PhantomData<T>,
}


impl<R: Reactor> Assembler<R> {
    pub fn root() -> Self {
        Assembler {
            id: Rc::new(AssemblyId::Root),
            dataflow: DepGraph::new(),
            reactions: Vec::new(),
            _t_phantom: PhantomData,
        }
    }


    fn subid(&mut self, idx: NodeId) -> AssemblyId {
        Nested {
            parent: Rc::clone(&self.id),
            ext_id: idx,
        }
    }

    pub fn assemble_subreactor<T: Reactor + 'static>(&mut self) -> Linked<RunnableReactor<T>> {
        // get the id before adding the node (this is a hack, see assert below)
        let idx = NodeIndex::new(self.dataflow.node_count());
        let subid = self.subid(idx);

        let mut sub_assembler = Assembler::<T> {
            id: Rc::new(subid),
            dataflow: DepGraph::new(),
            reactions: Vec::new(),
            _t_phantom: PhantomData,
        };

        let state = T::new(&mut sub_assembler);

        let r = RunnableReactor {
            state,
            data_flow: sub_assembler.dataflow,
        };

        let result = self.create_node(r);

        assert_eq!(result.local_id, idx,
                   "Mismatched ID (this means, the code is outdated to work with the petgraph crate, should never happen)");

        result
    }

    pub fn add_reaction(&mut self, reaction: Reaction<R>) -> Linked<Reaction<R>>
        where R: 'static {

        let linked = self.create_node(reaction);
        self.reactions.push(linked.local_id);
        linked
    }

    /// Create a node, associating it a ID in the graph (which
    /// is hidden in the returned Stamped instance).
    ///
    /// The N is for example a port, or a reactor.
    pub fn create_node<N: GraphElement + 'static>(&mut self, elt: N) -> Linked<N> {
        // todo guarantee unicity

        let rc = Rc::new(elt);
        let rc_erased: Rc<dyn GraphElement> = rc.clone();

        let id = self.dataflow.add_node(rc_erased);

        Linked {
            assembly_id: Rc::clone(&self.id),
            local_id: id,
            data: rc,
        }
    }

    pub fn connect<T>(&mut self,
                      upstream: &Linked<OutPort<T>>,
                      downstream: &Linked<InPort<T>>) {
        assert_eq!(upstream.assembly_id.parent(), downstream.assembly_id.parent(),
                   "Connection between ports must be made between sibling reactors");

        downstream.bind(&upstream.data);

        self.dataflow.add_edge(
            upstream.local_id,
            downstream.local_id,
            EdgeTag::PortConnection,
        );
    }


    /// Declare the dependencies of a reaction
    /// Module super::reaction defines a macro to do that easily
    pub fn reaction_link<T>(&mut self,
                            reaction: &Linked<Reaction<R>>,
                            element: &Linked<T>,
                            is_dep: bool) // if false, this is an antidependency
        where T: GraphElement {
        let target_kind = element.data.kind();
        match target_kind {
            NodeKind::Input | NodeKind::Output => {
                if is_dep ^ (target_kind == NodeKind::Input) {
                    // C(reaction) == C(C(port))
                    assert_eq!(Some(reaction.assembly_id.borrow()), element.assembly_id.parent(),
                               "A reaction may only affect input ports of sibling reactors");
                } else {
                    // C(reaction) == C(port)
                    assert_eq!(reaction.assembly_id, element.assembly_id,
                               "A reaction may only depend on input ports of its container")
                }
            }
            NodeKind::Action => {
                assert_eq!(reaction.assembly_id, element.assembly_id,
                           "A reaction may only depend on/schedule the actions of its container");

                unimplemented!("actions");
            }
            NodeKind::Reaction | NodeKind::Reactor => {
                panic!("A reaction cannot declare a dependency on a {:?}", target_kind)
            }
        };

        if is_dep {
            self.dataflow.add_edge(reaction.local_id, element.local_id, EdgeTag::ReactionDep)
        } else {
            self.dataflow.add_edge(element.local_id, reaction.local_id, EdgeTag::ReactionAntiDep)
        };
    }


    // fn stamp<N>(&mut self, elt: N, tag: NodeTag)
}


pub struct RunnableReactor<R: Reactor> {
    /// Strongly typed state (ports, reactions, etc)
    pub(crate) state: R,

    /// The flow graph delimited by inputs & outputs.
    /// This is local to a reactor and not global. It
    /// determines the topological ordering between
    /// reactions & mutations *on an instantaneous time step*
    data_flow: DepGraph,

    // Those ids are local to this reactor's topology

    // Nested reactors are black boxes, which share a single id for all ports

    // inputs: Vec<NodeId>,
    // outputs: Vec<NodeId>,
}

impl<R: Reactor> GraphElement for RunnableReactor<R> {
    fn kind(&self) -> NodeKind {
        NodeKind::Reactor
    }
}

impl<R: Reactor> RunnableReactor<R> {
    fn downstream<N>(&self, stamped: &Linked<N>) -> Neighbors<EdgeTag, NodeIdRepr> {
        self.data_flow.neighbors_directed(stamped.local_id, Direction::Outgoing)
    }
}

