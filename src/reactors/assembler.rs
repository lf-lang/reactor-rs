//! Assembler logic
//!
//! This contains the logic to build a reactor graph, upon
//! initialization of the program.


use std::fmt::{Debug, Formatter, Display};
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
use std::any::type_name;
use crate::reactors::action::Action;

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
#[derive(Eq, PartialEq, Clone)]
enum AssemblyId {
    Root,
    Nested {
        // This is the node id used in the parent
        ext_id: NodeId,
        // the id of the parent
        parent: Rc<AssemblyId>,

        // this is just for debugging
        typename: &'static str,
    },
}

impl Display for AssemblyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root => write!(f, ""),
            AssemblyId::Nested { typename, ext_id, parent } => {
                Debug::fmt(parent, f);
                write!(f, "/{}[{}]", typename, ext_id.index())
            }
        }
    }
}

impl Debug for AssemblyId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
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

    data_flow: DepGraph,

    actions: Vec<NodeId>,

    /// This order defines priority of reactions
    reactions: Vec<NodeId>,

    // These remain stable, even in case of internal mutation of the reactor
    inputs: Vec<NodeId>,
    outputs: Vec<NodeId>,

    _t_phantom: PhantomData<T>,
}


impl<R: Reactor> Assembler<R> {
    pub fn root() -> Self {
        Self::new(Rc::new(AssemblyId::Root))
    }

    fn new(id: Rc<AssemblyId>) -> Self {
        Assembler {
            id,
            data_flow: DepGraph::new(),
            reactions: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            actions: Vec::new(),
            _t_phantom: PhantomData,
        }
    }

    fn subid<T: Reactor>(&mut self, idx: NodeId) -> AssemblyId {
        Nested {
            parent: Rc::clone(&self.id),
            ext_id: idx,
            typename: type_name::<T>(),
        }
    }

    pub fn assemble_subreactor<T: Reactor + 'static>(&mut self) -> Linked<RunnableReactor<T>> {
        // get the id before adding the node (this is a hack, see assert below)
        let idx = NodeIndex::new(self.data_flow.node_count());
        let subid = self.subid::<T>(idx);

        let mut sub_assembler = Assembler::<T>::new(Rc::new(subid));

        let state = T::new(&mut sub_assembler);

        let r = RunnableReactor {
            id: sub_assembler.id,
            state,
            data_flow: sub_assembler.data_flow,
        };

        let result = self.create_node(r);

        assert_eq!(result.local_id, idx,
                   "Mismatched ID (this means, the code is outdated to work with the petgraph crate, should never happen)");

        result
    }

    pub fn declare_input<T: 'static>(&mut self, port: InPort<T>) -> Linked<InPort<T>> {
        let linked = self.create_node(port);
        self.inputs.push(linked.local_id);
        linked
    }

    pub fn declare_output<T: 'static>(&mut self, port: OutPort<T>) -> Linked<OutPort<T>> {
        let linked = self.create_node(port);
        self.outputs.push(linked.local_id);
        linked
    }

    pub fn declare_reaction(&mut self, reaction: Reaction<R>) -> Linked<Reaction<R>>
        where R: 'static {
        let linked = self.create_node(reaction);
        self.reactions.push(linked.local_id);
        linked
    }

    pub fn declare_action(&mut self, action: Action) -> Linked<Action> {
        let linked = self.create_node(action);
        self.actions.push(linked.local_id);
        linked
    }

    /// Create a node, associating it a ID in the graph (which
    /// is hidden in the returned Stamped instance).
    ///
    /// The N is for example a port, or a reactor.
    fn create_node<N: GraphElement + 'static>(&mut self, elt: N) -> Linked<N> {
        // todo guarantee unicity

        let rc = Rc::new(elt);
        let rc_erased: Rc<dyn GraphElement> = rc.clone();

        let id = self.data_flow.add_node(rc_erased);

        Linked {
            assembly_id: Rc::clone(&self.id),
            local_id: id,
            data: rc,
        }
    }

    pub fn connect<T>(&mut self,
                      upstream: &Linked<OutPort<T>>,
                      downstream: &Linked<InPort<T>>) {
        assert_eq!(upstream.assembly_id.parent(), Some(self.id.borrow()),
                   "Cannot connect outside of this reactor");
        assert_eq!(upstream.assembly_id.parent(), downstream.assembly_id.parent(),
                   "Connection between ports must be made between sibling reactors");

        downstream.bind(&upstream.data);

        self.data_flow.add_edge(
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
                               "A reaction may only depend on input ports of its container");
                }

                // TODO those IDs are not local
                if is_dep {
                    // reaction uses the item, data flows from element to reaction
                    self.data_flow.add_edge(element.local_id, reaction.local_id, EdgeTag::ReactionDep)
                } else {
                    self.data_flow.add_edge(reaction.local_id, element.local_id, EdgeTag::ReactionAntiDep)
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
    }
}

// this is private to the assembler impl
macro_rules! record_node {
        ($node:expr, $vec:expr) => {
             let linked = self.create_node($node);
             $vec.push(linked.local_id);
             linked
        };
    }

/// Declares the dependencies of a reaction on ports & actions
#[macro_export]
macro_rules! link_reaction {
    {($reaction:expr) with ($assembler:expr) (uses $( $dep:expr )*) (affects $( $anti:expr )*)} => {

        {
            $(
                $assembler.reaction_link($reaction, $dep, true);
            )*
            $(
                $assembler.reaction_link($reaction, $anti, false);
            )*
        }
    };
}


pub struct RunnableReactor<R: Reactor> {
    id: Rc<AssemblyId>,

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

impl<R: Reactor> Debug for RunnableReactor<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.id, f)
    }
}

impl<R: Reactor> RunnableReactor<R> {
    fn downstream<N>(&self, stamped: &Linked<N>) -> Neighbors<EdgeTag, NodeIdRepr> {
        self.data_flow.neighbors_directed(stamped.local_id, Direction::Outgoing)
    }
}

