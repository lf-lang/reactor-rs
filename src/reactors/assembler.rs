//! Assembler logic
//!
//! This contains the logic to build a reactor graph, upon
//! initialization of the program.


use std::any::type_name;
use std::borrow::{Borrow, BorrowMut};
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::process::id;
use std::rc::Rc;

use petgraph::algo::toposort;
use petgraph::Direction;
use petgraph::graph::{DiGraph, Neighbors};
use petgraph::graph::NodeIndex;

use crate::reactors::action::Action;
use crate::reactors::assembler::AssemblyId::Nested;

use super::port::{InPort, OutPort};
use super::reaction::Reaction;
use super::reactor::Reactor;

type NodeIdRepr = u32;
type NodeId = NodeIndex<NodeIdRepr>;


pub trait GraphElement {
    fn kind(&self) -> NodeKind;
    fn name(&self) -> &'static str;
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
    /*
       N: Reaction -> M: Reaction
       means N has greater priority than M
     */
    ReactionPriority,
}


type NodeData<'a> = Rc<dyn GraphElement + 'a>;

/// The dependency graph between structures
type DepGraph<'a> = DiGraph<NodeData<'a>, EdgeTag, NodeIdRepr>;


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
                Debug::fmt(parent, f)?;
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

#[derive(Eq, PartialEq, Clone)]
struct GlobalId {
    assembly_id: Rc<AssemblyId>,
    local_id: NodeId,

    kind: NodeKind,
    name: &'static str,
}

impl Debug for GlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.assembly_id, f)?;
        write!(f, "/{}[{}]: {}", self.kind, self.local_id.index(), self.name)
    }
}


/// Zips an element with its ID relative to all the other elements
/// in the graph.
pub struct Linked<T> {
    id: GlobalId,

    /// Value
    /// TODO is pin necessary?
    data: Rc<T>,
}

impl<T> Linked<T> {
    fn assembly_id(&self) -> &AssemblyId {
        self.id.assembly_id.borrow()
    }

    fn local_id(&self) -> NodeId {
        self.id.local_id
    }
}

impl<T> Deref for Linked<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}


impl<'b, R: Reactor> DerefMut for Linked<RunnableReactor<'b, R>> {
    fn deref_mut(&mut self) -> &mut RunnableReactor<'b, R> {
        (&self.data).borrow_mut()
    }
}

impl<T> Debug for Linked<T>
    where T: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}


#[derive(Debug, Eq, PartialEq, Clone)]
pub enum NodeKind {
    Input,
    Output,
    Reaction,
    Reactor,
    Action,
    // TODO
}

impl Display for NodeKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
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
pub struct Assembler<'a, T: Reactor + 'a> {
    /// The ID of this assembler. This is the path from the
    /// root reactor to this assembly, used for equality
    id: Rc<AssemblyId>,

    data_flow: DepGraph<'a>,

    actions: Vec<NodeId>,

    /// This order defines priority of reactions
    reactions: Vec<NodeId>,

    // These remain stable, even in case of internal mutation of the reactor
    inputs: Vec<NodeId>,
    outputs: Vec<NodeId>,

    _t_phantom: PhantomData<T>,
}


impl<'a, R: Reactor> Assembler<'a, R> {
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

    pub fn assemble_subreactor<'b: 'a, T: Reactor + 'b>(&mut self) -> Linked<RunnableReactor<'b, T>> {
        // get the id before adding the node (this is a hack, see assert below)
        let idx = NodeIndex::new(self.data_flow.node_count());
        let subid = self.subid::<T>(idx);

        let mut sub_assembler = Assembler::<'b, T>::new(Rc::new(subid));

        let mut state = Box::new(T::new(&mut sub_assembler));

        // Add priority links
        for (r0, r1) in sub_assembler.reactions.iter().zip(sub_assembler.reactions.iter().skip(1)) {
            // r0 has higher priority than r1
            sub_assembler.data_flow.add_edge(*r0, *r1, EdgeTag::ReactionPriority);
        }

        let r = RunnableReactor::<'b, T>::new(state, sub_assembler);

        let result = self.create_node(r);

        assert_eq!(result.local_id(), idx,
                   "Mismatched ID (this means, the code is outdated to work with the petgraph crate, should never happen)");

        result
    }

