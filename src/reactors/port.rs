use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use super::assembler::{GraphElement, Stamped, NodeKind};

use super::assembler::Assembler;
use std::pin::Pin;

#[derive(Debug)]
pub struct InPort<T> {
    name: &'static str,

    /// The binding for an input port is the output port to
    /// which it is connected.
    ///
    /// RefCell<            // For internal mutability
    /// Option<             // The port may be unbound
    /// Pin<                // The inner reference may not be moved
    /// Rc<                 // The referenced output port may be referenced by several input ports
    /// OutPort<T>>>>>      // Finally the value (which in fact is in its own refcell)
    ///
    binding: RefCell<Option<Pin<Rc<OutPort<T>>>>>,
}

impl<T> GraphElement for InPort<T> {
    fn kind(&self) -> NodeKind {
        NodeKind::Input
    }
}

impl<T> InPort<T> {
    pub fn new(assembler: &mut Assembler, name: &'static str) -> Stamped<InPort<T>>
        where T: 'static {
        assembler.create_node(
            InPort {
                name,
                binding: RefCell::new(None),
            }
        )
    }

    pub fn bind(&self, binding: &Pin<Rc<OutPort<T>>>) {
        // it's important that the borrow here is dropped before borrow_mut is called
        //                                        vvvvvv
        if let Some(b) = self.binding.borrow().as_deref() {
            panic!("Input port {} already bound to {}", self.name, b.name)
        }
        *self.binding.borrow_mut() = Some(binding.clone())
    }

    pub fn borrow_or_panic(&self) -> Pin<Rc<OutPort<T>>> {
        let x = self.binding.borrow();
        match &*x {
            Some(output) => Pin::clone(output),
            None => panic!("No binding for port {}", self.name)
        }
    }
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
    pub fn new(assembler: &mut Assembler, name: &'static str, initial_val: T) -> Stamped<OutPort<T>>
        where T: 'static, {
        assembler.create_node(
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
}
