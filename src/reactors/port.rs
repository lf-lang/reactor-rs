use std::cell::{RefCell, Ref};

use std::rc::Rc;
use std::borrow::Borrow;
use std::ops::Deref;


#[derive(Debug)]
pub enum Port<'a, T> {
    Input(&'a InPort<T>),
    Output(&'a OutPort<T>),
}

#[derive(Debug)]
pub struct InPort<T> {
    name: &'static str,
    /// The binding for an input port is the output port to
    /// which it is connected.
    binding: Option<Rc<OutPort<T>>>,
}

impl<T> InPort<T> {
    pub fn new(name: &'static str) -> InPort<T> {
        InPort { name, binding: None }
    }

    pub fn bind(&mut self, binding: &Rc<OutPort<T>>) {
        if let Some(b) = &self.binding {
            panic!("Input port {} already bound to {}", self.name, b.name)
        }
        self.binding = Some(Rc::clone(binding))
    }

    pub fn borrow_or_panic(&self) -> impl Deref<Target = T>  + '_ {
        match &self.binding {
            Some(output) => output.borrow_val(),
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
    pub fn new(name: &'static str, initial_val: T) -> OutPort<T> {
        OutPort { name, cell: RefCell::new(initial_val) }
    }

    pub fn borrow_val(&self) -> impl Deref<Target = T> + '_ {
        self.cell.borrow()
    }

    pub fn set(&self, new_val: T) {
        *self.cell.borrow_mut() = new_val
    }
}
