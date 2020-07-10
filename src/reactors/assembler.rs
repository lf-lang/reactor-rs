use std::hash::Hash;

use petgraph::Graph;
use petgraph::stable_graph::StableDiGraph;
use crate::reactors::reactor::Reactor;
use crate::reactors::port::{Port, InPort};
use petgraph::graph::DiGraph;
use std::any::Any;
use petgraph::graph::NodeIndex;
use std::borrow::Borrow;
use std::rc::Rc;
use std::marker::PhantomData;
use std::fmt::{Debug, Formatter};


type NodeIdRepr = u32;
type NodeId = NodeIndex<NodeIdRepr>;


pub trait GraphElement<'a> {}

enum EdgeTag {}


type NodeData<'a> = Rc<Box<&'a dyn GraphElement<'a>>>;

/// The dependency graph between structures
type DepGraph<'a> = DiGraph<NodeData<'a>, EdgeTag, NodeIdRepr>;


/// Manages construction of the global topology
/// Everything needs to be assigned a global ID
pub struct Assembler<'a> {
    graph: &'a DepGraph<'a>,

    parent: Option<&'a Assembler<'a>>,

}


/// Zips an element with its global graph id
pub struct Stamped<'a, T> {
    id: NodeId,
    data: Rc<Box<T>>,

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
            graph: &DepGraph::new(),
        }
    }

    /// Create a node, associating it a ID in the graph (which
    /// is hidden in the returned Stamped instance).
    ///
    pub fn create_node<N: GraphElement<'a> + 'a>(&mut self, elt: N) -> Stamped<'a, N> {
        let elt_box: Box<N> = Box::new(elt);
        let upcast: Box<&'a dyn GraphElement<'a>> = Box::new(&elt);
        let rc = Rc::new(elt_box);
        // let elt_box_erased: Box<&'a dyn GraphElement> = elt_box;
        let id = self.graph.add_node(Rc::new(upcast));


        Stamped {
            id,
            data: Rc::clone(&rc),
            life: PhantomData,
        }
    }

    // fn stamp<N>(&mut self, elt: N, tag: NodeTag)
}
