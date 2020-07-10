use std::any::Any;

use super::port::{InPort, OutPort, Port};
use std::rc::Rc;

/// Trait for a reactor.
pub trait Reactor<'a> {

    // TODO reify reactions
}


/// The World reactor is the toplevel reactor. It has no output
/// or output ports, no state.
/// TODO this is not needed if reactors manage themselves the creation of their ReactorGraph
pub struct World {}

impl World {
    pub fn new() -> Self {
        let mut world = World {};


        world
    }
}
//
// impl Reactor<'static> for World {
//     fn ports(&self) -> Vec<Box<Port<'static, ()>>> {
//         vec![]
//     }
// }


// Dummy reactor implementations

#[derive(Debug)]
pub struct ProduceReactor {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub value: Rc<OutPort<i32>>
}

impl ProduceReactor {
    pub fn new() -> Self {
        ProduceReactor {
            value: Rc::new(OutPort::new("value", 0))
        }
    }
}

#[derive(Debug)]
pub struct ConsumeReactor {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub input: InPort<i32>
}

impl ConsumeReactor {
    pub fn new() -> Self {
        ConsumeReactor { input: InPort::new("value") }
    }

    pub fn emit(&self) {
        let v = *self.input.borrow_or_panic();
        println!("{}", v)
    }
}


//
// impl<'a> Reactor<'a> for ProduceReactor {
//     fn ports(&self) -> Vec<Box<Port<'a, dyn Any>>> {
//         vec![
//             Box::new(Port::Output(&self.value)),
//         ]
//         // Vec::new()
//         // &[
//         //
//         // ]
//     }
// }
