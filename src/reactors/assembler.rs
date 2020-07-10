use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::rc::Rc;

use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;

use crate::reactors::port::{InPort, OutPort};
use crate::reactors::reactor::Reactor;
use crate::reactors::reaction::Reaction;
use std::ops::Deref;
use std::pin::Pin;

type NodeIdRepr = u32;
type NodeId = NodeIndex<NodeIdRepr>;


pub trait GraphElement<'a> {
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


type NodeData<'a> = Pin<Rc<dyn GraphElement<'a>>>;

/// The dependency graph between structures
type DepGraph<'a> = DiGraph<NodeData<'a>, EdgeTag, NodeIdRepr>;


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
pub struct Assembler<'a> {
    // TODO the assembler should be a zipper
    graph: DepGraph<'a>,

    parent: Option<Box<Assembler<'a>>>,
}


/// Zips an element with its global graph id
pub struct Stamped<'a, T> {
    id: NodeId,
    data: Pin<Rc<T>>,

    life: PhantomData<&'a ()>,
}

impl<'a, T> Deref for Stamped<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.deref()
    }
}

impl<'a, T> Debug for Stamped<'a, T>
    where T: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}


pub enum NodeKind {
    Input,
    Output,
    Reaction,
    Reactor,
    // TODO
}


impl<'a> Assembler<'a> {
    pub fn new(parent: Option<Box<Assembler<'a>>>) -> Self {
        Assembler {
            parent: parent,
            graph: DepGraph::new(),
        }
    }


    pub fn root() -> Self {
        Self::new(None)
    }

    /// Create a node, associating it a ID in the graph (which
    /// is hidden in the returned Stamped instance).
    ///
    /// The N is for example a port, or a reactor. The returned
    /// value is used to associate topological info with the node
    /// (todo hierarchy relations + dependency relations)
    ///
    pub fn create_node<N: GraphElement<'a> + 'a>(&mut self, elt: N) -> Stamped<'a, N> {
        // let elt_box: Rc<N> = Box::new(elt);
        // let mut upcast: Rc<dyn GraphElement<'a>> = Box::new(elt_box.);

        let rc = Rc::pin(elt);
        let rc_erased: Pin<Rc<dyn GraphElement<'a> + 'a>> = rc.clone();

        let id = self.graph.add_node(rc_erased);

        Stamped {
            id,
            data: rc,
            life: PhantomData,
        }
    }

    pub fn connect<T>(&mut self,
                      upstream: &Stamped<'a, OutPort<T>>,
                      downstream: &Stamped<'a, InPort<T>>) {
        // todo assertions

        downstream.bind(&upstream.data);


        self.graph.add_edge(
            upstream.id,
            downstream.id,
            EdgeTag::PortConnection,
        );
    }

    pub fn reaction_link<T, R>(&mut self,
                               reaction: Stamped<'a, Reaction<'a, R>>,
                               element: Stamped<'a, T>,
                               fwd: bool)
        where T: GraphElement<'a>, R: Reactor<'a> {

        let tag = if fwd {
            EdgeTag::ReactionDep
        } else {
            EdgeTag::ReactionAntiDep
        };

        match element.data.kind() {
            NodeKind::Input => {
                // validity
                //     fwd && C(p) == self.reactor      => dependency on container input
                //  or !fwd && C(C(p)) == self.reactor  => antidependency on sibling output

                self.graph.add_edge(
                    reaction.id,
                    element.id,
                    tag,
                )
            }
            NodeKind::Output => {
                // validity
                //     !fwd && C(p) == self.reactor     => antidependency on container output
                //  or fwd && C(C(p)) == self.reactor   => dependency on sibling output

                self.graph.add_edge(
                    reaction.id,
                    element.id,
                    tag,
                )
            }
            _ => {
                unimplemented!();
            }
        };
    }


    // fn stamp<N>(&mut self, elt: N, tag: NodeTag)
}
