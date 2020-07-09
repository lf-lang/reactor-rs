use super::port::{OutPort, InPort, Port};
use std::cell::RefCell;



/// A reactor contains other reactors and has input and output
/// ports to connect it to other reactors in the enclosing reactor.
///
/// The top-level reactor (root of the reactor tree) has no outputs
/// or inputs.
///
/// The lifetime parameter 'a corresponds to the lifetime of this
/// reactor, but since the children need to have the same lifetime,
/// basically this corresponds to the lifetime of the root.
///
/// Trees are constructed top-down, they probably will support
/// mutation but do so by reconstructing part of the tree (functional
/// style). We have to see whether this is desirable behavior,
/// as for example the external resources a reactor holds-on to
/// may not be cloned.
///
///
/// TODO better to build the trees bottom up?
///
/// TODO would it be better to have Reactor a trait, and a separate
///    impl for every different reactor?
///
pub struct Reactor<'a> {
    outputs: Vec<OutPort>,
    inputs: Vec<InPort<'a>>,

    /// Contained reactors
    children: Vec<&'a Reactor<'a>>,
}


impl<'a> Reactor<'a> {
    pub fn new() -> Reactor<'a> {
        Reactor {
            outputs: Vec::new(),
            inputs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: &'a Reactor<'a>) {
        self.children.push(child);
    }

    // pub fn connect_children(&mut self)
}
