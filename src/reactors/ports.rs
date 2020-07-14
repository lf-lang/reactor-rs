use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::collections::hash_set::IntoIter;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{DerefMut, Deref};
use std::rc::Rc;

use crate::reactors::id::{GlobalId, Identified};
use crate::reactors::ports::BindStatus::{PortBound, Unbound};
use std::collections::hash_map::RandomState;


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PortKind { Input, Output }


// fixme thread safety
pub struct PortId<T> {
    kind: PortKind,
    global_id: GlobalId,
    _phantom_t: PhantomData<T>,

    upstream_binding: Rc<RefCell<Binding<T>>>,
}

impl<T> PortId<T> {
    pub fn kind(&self) -> PortKind {
        self.kind
    }

    pub(in crate) fn get(&self) -> T where T: Copy {
        let cell: &RefCell<Binding<T>> = self.upstream_binding.borrow();
        let cell_ref: Ref<Binding<T>> = RefCell::borrow(cell);
        let binding: &Binding<T> = cell_ref.deref();

        let (_, class) = binding;

        let class_cell: &PortEquivClass<T> = Rc::borrow(class);

        // Here's it's copied
        let value = *(&class_cell.cell.borrow()).deref();

        value
    }

    pub(in crate) fn set(&self, new_value: T) {
        let cell: &RefCell<Binding<T>> = self.upstream_binding.borrow();
        let cell_ref: Ref<Binding<T>> = RefCell::borrow(cell);
        let binding: &Binding<T> = cell_ref.deref();

        let (_, class) = binding;

        let class_cell: &PortEquivClass<T> = Rc::borrow(class);

        *class_cell.cell.borrow_mut().deref_mut() = new_value;
    }

    pub(in super) fn forward_to(&self, downstream: &PortId<T>) -> Result<(), String> {
        // let binding_borrow: &RefCell<Binding<T>> = Rc::borrow(&downstream.upstream_binding);

        let mut mut_downstream_cell = (&downstream.upstream_binding).borrow_mut();
        let (downstream_status, ref downstream_class) = *mut_downstream_cell;

        match downstream_status {
            BindStatus::PortBound => Err(format!("Port {} is already bound to another port", downstream.global_id)),
            BindStatus::DependencyBound => Err(format!("Port {} receives values from a reaction", downstream.global_id)),
            BindStatus::Unbound => {
                let mut self_cell = self.upstream_binding.borrow_mut();
                let (_, my_class) = self_cell.deref_mut();

                my_class.downstreams.borrow_mut().insert(HashableBinding::new(downstream));

                let new_binding = (BindStatus::PortBound, Rc::clone(&my_class));

                downstream_class.set_upstream(my_class);
                *mut_downstream_cell.deref_mut() = new_binding;

                Ok(())
            }
        }
    }


    pub(in super) fn new(kind: PortKind, global_id: GlobalId) -> Self where T: IgnoredDefault {
        PortId::<T> {
            kind,
            global_id,
            _phantom_t: PhantomData,
            upstream_binding: Rc::new(RefCell::new((Unbound, Rc::new(PortEquivClass::<T>::new(IgnoredDefault::ignored_default()))))),
        }
    }
}


impl<T> Identified for PortId<T> {
    fn global_id(&self) -> &GlobalId {
        &self.global_id
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
enum BindStatus {
    Unbound,
    PortBound,
    DependencyBound,
}


type Binding<T> = (BindStatus, Rc<PortEquivClass<T>>);

struct HashableBinding<T> {
    cell: Rc<RefCell<Binding<T>>>,
    key: GlobalId,
}

impl<T> HashableBinding<T> {
    fn new(port: &PortId<T>) -> HashableBinding<T> {
        HashableBinding {
            key: port.global_id.clone(),
            cell: Rc::clone(&port.upstream_binding),
        }
    }
}

impl<T> PartialEq for HashableBinding<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T> Eq for HashableBinding<T> {}

impl<T> Hash for HashableBinding<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}


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

    downstreams: RefCell<HashSet<HashableBinding<T>>>,
}

impl<T> PortEquivClass<T> {
    fn new(initial: T) -> Self {
        PortEquivClass {
            cell: RefCell::new(initial),
            downstreams: Default::default(),
        }
    }

    /// This updates all downstreams to point to the given equiv class instead of `self`
    fn set_upstream(&self, new_binding: &Rc<PortEquivClass<T>>) {
        for hashed in &*self.downstreams.borrow() {
            let cell: &RefCell<Binding<T>> = Rc::borrow(&hashed.cell);
            let mut ref_mut = cell.borrow_mut();
            let b: Binding<T> = (ref_mut.0, Rc::clone(new_binding));
            *ref_mut.deref_mut() = b;
        }
    }
}

/// Ports need an initial value, which is not observed by anyone.
pub trait IgnoredDefault {
    fn ignored_default() -> Self;
}

impl<T> IgnoredDefault for T where T: Default {
    fn ignored_default() -> Self {
        Default::default()
    }
}
