use std::any::Any;

use super::port::{InPort, OutPort, Port};
use super::reaction::{Reaction};
use std::rc::Rc;
use crate::reactors::assembler::{Assembler, Stamped, GraphElement};

/// Trait for a reactor.
pub trait Reactor<'a> {
    // Translation strategy:
    // Ports are struct fields
    // Reactions are implemented with regular methods, described by a Reaction

    fn reactions(&self) -> Vec<Reaction<'a, Self>> where Self: Sized;
}


impl<'a, T> GraphElement<'a> for T
    where T: Reactor<'a> + Sized {}


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
pub struct ConsumeReactor<'a> {
    /// This is the ouput port, that should be borrowed
    // output: RefCell<i32>,
    // output_borrower: Rc<RefCell<i32>>,

    pub input: Stamped<'a, InPort<i32>>
}

impl<'a> ConsumeReactor<'a> {
    pub fn new(assembler: &mut Assembler<'a>) -> Self {
        ConsumeReactor { input: InPort::new(assembler, "value") }
    }
}


impl<'a> Reactor<'a> for ConsumeReactor<'a> {
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
