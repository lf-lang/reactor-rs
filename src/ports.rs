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
#[cfg(not(feature = "no-unsafe"))]
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt::Debug;
#[cfg(feature = "no-unsafe")]
use std::ops::Deref;
use std::ops::{DerefMut, Index, IndexMut};
use std::rc::Rc;
use std::time::Instant;

use atomic_refcell::AtomicRefCell;
use AssemblyErrorImpl::{CannotBind, CyclicDependency};

use crate::assembly::{AssemblyError, AssemblyErrorImpl, PortId, PortKind, TriggerId, TriggerLike};
use crate::{EventTag, ReactionTrigger};

/// A read-only reference to a port.
#[repr(transparent)]
pub struct ReadablePort<'a, T: Sync>(&'a Port<T>);

impl<'a, T: Sync> ReadablePort<'a, T> {
    #[inline(always)]
    pub fn new(port: &'a Port<T>) -> Self {
        Self(port)
    }
}

impl<T: Sync> ReactionTrigger<T> for ReadablePort<'_, T> {
    #[inline]
    fn get_value(&self, _now: &EventTag, _start: &Instant) -> Option<T>
    where
        T: Copy,
    {
        self.0.get()
    }

    #[inline]
    fn use_value_ref<O>(&self, _now: &EventTag, _start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O {
        self.0.use_ref(|opt| action(opt.as_ref()))
    }
}

/// A write-only reference to a port.
pub struct WritablePort<'a, T: Sync>(&'a mut Port<T>);

impl<'a, T: Sync> WritablePort<'a, T> {
    #[inline(always)]
    #[doc(hidden)]
    pub fn new(port: &'a mut Port<T>) -> Self {
        Self(port)
    }

    /// Set the value, see [super::ReactionCtx::set]
    /// Note: we use a closure to process the dependencies to
    /// avoid having to clone the dependency list just to return it.
    pub(crate) fn set_impl(&mut self, v: T) {
        self.0.set_impl(Some(v))
    }

    pub(crate) fn get_id(&self) -> TriggerId {
        self.0.get_id()
    }

    pub(crate) fn kind(&self) -> PortKind {
        self.0.kind
    }
}

/// Internal type, not communicated to reactions.
pub struct PortBank<T: Sync> {
    ports: Vec<Port<T>>,
    id: TriggerId,
}

impl<T: Sync> PortBank<T> {
    pub(crate) fn new(ports: Vec<Port<T>>, id: TriggerId) -> Self {
        Self { ports, id }
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Port<T>> {
        self.ports.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.ports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ports.is_empty()
    }
}

impl<T: Sync> TriggerLike for PortBank<T> {
    fn get_id(&self) -> TriggerId {
        self.id
    }
}

impl<'a, T: Sync> IntoIterator for &'a mut PortBank<T> {
    type Item = &'a mut Port<T>;
    type IntoIter = std::slice::IterMut<'a, Port<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.ports.iter_mut()
    }
}

impl<T: Sync> Index<usize> for PortBank<T> {
    type Output = Port<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.ports[index]
    }
}

impl<T: Sync> IndexMut<usize> for PortBank<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.ports[index]
    }
}

/// A read-only reference to a port bank.
pub struct ReadablePortBank<'a, T: Sync>(&'a PortBank<T>);

impl<'a, T: Sync> ReadablePortBank<'a, T> {
    #[inline(always)]
    #[doc(hidden)]
    pub fn new(port: &'a PortBank<T>) -> Self {
        Self(port)
    }

    /// Returns the length of the bank
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.ports.len()
    }

    /// Returns true if the bank is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.ports.is_empty()
    }

    /// Returns the ith component
    #[inline(always)]
    pub fn get(&self, i: usize) -> ReadablePort<T> {
        ReadablePort(&self.0.ports[i])
    }
}

impl<'a, T: Sync> IntoIterator for ReadablePortBank<'a, T> {
    type Item = ReadablePort<'a, T>;
    type IntoIter = std::iter::Map<std::slice::Iter<'a, Port<T>>, fn(&'a Port<T>) -> ReadablePort<'a, T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.ports.iter().map(ReadablePort)
    }
}

pub struct WritablePortBank<'a, T: Sync>(&'a mut PortBank<T>);

