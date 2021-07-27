/*
 * Copyright (c) 2021, TU Dresden.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
 * THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
 * PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
 * THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use crate::ReactionSet;

/// A read-only reference to a port.
#[repr(transparent)]
pub struct ReadablePort<'a, T> {
    port: &'a Port<T>,
}

impl<'a, T> ReadablePort<'a, T> {
    #[inline]
   pub fn new(port: &'a Port<T>) -> Self {
        Self { port }
    }

    /// Copies the value out, see [super::ReactionCtx::get]
    #[inline]
    pub(in crate) fn get(&self) -> Option<T> where T: Copy {
        self.port.get()
    }

    /// Copies the value out, see [super::ReactionCtx::use_ref]
    #[inline]
    pub(in crate) fn use_ref<O>(&self, action: impl FnOnce(&T) -> O ) -> Option<O> {
        let lock = self.port.cell.lock().unwrap();
        let deref = lock.cell.borrow();
        let opt: &Option<T> = &*deref;
        opt.as_ref().map(action)
    }
}

/// A write-only reference to a port.
pub struct WritablePort<'a, T> {
    port: &'a mut Port<T>,
}

impl<'a, T> WritablePort<'a, T> {
   pub fn new(port: &'a mut Port<T>) -> Self {
        Self { port }
    }

    /// Set the value, see [super::ReactionCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    pub(in crate) fn set_impl(&mut self, v: T, process_deps: impl FnOnce(&ReactionSet)) {
        self.port.set_impl(v, process_deps)
    }

}


/// Represents a port, which carries values of type `T`.
/// Ports reify the data inputs and outputs of a reactor.
///
/// They may be bound to another port, in which case the
/// upstream port forwards all values to the output port
/// (logically instantaneously). A port may have only one
/// upstream binding.
///
/// Output ports may also be explicitly [set](super::ReactionCtx::set)
/// within a reaction, in which case they may not have an
/// upstream port binding.
///
/// Those structural constraints are trusted to have been
/// verified by the code generator. If necessary we may be
/// able to add conditional compilation flags that enable
/// runtime checks.
///
///
pub struct Port<T> {
    cell: Arc<Mutex<PortCell<T>>>,
    debug_label: &'static str,
    status: BindStatus,
}

impl<T> Port<T> {
    /// Create a new port
    pub fn new() -> Self {
        Self::new_impl(None)
    }

    /// Create a new port with the given label
    pub fn labeled(label: &'static str) -> Self {
        Self::new_impl(Some(label))
    }

    // private
    fn new_impl(name: Option<&'static str>) -> Port<T> {
        Port {
            cell: Default::default(),
            debug_label: name.unwrap_or("<missing label>"),
            status: BindStatus::Free,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(label: &'static str) -> Port<T> {
        Self::new_impl(Some(label))
    }

    pub(in crate) fn get(&self) -> Option<T> where T: Copy {
        self.cell.lock().unwrap().cell.borrow().clone()
    }

    /// Set the value, see [super::ReactionCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    pub(in crate) fn set_impl(&mut self, v: T, process_deps: impl FnOnce(&ReactionSet)) {
        assert_ne!(self.status, BindStatus::Bound, "Bound port cannot be bound");

        let guard = self.cell.lock().unwrap();
        *(*guard).cell.borrow_mut() = Some(v);

        process_deps(&guard.downstream);
    }

    /// Only for glue code during assembly.
    pub fn set_downstream(&mut self, r: ReactionSet) {
        let mut class = self.cell.lock().unwrap();
        class.downstream = r;
    }

    #[cfg(test)]
    pub(in crate) fn get_downstream_deps(&self) -> ReactionSet {
        let class = self.cell.lock().unwrap();
        class.downstream.clone()
    }
}


impl<T> Debug for Port<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.debug_label.fmt(f)
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
pub fn bind_ports<T>(up: &mut Port<T>, mut down: &mut Port<T>) {
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


/// This is the internal cell type that is shared by ports.
struct PortCell<T> {
    /// Cell for the value.
    cell: RefCell<Option<T>>,
    /// The set of reactions which are scheduled when this cell is set.
    downstream: ReactionSet,
}

impl<T> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            downstream: Default::default(),
        }
    }
}
