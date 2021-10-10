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

use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Instant;

use atomic_refcell::{AtomicRef, AtomicRefCell};

use crate::{AssemblyError, EventTag, GlobalId, PortId, ReactionTrigger, TriggerId, TriggerLike};

/// A read-only reference to a port.
#[repr(transparent)]
pub struct ReadablePort<'a, T: Send>(&'a Port<T>);

impl<'a, T: Send> ReadablePort<'a, T> {
    #[inline(always)]
    pub fn new(port: &'a Port<T>) -> Self {
        Self(port)
    }
}

impl<T: Send> ReactionTrigger<T> for ReadablePort<'_, T> {
    #[inline]
    fn get_value(&self, _now: &EventTag, _start: &Instant) -> Option<T> where T: Copy {
        self.0.get()
    }

    #[inline]
    fn use_value_ref<O>(&self, _now: &EventTag, _start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O {
        self.0.use_ref(|opt| action(opt.as_ref()))
    }
}

/// A write-only reference to a port.
pub struct WritablePort<'a, T: Send> {
    port: &'a mut Port<T>,
}

impl<'a, T: Send> WritablePort<'a, T> {
    pub fn new(port: &'a mut Port<T>) -> Self {
        Self { port }
    }

    /// Set the value, see [super::ReactionCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    pub(in crate) fn set_impl(&mut self, v: T) {
        self.port.set_impl(Some(v))
    }

    pub(in crate) fn get_id(&self) -> TriggerId {
        self.port.get_id()
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
pub struct Port<T: Send> {
    id: GlobalId,
    bind_status: BindStatus,
    upstream_binding: Arc<AtomicRefCell<Arc<PortCell<T>>>>,
}

impl<T: Send> Port<T> {
    /// Create a new port
    pub fn new(id: GlobalId) -> Self {
        Self {
            id,
            bind_status: BindStatus::Free,
            upstream_binding: Arc::new(AtomicRefCell::new(Default::default())),
        }
    }

    #[inline]
    pub(in crate) fn get(&self) -> Option<T> where T: Copy {
        self.use_ref(Option::<T>::clone)
    }

    #[inline]
    pub(in crate) fn use_ref<R>(&self, f: impl FnOnce(&Option<T>) -> R) -> R {
        let cell_ref: AtomicRef<Arc<PortCell<T>>> = AtomicRefCell::borrow(&self.upstream_binding);
        let binding: &Arc<PortCell<T>> = cell_ref.deref();
        let class_cell: &PortCell<T> = Arc::borrow(binding);
        let cell_borrow: &AtomicRef<Option<T>> = &class_cell.cell.borrow();

        f(cell_borrow.deref())
    }

    /// Set the value, see [super::ReactionCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    #[inline]
    pub(in crate) fn set_impl(&mut self, new_value: Option<T>) {
        debug_assert_ne!(self.bind_status, BindStatus::Bound, "Cannot set a bound port ({})", self.id);

        let cell_ref: AtomicRef<Arc<PortCell<T>>> = AtomicRefCell::borrow(&self.upstream_binding);
        let class_cell: &PortCell<T> = Arc::borrow(cell_ref.deref());

        *class_cell.cell.borrow_mut() = new_value;
    }

    /// Called at the end of a tag.
    #[inline]
    pub(in crate) fn clear_value(&mut self) {
        // If this port is bound, then some other port has
        // a reference to the same cell but is not bound.
        if self.bind_status != BindStatus::Bound {
            self.set_impl(None)
        }
    }

    fn forward_to(&mut self, downstream: &mut Port<T>) -> Result<(), AssemblyError> {
        let mut mut_downstream_cell = (&downstream.upstream_binding).borrow_mut();

        if downstream.bind_status == BindStatus::Bound {
            return Err(AssemblyError::CannotBind(self.id, downstream.id))
        }

        downstream.bind_status = BindStatus::Bound;

        let my_class = self.upstream_binding.borrow_mut();

        my_class.downstreams.borrow_mut().insert(
            downstream.id.clone(),
            Arc::clone(&downstream.upstream_binding),
        );

        let new_binding = Arc::clone(&my_class);

        mut_downstream_cell.check_cycle(&self.id, &downstream.id)?;

        mut_downstream_cell.set_upstream(&my_class);
        *mut_downstream_cell.deref_mut() = new_binding;
        Ok(())
    }
}


impl<T: Send> Debug for Port<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl<T: Send> TriggerLike for Port<T> {
    fn get_id(&self) -> TriggerId {
        TriggerId(self.id)
    }
}


/// Make the downstream port accept values from the upstream port.
///
/// ### Panics
///
/// If the downstream port was already bound to some other port.
///
pub(in crate) fn bind_ports<T: Send>(up: &mut Port<T>, down: &mut Port<T>) -> Result<(), AssemblyError> {
    up.forward_to(down)
}


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum BindStatus {
    /// A bindable port is also writable explicitly (with set)
    Free,

    // Means that this port is the upstream of some bound port.
    // Upstream,

    /// A bound port cannot be written to explicitly. For a
    /// set of ports bound together, there is a single cell,
    /// and a single writable port through which values are
    /// communicated ([Self::Upstream]).
    Bound,
}


/// This is the internal cell type that is shared by ports.
struct PortCell<T: Send> {
    /// Cell for the value.
    cell: AtomicRefCell<Option<T>>,

    /// This is the set of ports that are "forwarded to".
    /// When you bind 2 ports A -> B, then the binding of B
    /// is updated to point to the equiv class of A. The downstream
    /// field of that equiv class is updated to contain B.
    ///
    /// Why?
    /// When you have bound eg A -> B and *then* bind U -> A,
    /// then both the equiv class of A and B (the downstream of A)
    /// need to be updated to point to the equiv class of U
    ///
    /// Coincidentally, this means we can track transitive
    /// cyclic port dependencies:
    /// - say you have bound A -> B, then B -> C
    /// - so all three refer to the equiv class of A, whose downstream is now {B, C}
    /// - if you then try binding C -> A, then we can know
    /// that C is in the downstream of A, indicating that there is a cycle.
    downstreams: AtomicRefCell<HashMap<PortId, Arc<AtomicRefCell<Arc<PortCell<T>>>>>>,
}

impl<T: Send> PortCell<T> {
    fn check_cycle(&self, upstream_id: &PortId, downstream_id: &PortId) -> Result<(), AssemblyError> {
        if (&*self.downstreams.borrow()).contains_key(upstream_id) {
            Err(AssemblyError::CyclicDependency(*upstream_id, *downstream_id))
        } else {
            Ok(())
        }
    }

    /// This updates all downstreams to point to the given equiv class instead of `self`
    fn set_upstream(&self, new_binding: &Arc<PortCell<T>>) {
        for (_, cell_rc) in &*self.downstreams.borrow() {
            let mut ref_mut = cell_rc.borrow_mut();
            *ref_mut.deref_mut() = Arc::clone(new_binding);
        }
    }
}

impl<T: Send> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            downstreams: Default::default(),
        }
    }
}
