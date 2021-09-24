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
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::{AssemblyError, GlobalId, PortId, ReactionSet, TriggerId, TriggerLike};

/// A read-only reference to a port.
#[repr(transparent)]
pub struct ReadablePort<'a, T> {
    port: &'a Port<T>,
}

impl<'a, T> ReadablePort<'a, T> {
    #[inline(always)]
    pub fn new(port: &'a Port<T>) -> Self {
        Self { port }
    }

    /// Copies the value out, see [super::ReactionCtx::get]
    #[inline(always)]
    pub(in crate) fn get(&self) -> Option<T> where T: Copy {
        self.port.get()
    }

    /// Copies the value out, see [super::ReactionCtx::use_ref]
    #[inline(always)]
    pub(in crate) fn use_ref<O>(&self, action: impl FnOnce(&T) -> O ) -> Option<O> {
        self.port.use_ref(|opt| opt.as_ref().map(action))
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
pub struct Port<T> {
    id: GlobalId,
    upstream_binding: Rc<RefCell<Binding<T>>>,
}

impl<T> Port<T> {
    /// Create a new port
    pub fn new(id: GlobalId) -> Self {
        Self {
            id,
            upstream_binding: Rc::new(RefCell::new(Binding(BindStatus::Free, Default::default()))),
        }
    }

    #[inline]
    pub(in crate) fn get(&self) -> Option<T> where T: Copy {
        self.use_ref(Option::<T>::clone)
    }

    #[inline]
    pub(in crate) fn use_ref<R>(&self, f: impl FnOnce(&Option<T>) -> R) -> R {
        let cell: &RefCell<Binding<T>> = self.upstream_binding.borrow();
        let cell_ref: Ref<Binding<T>> = RefCell::borrow(cell);
        let binding: &Binding<T> = cell_ref.deref();
        let Binding(_, class) = binding;
        let class_cell: &PortCell<T> = Rc::borrow(class);
        let cell_borrow: &Ref<Option<T>> = &class_cell.cell.borrow();

        f(cell_borrow.deref())
    }

    fn bind_status(&self) -> BindStatus {
        let binding: &RefCell<Binding<T>> = Rc::borrow(&self.upstream_binding);
        let Binding(status, _) = *binding.borrow();
        status
    }

    /// Set the value, see [super::ReactionCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    #[inline]
    pub(in crate) fn set_impl(&mut self, new_value: Option<T>) {
        debug_assert_ne!(self.bind_status(), BindStatus::Bound, "Cannot set a bound port ({})", self.id);

        let cell: &RefCell<Binding<T>> = self.upstream_binding.borrow();
        let cell_ref: Ref<Binding<T>> = RefCell::borrow(cell);
        // let binding: &Binding<T> = cell_ref.deref();

        let Binding(_, class) = cell_ref.deref();

        let class_cell: &PortCell<T> = Rc::borrow(class);

        class_cell.cell.replace(new_value);
    }

    /// Called at the end of a tag.
    #[inline]
    pub(in crate) fn clear_value(&mut self) {
        // If this port is bound, then some other port has
        // a reference to the same cell but is not bound.
        if self.bind_status() != BindStatus::Bound {
            self.set_impl(None)
        }
    }

    /// Only for glue code during assembly.
    pub fn set_downstream(&mut self, r: ReactionSet) {
        let binding = (*self.upstream_binding).borrow();
        let Binding(_, class) = binding.deref();
        *class.triggered_reactions.borrow_mut() = r;
    }

    #[cfg(test)]
    pub(in crate) fn get_downstream_deps(&self) -> ReactionSet {
        let binding = (*self.upstream_binding).borrow();
        let Binding(_, class) = binding.deref();
        let triggered = class.triggered_reactions.borrow();
        triggered.clone()
    }

    fn forward_to(&mut self, downstream: &mut Port<T>) -> Result<(), AssemblyError> {
        let mut mut_downstream_cell = (&downstream.upstream_binding).borrow_mut();
        let Binding(downstream_status, ref downstream_class) = *mut_downstream_cell;

        if downstream_status == BindStatus::Bound {
            return Err(AssemblyError::CannotBind(self.id, downstream.id))
        }
        assert_ne!(downstream_status, BindStatus::Bound, "Downstream port cannot be bound a second time");
        let mut self_cell = self.upstream_binding.borrow_mut();
        let Binding(_, my_class) = self_cell.deref_mut();

        my_class.downstreams.borrow_mut().insert(
            downstream.id.clone(),
            Rc::clone(&downstream.upstream_binding),
        );

        let new_binding = Binding(BindStatus::Bound, Rc::clone(&my_class));

        downstream_class.check_cycle(&self.id, &downstream.id)?;

        downstream_class.set_upstream(my_class);
        *mut_downstream_cell.deref_mut() = new_binding;
        Ok(())
    }
}


impl<T> Debug for Port<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl<T> TriggerLike for Port<T> {
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
pub(in crate) fn bind_ports<T>(up: &mut Port<T>, down: &mut Port<T>) -> Result<(), AssemblyError> {
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

impl Default for BindStatus {
    fn default() -> Self {
        Self::Free
    }
}

#[derive(Default)]
struct Binding<T>(BindStatus, Rc<PortCell<T>>);


/// This is the internal cell type that is shared by ports.
struct PortCell<T> {
    /// Cell for the value.
    cell: RefCell<Option<T>>,
    /// The set of reactions which are scheduled when this cell is set.
    triggered_reactions: RefCell<ReactionSet>,

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
    downstreams: RefCell<HashMap<PortId, Rc<RefCell<Binding<T>>>>>,
}

impl<T> PortCell<T> {
    fn check_cycle(&self, upstream_id: &PortId, downstream_id: &PortId) -> Result<(), AssemblyError> {
        if (&*self.downstreams.borrow()).contains_key(upstream_id) {
            Err(AssemblyError::CyclicDependency(*upstream_id, *downstream_id))
        } else {
            Ok(())
        }
    }

    /// This updates all downstreams to point to the given equiv class instead of `self`
    fn set_upstream(&self, new_binding: &Rc<PortCell<T>>) {
        for (_, cell_rc) in &*self.downstreams.borrow() {
            let cell: &RefCell<Binding<T>> = Rc::borrow(cell_rc);
            let mut ref_mut = cell.borrow_mut();
            *ref_mut.deref_mut() = Binding(ref_mut.0, Rc::clone(new_binding));
        }
    }
}

impl<T> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            cell: Default::default(),
            triggered_reactions: Default::default(),
            downstreams: Default::default(),
        }
    }
}
