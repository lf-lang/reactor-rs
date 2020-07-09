use super::port::Port;
use std::cell::RefCell;
use std::collections::HashMap;
use std::any::Any;

/// Trait for a reactor.
pub trait Reactor {
    /// The set of output & input ports.
    /// This is manipulated by the container reactor, as part
    /// of a ReactorGraph
    fn ports(&self) -> &[Port];

    // TODO reify reactions
}

type RPort = (Box<dyn Reactor>, Port);

/// A reactor graph describes the internal topology of a reactor.
/// It connects ports of the sub-reactors of a container.
struct ReactorGraph {
    // TODO connections + use zipper idea
    nodes: Vec<Box<dyn Reactor>>,
    // map of input port to their connection
    edges: HashMap<RPort, RPort>,
}

impl ReactorGraph {
    fn empty() -> ReactorGraph {
        ReactorGraph { nodes: Vec::new(), edges: HashMap::new() }
    }
}


/// The World reactor is the toplevel reactor. It has no output
/// or output ports, no state.
/// TODO this is not needed if reactors manage themselves the creation of their ReactorGraph
pub struct World {}

impl World {
    const NO_PORTS: [Port; 0] = [];


    pub fn new() -> Self {
        World {}
    }
}

impl Reactor for World {
    fn ports(&self) -> &[Port] {
        &Self::NO_PORTS
    }
}


// Dummy reactor implementations

#[derive(Debug)]
pub struct ProduceReactor {
    state: i32,
}

impl ProduceReactor {
    const PORTS: [Port; 1] = [Port::Output("value")];

    pub fn new() -> Self {
        ProduceReactor { state: 4 }
    }
}

impl Reactor for ProduceReactor {
    fn ports(&self) -> &[Port] {
        &Self::PORTS
    }
}
