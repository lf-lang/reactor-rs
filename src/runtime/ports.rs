use std::cell::Cell;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

use crate::runtime::ToposortedReactions;

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
pub struct Port<T, Kind, Deps = ToposortedReactions> {
    cell: Arc<Mutex<PortCell<T, Deps>>>,
    _marker: PhantomData<Kind>,
    debug_label: &'static str,
    status: BindStatus,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum BindStatus {
    Bindable,
    Bound,
}

impl<T, K> Debug for Port<T, K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug_label.fmt(f)
    }
}

impl<T, K, Deps> Port<T, K, Deps> {
    // private
    fn new_impl(name: Option<&'static str>) -> Port<T, K, Deps> where Deps: Default {
        Port {
            cell: Default::default(),
            _marker: Default::default(),
            debug_label: name.unwrap_or("<missing label>"),
            status: BindStatus::Bindable,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(label: &'static str) -> Port<T, K, Deps> where Deps: Default {
        Self::new_impl(Some(label))
    }

    #[cfg(test)]
    pub(crate) fn get_downstream_deps(&self) -> Option<Deps> where Deps: Clone {
        let class = self.cell.lock().unwrap();
        class.downstream.clone()
    }

    /// Only for glue code during assembly.
    pub fn set_downstream(&mut self, r: Deps) {
        let mut class = self.cell.lock().unwrap();
        class.downstream = Some(r);
    }
}

/// Make the downstream port accept values from the upstream port
/// For this to work this function must be called in topological
/// order between bound ports
/// Eg
/// ```
/// a.out -> b.in -> b.out -> c.in;
///                  b.out -> d.in;
/// ```
///
/// Must be translated as
///
/// ```
/// bind(a.out, b.in);
/// bind(b.in, b.out);
/// bind(b.out, d.in);
/// bind(b.out, c.in);
/// ```
///
/// Also the edges must be that of a transitive reduction of
/// the graph, as the down port is destroyed.
///
/// ### Panics
///
/// If the downstream port was already bound to some other port.
pub fn bind_ports<T, U, D, Deps>(up: &mut Port<T, U, Deps>, mut down: &mut Port<T, D, Deps>) where Deps: Absorbing {
    assert_eq!(down.status, BindStatus::Bindable, "Downstream port is already bound");

    {
        let mut upclass = up.cell.lock().unwrap();
        let mut downclass = down.cell.lock().unwrap();

        let uc: &mut PortCell<T, Deps> = upclass.deref_mut();
        let dc: &mut PortCell<T, Deps> = downclass.deref_mut();

        let up_deps = uc.downstream.as_mut().expect("Upstream port cannot be bound");
        // note we take it to mark it as "unbindable"
        // in the future                  vvvvvv
        let mut down_deps = dc.downstream.take().expect("Downstream port is already bound");

        up_deps.absorb(&mut down_deps);
    }

    // this is the reason we need a topo ordering, see also tests
    down.cell = up.cell.clone();
    down.status = BindStatus::Bound;
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
    pub(in crate) fn set_impl(&mut self, v: T, process_deps: impl FnOnce(&ToposortedReactions)) {
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

struct PortCell<T, Deps = ToposortedReactions> {
    /// Cell for the value
    cell: Cell<Option<T>>,
    /// If None, then this cell is bound. Any attempt to bind it to a new upstream will fail.
    downstream: Option<Deps>,
}

impl<T, Deps> Default for PortCell<T, Deps> where Deps: Default {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            downstream: Some(Deps::default()), // note: not None
        }
    }
}

/// This trait is only used to be able to fake a reaction type
/// in tests
#[doc(hidden)]
pub trait Absorbing {
    /// Merge the parameter into this object.
    /// This function is idempotent.
    fn absorb(&mut self, other: &mut Self);
}

impl<T> Absorbing for Vec<T> {
    fn absorb(&mut self, other: &mut Self) {
        /// TODO when absorbing reactions we need to preserve the topological sort
        ///  I think it would suffice to
        self.append(other)
    }
}
