use std::cell::{RefCell, Ref};
use std::marker::PhantomData;
use std::rc::Rc;

use crate::reactors::id::{GlobalId, Identified};
use std::ops::DerefMut;
use crate::reactors::ports::PortBinding::{PortBound, Unbound};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PortKind { Input, Output }

pub struct PortId<T> {
    kind: PortKind,
    global_id: GlobalId,
    _phantom_t: PhantomData<T>,

    /// Ports have a slot in which they accumulate values.
    /// The outer RefCell lets us change the binding internally.
    /// The inner one stores values.
    ///
    /// fixme this doesn't work transitively!
    binding: RefCell<(PortBinding, Rc<RefCell<T>>)>,
}

impl<T> PortId<T> {
    fn kind(&self) -> PortKind {
        self.kind
    }

    pub(in super) fn forward_to(&self, downstream: &PortId<T>) -> Result<(), String> {
        let mut cell = downstream.binding.borrow_mut(); // reserve the binding

        match *cell {
            (PortBinding::PortBound, _) => Err(format!("Port {} is already bound to another port", downstream.global_id)),
            (PortBinding::DependencyBound, _) => Err(format!("Port {} receives values from a reaction", downstream.global_id)),
            (PortBinding::Unbound, _) => {
                let (_, my_cell) = &*self.binding.borrow();

                *cell.deref_mut() = (PortBinding::PortBound, Rc::clone(&my_cell));
                Ok(())
            }
        }
    }


    pub(in super) fn new(kind: PortKind, global_id: GlobalId, default: T) -> Self {
        PortId::<T> {
            kind,
            global_id,
            _phantom_t: PhantomData,
            binding: RefCell::new((Unbound, Rc::new(RefCell::<T>::new(default)))),
        }
    }
}

impl<T> Identified for PortId<T> {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}


enum PortBinding {
    Unbound,
    PortBound,
    DependencyBound,
}
