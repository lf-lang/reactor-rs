use std::rc::Rc;
use std::cell::Cell;
use crate::runtime::ReactionInvoker;
use std::marker::PhantomData;
use std::ops::Deref;

// clients may only use InputPort and OutputPort
// but there's a single implementation.

// todo maybe a type token to represent bound/unbound state

pub type InputPort<T> = Port<T, Input>;
pub type OutputPort<T> = Port<T, Output>;

struct Input;
struct Output;

#[derive(Clone)]
pub(in super) struct Port<T, Kind> {
    cell: Rc<PortCell<T>>,
    _marker: PhantomData<Kind>,
}

impl<T, K> Port<T, K> {
    // private
    fn new_impl() -> Port<T, K> {
        Port {
            cell: Rc::new(PortCell::new()),
            _marker: PhantomData,
        }
    }
}

impl<T> InputPort<T> {
    pub fn new() -> Self {
        Self::new_impl()
    }

    pub(in super) fn get(&self) -> Option<T> {
        Rc::deref(&self.cell).cell.get()
    }
}

impl<T> OutputPort<T> {
    pub fn new() -> Self {
        Self::new_impl()
    }

    pub(in super) fn set(&mut self, v: T) {
        Rc::deref(&self.cell).cell.set(Some(v));
    }
}


struct PortCell<T> {
    cell: Cell<Option<T>>,
    downstream: Vec<Rc<ReactionInvoker>>,
}

impl<T> PortCell<T> {
    fn new() -> PortCell<T> {
        PortCell {
            cell: Cell::new(None),
            downstream: Vec::new(),
        }
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
pub fn bind<T, U, D>(up: &mut Port<T, U>, mut down: &mut Port<T, D>) {
    up.cell.downstream.append(&mut down.cell.downstream);
    down.cell = Rc::clone(&up.cell);
}

