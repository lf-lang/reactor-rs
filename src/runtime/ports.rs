
use std::cell::{Cell};
use crate::runtime::{Dependencies};
use std::marker::PhantomData;


use std::sync::{Mutex, Arc};

// clients may only use InputPort and OutputPort
// but there's a single implementation.

// todo maybe a type token to represent bound/unbound state

/// An input port.
pub type InputPort<T> = Port<T, Input>;
/// An output port.
pub type OutputPort<T> = Port<T, Output>;

#[doc(hidden)]
pub struct Input;
#[doc(hidden)]
pub struct Output;

/// Represents a port, which carries values of type `T`.
/// Ports reify the data inputs and outputs of a reactor.
///
/// They may be bound to another port, in which case the
/// upstream port forwards all values to the output port
/// (logically instantaneously). A port may have only one
/// upstream binding.
///
/// Output ports may also be explicitly [set](super::LogicalCtx::set)
/// within a reaction, in which case they may not have an
/// upstream port binding.
///
/// Those structural constraints are trusted to have been
/// verified by the code generator. If necessary we may be
/// able to add conditional compilation flags that enable
/// runtime checks.
///
#[derive(Clone)]
pub struct Port<T, Kind> {
    cell: Arc<Mutex<PortCell<T>>>,
    _marker: PhantomData<Kind>,
}

impl<T, K> Port<T, K> {
    // private
    fn new_impl() -> Port<T, K> {
        Port {
            cell: Default::default(),
            _marker: Default::default(),
        }
    }

    /// Only for glue code during assembly.
    pub fn set_downstream(&mut self, r: Dependencies) {
        let mut upclass = self.cell.lock().unwrap();
        upclass.downstream = r;
    }
}

/// Make the downstream port accept values from the upstream port
/// For this to work this function must be called in reverse topological
/// order between bound ports
/// Eg
/// ```
/// a.out -> b.in;
/// b.in -> b.out;
/// b.out -> c.in;
/// b.out -> d.in;
/// ```
///
/// Must be translated as
///
/// ```
/// bind(b.out, d.in);
/// bind(b.out, c.in);
/// bind(b.in, b.out);
/// bind(a.out, b.in);
/// ```
///
/// Also the edges must be that of a transitive reduction of
/// the graph, as the down port is destroyed.
pub fn bind_ports<T, U, D>(up: &mut Port<T, U>, mut down: &mut Port<T, D>) {
    {
        let mut upclass = up.cell.lock().unwrap();
        let mut downclass = down.cell.lock().unwrap();
        (&mut upclass.downstream).append(&mut downclass.downstream);
    }
    down.cell = up.cell.clone();
}


impl<T> InputPort<T> {
    /// Create a new input port
    pub fn new() -> Self {
        Self::new_impl()
    }

    /// Copies the value out, see [super::LogicalCtx::get]
    pub(in super) fn get(&self) -> Option<T> where T: Copy {
        self.cell.lock().unwrap().cell.get()
    }
}

impl<T> OutputPort<T> {
    /// Create a new output port
    pub fn new() -> Self {
        Self::new_impl()
    }

    /// Set the value, see [super::LogicalCtx::set]
    pub(in super) fn set(&mut self, v: T) -> Dependencies {
        let guard = self.cell.lock().unwrap();
        (*guard).cell.set(Some(v));

        guard.downstream.clone()
    }
}

impl<T> Default for InputPort<T> {
    fn default() -> Self {
        Self::new()
    }
}


impl<T> Default for OutputPort<T> {
    fn default() -> Self {
        Self::new()
    }
}


struct PortCell<T> {
    cell: Cell<Option<T>>,
    downstream: Dependencies,
}

impl<T> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            downstream: Default::default(),
        }
    }
}
