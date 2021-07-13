use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::ToposortedReactions;

// clients may only use InputPort and OutputPort
// but there's a single implementation.

/// An input port. fixme input ports cannot be written to, which is necessary when writing to inner reactor.
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
    debug_label: &'static str,
    status: BindStatus,
    _marker: PhantomData<Kind>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum BindStatus {
    /// A bindable port is also writable explicitly (with set)
    Free,
    /// Means that this port is the upstream of some bound port.
    Upstream,
    /// A bound port cannot be written to explicitly. For a
    /// set of ports bound together, there is a single cell,
    /// and a single writable port through which values are
    /// communicated ([Self::Upstream]).
    Bound,
}

impl<T, K> Port<T, K> {
    // private
    fn new_impl(name: Option<&'static str>) -> Port<T, K> {
        Port {
            cell: Default::default(),
            _marker: Default::default(),
            debug_label: name.unwrap_or("<missing label>"),
            status: BindStatus::Free,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(label: &'static str) -> Port<T, K> {
        Self::new_impl(Some(label))
    }

    /// Only for glue code during assembly.
    pub fn set_downstream(&mut self, r: ToposortedReactions) {
        let mut class = self.cell.lock().unwrap();
        class.downstream = r;
    }
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
        self.cell.lock().unwrap().cell.borrow().clone()
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
    pub(in crate) fn set_impl(&mut self, v: T, process_deps: impl FnOnce(&ToposortedReactions)) {
        assert_ne!(self.status, BindStatus::Bound, "Bound port cannot be bound");

        let guard = self.cell.lock().unwrap();
        *(*guard).cell.borrow_mut() = Some(v);

        process_deps(&guard.downstream);
    }

    /// Only output ports can be explicitly set, so only them
    /// produce events and hence need access to the set of their
    /// dependencies. This is why we only test those.
    #[cfg(test)]
    pub(crate) fn get_downstream_deps(&self) -> ToposortedReactions {
        let class = self.cell.lock().unwrap();
        class.downstream.clone()
    }
}

impl<T, K> Debug for Port<T, K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug_label.fmt(f)
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




/// Make the downstream port accept values from the upstream port
/// For this to work this function must be called in topological
/// order between bound ports
/// Eg
/// ```text
/// a_out -> b_in -> b_out -> c_in;
///                  b_out -> d_in;
/// ```
///
/// Must be translated as
///
/// ```
///
/// # fn main() {
/// # use reactor_rt::{OutputPort, InputPort, bind_ports};
/// # let mut a_out = OutputPort::<i32>::new();
/// # let mut b_out = OutputPort::<i32>::new();
/// # let mut b_in = InputPort::<i32>::new();
/// # let mut c_in = InputPort::<i32>::new();
/// # let mut d_in = InputPort::<i32>::new();
/// bind_ports(&mut a_out, &mut b_in);
/// bind_ports(&mut b_in, &mut b_out);
/// bind_ports(&mut b_out, &mut d_in);
/// bind_ports(&mut b_out, &mut c_in);
/// # }
/// ```
///
/// Also the edges must be that of a transitive reduction of
/// the graph, as the down port is destroyed. These responsibilities
/// are shifted onto the code generator.
///
/// ### Panics
///
/// If the downstream port was already bound to some other port.
///
pub fn bind_ports<T, U, D>(up: &mut Port<T, U>, mut down: &mut Port<T, D>) {
    assert_ne!(down.status, BindStatus::Bound, "Downstream port cannot be bound a second time");
    // in a topo order the downstream is always free
    assert_ne!(down.status, BindStatus::Upstream, "Ports are being bound in a non topological order");

    {
        let mut upclass = up.cell.lock().unwrap();
        let mut downclass = down.cell.lock().unwrap();

        // todo we need to make sure that this merge preserves the toposort, eg removes duplicates
        (&mut upclass.downstream).append(&mut downclass.downstream);
    }

    // this is the reason we need a topo ordering, see also tests
    down.cell = up.cell.clone();
    down.status = BindStatus::Bound;
    up.status = BindStatus::Upstream;
}


/// This is the internal cell type that is shared by ports.
struct PortCell<T> {
    /// Cell for the value.
    cell: RefCell<Option<T>>,
    /// The set of reactions which are scheduled when this cell is set.
    downstream: ToposortedReactions,
}

impl<T> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            downstream: Default::default(),
        }
    }
}