impl<'a, T: Sync> WritablePortBank<'a, T> {
    #[doc(hidden)]
    #[inline(always)]
    pub fn new(port: &'a mut PortBank<T>) -> Self {
        Self(port)
    }

    /// Returns the length of the bank
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.ports.len()
    }

    /// Returns true if the bank is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.ports.is_empty()
    }

    /// Returns the ith component
    #[inline(always)]
    pub fn get(&mut self, i: usize) -> WritablePort<T> {
        WritablePort(&mut self.0.ports[i])
    }
}

impl<'a, T: Sync> IntoIterator for WritablePortBank<'a, T> {
    type Item = WritablePort<'a, T>;
    type IntoIter = std::iter::Map<std::slice::IterMut<'a, Port<T>>, fn(&'a mut Port<T>) -> WritablePort<'a, T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.ports.iter_mut().map(WritablePort)
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
pub struct Port<T: Sync> {
    id: TriggerId,
    kind: PortKind,
    bind_status: BindStatus,
    #[cfg(feature = "no-unsafe")]
    upstream_binding: Rc<AtomicRefCell<Rc<PortCell<T>>>>,
    #[cfg(not(feature = "no-unsafe"))]
    upstream_binding: Rc<UnsafeCell<Rc<PortCell<T>>>>,
    //                              ^^
    // Note that manipulating this Rc is really unsafe and
    // requires care to avoid UB.
    // - Cloning the Rc from different threads concurrently is UB.
    // But in this framework, that Rc is cloned only during the
    // assembly phase, which occurs entirely in the main
    // scheduler thread.
    // - Modifying the contents concurrently is also UB.
    // But, by construction of the dependency graph,
    //   - there is at most one reaction that may set the port,
    // so we never borrow the PortCell mutably twice concurrently.
    //   - that reaction, if any, must be executed BEFORE any
    // reaction that reads the port. That BEFORE is a
    // synchronization barrier in the parallel runtime
    // implementation, so there is no simultaneous mutable and immutable borrow.
    //
    //
}

impl<T: Sync> Port<T> {
    /// Create a new port
    pub(crate) fn new(id: TriggerId, kind: PortKind) -> Self {
        Self {
            id,
            kind,
            bind_status: BindStatus::Free,
            #[cfg(feature = "no-unsafe")]
            upstream_binding: Rc::new(AtomicRefCell::new(Default::default())),
            #[cfg(not(feature = "no-unsafe"))]
            upstream_binding: Rc::new(UnsafeCell::new(Default::default())),
        }
    }

    #[inline]
    pub(crate) fn get(&self) -> Option<T>
    where
        T: Copy,
    {
        self.use_ref(Option::<T>::clone)
    }

