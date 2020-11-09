use std::rc::Rc;
use std::cell::{Cell, Ref};
use crate::runtime::{ReactionInvoker, Dependencies};
use std::marker::PhantomData;
use std::ops::Deref;
use std::cell::RefCell;

// clients may only use InputPort and OutputPort
// but there's a single implementation.

// todo maybe a type token to represent bound/unbound state

pub type InputPort<T> = Port<T, Input>;
pub type OutputPort<T> = Port<T, Output>;

pub struct Input;
pub struct Output;

#[derive(Clone)]
pub struct Port<T, Kind> {
    cell: Rc<RefCell<PortCell<T>>>,
    _marker: PhantomData<Kind>,
}

impl<T, K> Port<T, K> {
    // private
    fn new_impl() -> Port<T, K> {
        Port {
            cell: Rc::new(RefCell::new(PortCell::new())),
            _marker: PhantomData,
        }
    }

    pub fn set_downstream(&mut self, mut r: Dependencies) {
        let mut upclass = self.cell.borrow_mut();
        upclass.downstream = r;
    }
}

/// Make the downstream port accept values from the upstream port
/// For this to work this function must be called in reverse topological
/// order between bound ports
/// Eg
/// a.out -> b.in
/// b.in -> b.out
/// b.out -> c.in
/// b.out -> d.in
///
/// Must be translated as
///
/// bind(b.out, d.in)
/// bind(b.out, c.in)
/// bind(b.in, b.out)
/// bind(a.out, b.in)
///
/// Also the edges must be that of a transitive reduction of
/// the graph, as the down port is destroyed.
pub fn bind_ports<T, U, D>(up: &mut Port<T, U>, mut down: &mut Port<T, D>) {
    {
        let mut upclass = up.cell.borrow_mut();
        let mut downclass = down.cell.borrow_mut();
        (&mut upclass.downstream).append(&mut downclass.downstream);
    }
    down.cell = Rc::clone(&up.cell);
}


impl<T> InputPort<T> {
    pub fn new() -> Self {
        Self::new_impl()
    }

    pub(in super) fn get(&self) -> Option<T> where T : Copy {
        self.cell.borrow().cell.get()
    }
}

impl<T> OutputPort<T> {
    pub fn new() -> Self {
        Self::new_impl()
    }

    pub(in super) fn set(&mut self, v: T) -> Ref<Dependencies> {
        (*self.cell.borrow_mut()).cell.set(Some(v));
        Ref::map(self.cell.borrow(),
                 |t| &t.downstream)
    }
}


struct PortCell<T> {
    cell: Cell<Option<T>>,
    downstream: Dependencies,
}

impl<T> PortCell<T> {
    fn new() -> PortCell<T> {
        PortCell {
            cell: Cell::new(None),
            downstream: Default::default(),
        }
    }
}
