use std::cell::{RefCell, Ref};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use super::assembler::{GraphElement, Linked, NodeKind};

use super::assembler::Assembler;
use std::pin::Pin;
use crate::reactors::reactor::Reactor;

#[derive(Debug)]
pub struct InPort<T> {
    name: &'static str,

    /// The binding for an input port is the output port to
    /// which it is connected.
    ///
    /// todo this scheme is not thread-safe, abandon internal mutability?
    ///
    /// RefCell<            // For internal mutability
    /// Option<             // The port may be unbound
    /// Rc<                 // The referenced output port may be referenced by several input ports
    /// OutPort<T>>>>       // Finally the value (which in fact is in its own refcell)
    ///
    binding: RefCell<Option<Rc<OutPort<T>>>>,
}

impl<T> GraphElement for InPort<T> {
    fn kind(&self) -> NodeKind {
        NodeKind::Input
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

impl<T> InPort<T> {
    pub fn new<'a, R: Reactor + 'a>(assembler: &mut Assembler<'a, R>,
                                    name: &'static str) -> Linked<InPort<T>> where T: 'a {
        assembler.declare_input(
            InPort {
                name,
                binding: RefCell::new(None),
            }
        )
    }

    pub fn bind(&self, binding: &Rc<OutPort<T>>) {
        // it's important that the borrow here is dropped before borrow_mut is called
        //                                        vvvvvv
        if let Some(b) = self.binding.borrow().as_deref() {
            panic!("Input port {} already bound to {}", self.name, b.name)
        }
        *self.binding.borrow_mut() = Some(binding.clone())
    }

    pub fn borrow_or_panic(&self) -> Rc<OutPort<T>> {
        let x = self.binding.borrow();
        match &*x {
            Some(output) => Rc::clone(output),
            None => panic!("No binding for port {}", self.name)
        }
    }
}

// Get the current value, or panics
#[macro_export]
macro_rules! port_value {
    [$port:expr] => {*($port).borrow_or_panic().get()};
}


/// An OutPort is an internally mutable container for a value
/// In reactors, ports should be wrapped inside an Rc; when
/// linked to an input port of another reactor, that Rc should
/// be cloned.
#[derive(Debug)]
pub struct OutPort<T> {
    name: &'static str,
    /// Mutable container for the value
    cell: RefCell<T>,
}


impl<T> OutPort<T> {
    pub fn new<'a, R: Reactor + 'a>(assembler: &mut Assembler<'a, R>,
                                    name: &'static str,
                                    initial_val: T) -> Linked<OutPort<T>> where T: 'a, {
        assembler.declare_output(
            OutPort { name, cell: RefCell::new(initial_val) }
        )
    }

    pub fn set(&self, new_val: T) {
        *self.cell.borrow_mut() = new_val
    }

    pub fn get(&self) -> impl Deref<Target=T> + '_ {
        self.cell.borrow()
    }

    pub fn get_mut(&self) -> impl DerefMut<Target=T> + '_ {
        self.cell.borrow_mut()
    }
}

impl<T> GraphElement for OutPort<T> {
    fn kind(&self) -> NodeKind {
        NodeKind::Output
    }

    fn name(&self) -> &'static str {
        self.name
    }
}