    cfg_if! {
        if #[cfg(feature = "no-unsafe")] {
            pub(crate) fn use_ref<R>(&self, f: impl FnOnce(&Option<T>) -> R) -> R {
                use atomic_refcell::AtomicRef;
                let cell_ref: AtomicRef<Rc<PortCell<T>>> = AtomicRefCell::borrow(&self.upstream_binding);
                let binding: &Rc<PortCell<T>> = cell_ref.deref();
                let class_cell: &PortCell<T> = Rc::borrow(binding);
                let cell_borrow: &AtomicRef<Option<T>> = &class_cell.value.borrow();

                f(cell_borrow.deref())
            }

            /// Set the value, see [super::ReactionCtx::set]
            /// Note: we use a closure to process the dependencies to
            /// avoid having to clone the dependency list just to return it.
            pub(crate) fn set_impl(&mut self, new_value: Option<T>) {
                use atomic_refcell::AtomicRef;

                debug_assert_ne!(self.bind_status, BindStatus::Bound, "Cannot set a bound port ({:?})", self.id);

                let cell_ref: AtomicRef<Rc<PortCell<T>>> = AtomicRefCell::borrow(&self.upstream_binding);
                let class_cell: &PortCell<T> = Rc::borrow(cell_ref.deref());

                *class_cell.value.borrow_mut() = new_value;
            }

        } else {
             #[inline]
             pub(crate) fn use_ref<R>(&self, f: impl FnOnce(&Option<T>) -> R) -> R {
                let binding: &UnsafeCell<Rc<PortCell<T>>> = Rc::borrow(&self.upstream_binding);
                let opt: &Option<T> = unsafe {
                    let cell = &*binding.get();
                    &*cell.value.get()
                };
                f(opt)
            }

             #[inline]
             pub(crate) fn set_impl(&mut self, new_value: Option<T>) {
                debug_assert_ne!(self.bind_status, BindStatus::Bound, "Cannot set a bound port");

                let binding: &UnsafeCell<Rc<PortCell<T>>> = Rc::borrow(&self.upstream_binding);

                unsafe {
                    let cell: &Rc<PortCell<T>> = &*binding.get();
                    // note: using write instead of replace would not drop the old value
                    cell.value.get().replace(new_value);
                }
            }
        }
    }

    /// Called at the end of a tag.
    #[inline]
    pub(crate) fn clear_value(&mut self) {
        // If this port is bound, then some other port has
        // a reference to the same cell but is not bound.
        if self.bind_status != BindStatus::Bound {
            self.set_impl(None)
        }
    }

    pub(crate) fn forward_to(&mut self, downstream: &mut Port<T>) -> Result<(), AssemblyError> {
        let mut mut_downstream_cell = {
            cfg_if! {
                if #[cfg(feature = "no-unsafe")] {
                    (&downstream.upstream_binding).borrow_mut()
                } else {
                    unsafe { (&downstream.upstream_binding).get().as_mut().unwrap() }
                }
            }
        };

        if downstream.bind_status == BindStatus::Bound {
            return Err(AssemblyError(CannotBind(self.id, downstream.id)));
        }

        downstream.bind_status = BindStatus::Bound;

        let my_class = {
            cfg_if! {
                if #[cfg(feature = "no-unsafe")] {
                    self.upstream_binding.borrow_mut()
                } else {
                    unsafe { self.upstream_binding.get().as_mut().unwrap() }
                }
            }
        };

        my_class
            .downstreams
            .borrow_mut()
            .insert(downstream.id, Rc::clone(&downstream.upstream_binding));

        let new_binding = Rc::clone(&my_class);

        mut_downstream_cell.check_cycle(&self.id, &downstream.id)?;

        mut_downstream_cell.set_upstream(&my_class);
        *mut_downstream_cell.deref_mut() = new_binding;
        Ok(())
    }
}

impl<T: Sync> TriggerLike for Port<T> {
    fn get_id(&self) -> TriggerId {
        self.id
    }
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

#[cfg(feature = "no-unsafe")]
type DownstreamsSafe<T> = AtomicRefCell<HashMap<PortId, Rc<AtomicRefCell<Rc<PortCell<T>>>>>>;
#[cfg(not(feature = "no-unsafe"))]
type DownstreamsUnsafe<T> = AtomicRefCell<HashMap<PortId, Rc<UnsafeCell<Rc<PortCell<T>>>>>>;

/// This is the internal cell type that is shared by ports.
struct PortCell<T: Sync> {
    /// Cell for the value.
    #[cfg(feature = "no-unsafe")]
    value: AtomicRefCell<Option<T>>,
    #[cfg(not(feature = "no-unsafe"))]
    value: UnsafeCell<Option<T>>,

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
    #[cfg(feature = "no-unsafe")]
    downstreams: DownstreamsSafe<T>,
    #[cfg(not(feature = "no-unsafe"))]
    downstreams: DownstreamsUnsafe<T>,
}

impl<T: Sync> PortCell<T> {
    fn check_cycle(&self, upstream_id: &PortId, downstream_id: &PortId) -> Result<(), AssemblyError> {
        if (&*self.downstreams.borrow()).contains_key(upstream_id) {
            Err(AssemblyError(CyclicDependency(*upstream_id, *downstream_id)))
        } else {
            Ok(())
        }
    }

    /// This updates all downstreams to point to the given equiv class instead of `self`
    fn set_upstream(&self, new_binding: &Rc<PortCell<T>>) {
        for cell_rc in (*self.downstreams.borrow()).values() {
            cfg_if! {
                if #[cfg(feature = "no-unsafe")] {
                    let mut ref_mut = cell_rc.borrow_mut();
                    *ref_mut.deref_mut() = Rc::clone(new_binding);
                } else {
                    unsafe {
                        *cell_rc.get() = Rc::clone(new_binding);
                    }
                }
            }
        }
    }
}

impl<T: Sync> Default for PortCell<T> {
    fn default() -> Self {
        PortCell {
            value: Default::default(),
            downstreams: Default::default(),
        }
    }
}
