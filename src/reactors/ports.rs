use std::any::{Any, TypeId};
use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::collections::hash_set::IntoIter;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::rc::Rc;

use crate::reactors::id::{GlobalId, Identified};
use crate::reactors::ports::PortBinding::{PortBound, Unbound};

/// An equivalence class is a set of ports that are
/// bound together transitively. Then, if anyone is
/// set (there can be only one, that is unbound), then
/// the value must be forwarded to all the others.
///
/// No forwarding actually happens. Ports of the same
/// equivalence class have a reference to the equivalence class,
/// which has a unique cell to store data.
struct PortEquivClass<T> {
    // This the container for the value
    cell: RefCell<T>,
}


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PortKind { Input, Output }

// fixme thread safety
pub struct PortId<T> {
    kind: PortKind,
    global_id: GlobalId,
    _phantom_t: PhantomData<T>,

    /// Ports have a slot in which they accumulate values.
    /// The outer RefCell lets us change the binding internally.
    /// The inner one stores values.
    ///
    /// TODO you may be able to get rid of the outer RefCells by using mut references
    upstream_binding: RefCell<(PortBinding, Rc<PortEquivClass<T>>)>,

    downstream: HashSet<Rc<PortId<T>>>,
}

impl<T> PortId<T> {
    fn kind(&self) -> PortKind {
        self.kind
    }

    pub(in super) fn forward_to(&mut self, downstream: Rc<PortId<T>>) -> Result<(), String> {
        let mut downstream_cell = downstream.upstream_binding.borrow_mut(); // reserve the binding

        match *downstream_cell {
            (PortBinding::PortBound, _) => Err(format!("Port {} is already bound to another port", downstream.global_id)),
            (PortBinding::DependencyBound, _) => Err(format!("Port {} receives values from a reaction", downstream.global_id)),
            (PortBinding::Unbound, _) => {
                let (_, my_class) = &*self.upstream_binding.borrow();

                let new_binding = (PortBinding::PortBound, Rc::clone(&my_class));
                let my_downstream = &mut self.downstream;
                for x in &downstream.downstream {
                    *x.upstream_binding.borrow_mut() = new_binding.clone();
                    my_downstream.insert(x.clone());
                }
                *downstream_cell.deref_mut() = new_binding;
                my_downstream.insert(Rc::clone(&downstream));
                Ok(())
            }
        }
    }


    pub(in super) fn new(kind: PortKind, global_id: GlobalId, default: T) -> Self {
        PortId::<T> {
            kind,
            global_id,
            _phantom_t: PhantomData,
            upstream_binding: RefCell::new((Unbound, Rc::new(PortEquivClass { cell: RefCell::<T>::new(default) }))),
            downstream: Default::default(),
        }
    }
}

impl<T> PartialEq<Self> for PortId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.global_id == other.global_id
    }
}

impl<T> Eq for PortId<T> {}

impl<T> Hash for PortId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.global_id.hash(state)
    }
}

impl<T> Identified for PortId<T> {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}


#[derive(Clone)]
enum PortBinding {
    Unbound,
    PortBound,
    DependencyBound,
}
