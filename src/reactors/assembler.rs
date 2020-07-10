use std::any::Any;
use std::borrow::Borrow;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

use petgraph::Graph;
use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;

use crate::reactors::port::{InPort, OutPort};
use crate::reactors::reactor::Reactor;

type NodeIdRepr = u32;
type NodeId = NodeIndex<NodeIdRepr>;


pub trait GraphElement<'a> {}

struct EdgeTag {}


type NodeData<'a> = Rc<dyn GraphElement<'a>>;

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

    parent: Option<&'a Assembler<'a>>,

}


/// Zips an element with its global graph id
pub struct Stamped<'a, T> {
    id: NodeId,
    pub data: Rc<T>,

    life: PhantomData<&'a ()>,
}

impl<'a, T> Debug for Stamped<'a, T>
    where T: Debug {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.data.fmt(f)
    }
}


impl<'a> Assembler<'a> {
    pub fn new() -> Assembler<'a> {
        Assembler {
            parent: None,
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
    pub fn create_node<N: GraphElement<'a> + 'static>(&mut self, elt: N) -> Stamped<'a, N> {
        // let elt_box: Rc<N> = Box::new(elt);
        // let mut upcast: Rc<dyn GraphElement<'a>> = Box::new(elt_box.);

        let rc = Rc::new(elt);
        let rc_erased: Rc<dyn GraphElement<'a>> = rc.clone();

        let id = self.graph.add_node(rc_erased);

        Stamped {
            id,
            data: Rc::clone(&rc),
            life: PhantomData,
        }
    }

    pub fn connect<T>(&mut self,
                      upstream: &Stamped<'a, OutPort<T>>,
                      downstream: &Stamped<'a, InPort<T>>) {
        // todo assertions

        downstream.data.bind(&upstream.data);


        self.graph.add_edge(
            upstream.id,
            downstream.id,
            EdgeTag {},
        );
    }


    // fn stamp<N>(&mut self, elt: N, tag: NodeTag)
}
