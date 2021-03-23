use std::cell::Cell;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

use crate::runtime::Dependencies;

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
///
pub struct Port<T, Kind> {
    cell: Arc<Mutex<PortCell<T>>>,
    _marker: PhantomData<Kind>,
    debug_label: &'static str,
}

impl<T, K> Debug for Port<T, K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug_label.fmt(f)
    }
}

impl<T, K> Port<T, K> {
    // private
    fn new_impl(name: Option<&'static str>) -> Port<T, K> {
        Port {
            cell: Default::default(),
            _marker: Default::default(),
            debug_label: name.unwrap_or("<missing label>"),
        }
    }

    /// Only for glue code during assembly.
    pub fn set_downstream(&mut self, r: Dependencies) {
        let mut upclass = self.cell.lock().unwrap();
        upclass.downstream = Some(r);
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
    // todo these strategies contradict each other,
    //  to support proper cell binding, we need a topo ordering,
    //  and to support dependency merging, we need a reverse topo ordering.
    {

        let mut upclass = up.cell.lock().unwrap();
        let mut downclass = down.cell.lock().unwrap();

        let uc: &mut PortCell<T> = upclass.deref_mut();
        let dc: &mut PortCell<T> = downclass.deref_mut();

        let up_deps = uc.downstream.as_mut().expect("Upstream port cannot be bound");
        // note we take it to mark it as "unbindable"
        // in the future                  vvvvvv
        let mut down_deps = dc.downstream.take().expect("Downstream port is already bound");

        up_deps.append(&mut down_deps);
    }

    // this is the reason we need a topo ordering, see also tests
    down.cell = up.cell.clone();
}


impl<T> InputPort<T> {
    /// Create a new input port
    pub fn new() -> Self {
        Self::new_impl(None)
    }

    pub fn labeled(label: &'static str) -> Self {
        Self::new_impl(Some(label))
    }

    /// Copies the value out, see [super::LogicalCtx::get]
    pub(in crate) fn get(&self) -> Option<T> where T: Copy {
        self.cell.lock().unwrap().cell.get()
    }
}

impl<T> OutputPort<T> {
    /// Create a new input port
    pub fn new() -> Self {
        Self::new_impl(None)
    }

    /// Create a new port with the given label
    pub fn labeled(label: &'static str) -> Self {
        Self::new_impl(Some(label))
    }

    /// Set the value, see [super::LogicalCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    pub(in crate) fn set_impl(&mut self, v: T, process_deps: impl FnOnce(&Dependencies)) {
        let guard = self.cell.lock().unwrap();
        (*guard).cell.set(Some(v));

        process_deps(guard.downstream.as_ref().expect("Port is bound and cannot be set"));
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
    /// Cell for the value
    cell: Cell<Option<T>>,
    /// If None, then this cell is bound. Any attempt to bind it to a new upstream will fail.
    downstream: Option<Dependencies>,
}

impl<T> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            downstream: Some(Dependencies::default()), // note: not None
        }
    }
}
