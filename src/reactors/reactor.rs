use std::any::Any;

use super::port::{InPort, OutPort, Port};
use super::reaction::{Reaction};
use std::rc::Rc;

/// Trait for a reactor.
pub trait Reactor<'a> where Self: Sized {
    // Translation strategy:
    // Ports are struct fields
    // Reactions are implemented with regular methods, described by a Reaction

    fn reactions(&self) -> Vec<Reaction<'a, Self>>;
}

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

    fn incr_value(&self) {
        *self.value.get_mut() += 2
    }
}

impl<'a> Reactor<'a> for ProduceReactor {
    fn reactions(&self) -> Vec<Reaction<'a, Self>> {
        vec![
            // reaction! {
            //     "incr"("value") -> () {
            //         reactor.incr_value()
            //     }
            // }
            Reaction::new(
                "incr",
                vec![
                    "value",
                ],
                |reactor| {
                    {
                        reactor.incr_value()
                    }
                },
            )
        ]
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
}


impl<'a> Reactor<'a> for ConsumeReactor {
    fn reactions(&self) -> Vec<Reaction<'a, Self>> {
        vec![
            reaction! {
                "print_input"("input") -> (input) {
                    println!("{}", input)
                }
            }
        ]
    }
}