    pub fn declare_input<T: 'a>(&mut self, port: InPort<T>) -> Linked<InPort<T>> {
        let linked = self.create_node(port);
        self.inputs.push(linked.local_id());
        linked
    }

    pub fn declare_output<T: 'a>(&mut self, port: OutPort<T>) -> Linked<OutPort<T>> {
        let linked = self.create_node(port);
        self.outputs.push(linked.local_id());
        linked
    }

    pub fn declare_reaction<'b>(&mut self, reaction: Reaction<'b, R>) -> Linked<Reaction<'b, R>> where 'b : 'a {
        let linked = self.create_node(reaction);
        self.reactions.push(linked.local_id());
        linked
    }

    pub fn declare_action(&mut self, action: Action) -> Linked<Action> {
        let linked = self.create_node(action);
        self.actions.push(linked.local_id());
        linked
    }

    /// Create a node, associating it a ID in the graph (which
    /// is hidden in the returned Stamped instance).
    ///
    /// The N is for example a port, or a reactor.
    fn create_node<N: GraphElement + 'a>(&mut self, elt: N) -> Linked<N> {
        // todo guarantee unicity

        let rc = Rc::new(elt);
        let rc_erased: Rc<dyn GraphElement> = rc.clone();

        let id = self.data_flow.add_node(rc_erased);

        Linked {
            id: GlobalId {
                assembly_id: Rc::clone(&self.id),
                local_id: id,
                kind: (&rc).kind(),
                name: (&rc).name(),
            },
            data: rc,
        }
    }

    pub fn connect<T>(&mut self,
                      upstream: &Linked<OutPort<T>>,
                      downstream: &Linked<InPort<T>>) {
        assert_eq!(upstream.assembly_id().parent(), Some(self.id.borrow()),
                   "Cannot connect outside of this reactor");
        assert_eq!(upstream.assembly_id().parent(), downstream.assembly_id().parent(),
                   "Connection between ports must be made between sibling reactors");

        downstream.bind(&upstream.data);

        self.data_flow.add_edge(
            upstream.local_id(),
            downstream.local_id(),
            EdgeTag::PortConnection,
        );
    }


    /// Declare the dependencies of a reaction
    /// Module super::reaction defines a macro to do that easily
    pub fn reaction_link<T>(&mut self,
                            reaction: &Linked<Reaction<'a, R>>,
                            element: &Linked<T>,
                            is_dep: bool) // if false, this is an antidependency
        where T: GraphElement + 'a {
        let target_kind = element.data.kind();
        match target_kind {
            NodeKind::Input | NodeKind::Output => {
                if is_dep ^ (target_kind == NodeKind::Input) {
                    // C(reaction) == C(C(port))
                    assert_eq!(Some(reaction.assembly_id()), element.assembly_id().parent(),
                               "A reaction may only affect input ports of sibling reactors");
                } else {
                    // C(reaction) == C(port)
                    assert_eq!(reaction.assembly_id(), element.assembly_id(),
                               "A reaction may only depend on input ports of its container");
                }

                // TODO those IDs are not local
                if is_dep {
                    // reaction uses the item, data flows from element to reaction
                    self.data_flow.add_edge(element.local_id(), reaction.local_id(), EdgeTag::ReactionDep)
                } else {
                    self.data_flow.add_edge(reaction.local_id(), element.local_id(), EdgeTag::ReactionAntiDep)
                }
            }
            NodeKind::Action => {
                assert_eq!(reaction.assembly_id(), element.assembly_id(),
                           "A reaction may only depend on/schedule the actions of its container");

                unimplemented!("actions");
            }
            NodeKind::Reaction | NodeKind::Reactor => {
                panic!("A reaction cannot declare a dependency on a {:?}", target_kind)
            }
        };
    }
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


pub struct RunnableReactor<'a, R: Reactor> {
    id: Rc<AssemblyId>,

    /// Strongly typed state (ports, reactions, etc)
    pub(crate) state: Box<R>,

    /// The flow graph delimited by inputs & outputs.
    /// This is local to a reactor and not global. It
    /// determines the topological ordering between
    /// reactions & mutations *on an instantaneous time step*
    data_flow: DepGraph<'a>,

    // Those ids are local to this reactor's topology

    // Nested reactors are black boxes, which share a single id for all ports

    inputs: Vec<NodeId>,
    outputs: Vec<NodeId>,
}

impl<R: Reactor> GraphElement for RunnableReactor<'_, R> {
    fn kind(&self) -> NodeKind {
        NodeKind::Reactor
    }

    fn name(&self) -> &'static str {
        type_name::<R>()
    }
}

impl<R: Reactor> Debug for RunnableReactor<'_, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.id, f)
    }
}

impl<'a, R: Reactor> RunnableReactor<'a, R> {
    fn new(mut state: Box<R>, assembler: Assembler<'a, R>) -> RunnableReactor<'a, R> {
        RunnableReactor {
            id: assembler.id,
            state,
            data_flow: assembler.data_flow,
            inputs: assembler.inputs,
            outputs: assembler.outputs,
        }
    }


    fn downstream<N>(&self, stamped: &Linked<N>) -> impl Iterator<Item=NodeId> + '_ {
        self.data_flow.neighbors_directed(stamped.local_id(), Direction::Outgoing)
    }
}

