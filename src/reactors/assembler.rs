//! Assembler logic
//!
//! This contains the logic to build a reactor graph, upon
//! initialization of the program.


use std::fmt::{Debug, Formatter};
use std::rc::Rc;

use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;

use super::port::{InPort, OutPort};
use super::reactor::Reactor;
use super::reaction::Reaction;
use std::ops::Deref;
use std::pin::Pin;

type NodeIdRepr = u32;
pub(crate) type NodeId = NodeIndex<NodeIdRepr>;


pub trait GraphElement {
    fn kind(&self) -> NodeKind;
}

enum EdgeTag {
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


type NodeData = Pin<Rc<dyn GraphElement>>;

/// The dependency graph between structures
type DepGraph = DiGraph<NodeData, EdgeTag, NodeIdRepr>;


/// Manages the construction phase.
///
/// The topology has two parallel aspects:
/// - hierarchical relations (containment) forms a tree,
/// eg a reactor, port, etc are contained by a single reactor
/// - dependency relations form a graph, eg an output port may
/// be connected to several input ports of other reactors
///
/// Hierarchy relations are ideally built implicitly by the assembler.
/// It should remember the current container, and every time we add a
/// node, it should be linked that way.
///
/// Dependency relations are constructed explicitly by `connect`ing
/// elements. This uses the hierarchical information to validate the
/// structure (eg you can only connect ports that are on the same level
/// of the tree).
///
/// Internally the dependencies are encoded into a graph, which is the
/// output of the assembly -> should be passed to the scheduler later
///
pub struct Assembler {
    graph: DepGraph,

    // TODO the idea is to form a tree of assemblers (bottom up),
    //  to check the validity of dependencies (eg the level of a node)
    // children: Vec<Box<Assembler>>
}


/// Zips an element with its global graph id
pub struct Stamped<T> {
    id: NodeId,
    data: Pin<Rc<T>>,
}

impl<T> Deref for Stamped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}

impl<T> Debug for Stamped<T>
    where T: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}


#[derive(Debug)]
pub enum NodeKind {
    Input,
    Output,
    Reaction,
    Reactor,
    // TODO
}


impl Assembler {
    pub fn new() -> Self {
        Assembler {
            graph: DepGraph::new(),
        }
    }

    /// Create a node, associating it a ID in the graph (which
    /// is hidden in the returned Stamped instance).
    ///
    /// The N is for example a port, or a reactor. The returned
    /// value is used to associate topological info with the node
    /// (todo hierarchy relations + dependency relations)
    ///
    pub fn create_node<N: GraphElement + 'static>(&mut self, elt: N) -> Stamped<N> {
        // todo guarantee unicity

        let rc = Rc::pin(elt);
        let rc_erased: Pin<Rc<dyn GraphElement>> = rc.clone();

        let id = self.graph.add_node(rc_erased);

        Stamped {
            id,
            data: rc,
        }
    }

    pub fn connect<T>(&mut self,
                      upstream: &Stamped<OutPort<T>>,
                      downstream: &Stamped<InPort<T>>) {
        // todo assertions

        downstream.bind(&upstream.data);


        self.graph.add_edge(
            upstream.id,
            downstream.id,
            EdgeTag::PortConnection,
        );
    }

    pub fn reaction_link<T, R>(&mut self,
                               reaction: &Stamped<Reaction<R>>,
                               element: &Stamped<T>,
                               fwd: bool)
        where T: GraphElement, R: Reactor {
        let tag = if fwd {
            EdgeTag::ReactionDep
        } else {
            EdgeTag::ReactionAntiDep
        };

        match element.data.kind() {
            NodeKind::Input => {
                // todo validity
                //     fwd && C(p) == self.reactor      => dependency on container input
                //  or !fwd && C(C(p)) == self.reactor  => antidependency on sibling output

                self.graph.add_edge(
                    reaction.id,
                    element.id,
                    tag,
                )
            }
            NodeKind::Output => {
                // todo validity
                //     !fwd && C(p) == self.reactor     => antidependency on container output
                //  or fwd && C(C(p)) == self.reactor   => dependency on sibling output

                self.graph.add_edge(
                    reaction.id,
                    element.id,
                    tag,
                )
            }
            NodeKind::Reaction | NodeKind::Reactor => {
                panic!("A reaction cannot declare a dependency on a {:?}", element.data.kind())
            }
        };
    }


    // fn stamp<N>(&mut self, elt: N, tag: NodeTag)
}
