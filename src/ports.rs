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
pub struct Port<T: Sync> {
    id: TriggerId,
    kind: PortKind,
    bind_status: BindStatus,
    upstream_binding: Rc<UncheckedCell<Rc<PortCell<T>>>>,
    //                                 ^^
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

    pub(crate) fn is_present_now(&self) -> bool {
        self.use_ref(|opt| opt.is_some())
    }

    pub(crate) fn get_kind(&self) -> PortKind {
        self.kind
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

             /// Returns a reference to the value. It is not possible to
             /// implement this in safe code without reimplementing the same
             /// kind of logic as AtomicRef, because the ref has to hold
             /// two borrows at the same time.
             pub(crate) fn get_ref(&self) -> Option<&T> {
                 let binding: &UnsafeCell<Rc<PortCell<T>>> = Rc::borrow(&self.upstream_binding);
                 unsafe {
                     let cell = &*binding.get();
                     let opt = &*cell.value.get();
                    opt.as_ref()
                 }
             }

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
                    (downstream.upstream_binding).borrow_mut()
                } else {
                    unsafe { downstream.upstream_binding.get().as_mut().unwrap() }
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

impl<T: Sync> ReactionTrigger<T> for Port<T> {
    #[inline]
    fn is_present(&self, _now: &EventTag, _start: &Instant) -> bool {
        self.is_present_now()
    }

    #[inline]
    fn get_value(&self, _now: &EventTag, _start: &Instant) -> Option<T>
    where
        T: Copy,
    {
        self.get()
    }

    #[inline]
    fn use_value_ref<O>(&self, _now: &EventTag, _start: &Instant, action: impl FnOnce(Option<&T>) -> O) -> O {
        self.use_ref(|opt| action(opt.as_ref()))
    }
}

#[cfg(not(feature = "no-unsafe"))]
impl<T: Sync> crate::triggers::ReactionTriggerWithRefAccess<T> for Port<T> {
    fn get_value_ref(&self, _now: &EventTag, _start: &Instant) -> Option<&T> {
        self.get_ref()
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

cfg_if! {
    if #[cfg(feature = "no-unsafe")] {
        type Downstreams<T> = AtomicRefCell<HashMap<PortId, Rc<AtomicRefCell<Rc<PortCell<T>>>>>>;
        type UncheckedCell<T> = AtomicRefCell<T>;
    } else {
        type Downstreams<T> = AtomicRefCell<HashMap<PortId, Rc<UnsafeCell<Rc<PortCell<T>>>>>>;
        type UncheckedCell<T> = UnsafeCell<T>;
    }
}

/// This is the internal cell type that is shared by ports.
struct PortCell<T: Sync> {
    /// Cell for the value.
    value: UncheckedCell<Option<T>>,

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
    downstreams: Downstreams<T>,
}

impl<T: Sync> PortCell<T> {
    fn check_cycle(&self, upstream_id: &PortId, downstream_id: &PortId) -> Result<(), AssemblyError> {
        if (*self.downstreams.borrow()).contains_key(upstream_id) {
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

/// A multiport is a vector of independent ports (its _channels_)
/// Multiports have special Lingua Franca syntax, similar to reactor banks.
pub struct Multiport<T: Sync> {
    ports: Vec<Port<T>>,
    id: TriggerId,
}

impl<T: Sync> Multiport<T> {
    /// Create a multiport from the given vector of ports.
    #[inline(always)]
    pub(crate) fn new(ports: Vec<Port<T>>, id: TriggerId) -> Self {
        Self { ports, id }
    }

    /// Returns the number of channels.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.ports.len()
    }

    /// Returns true if this multiport is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.ports.is_empty()
    }

    /// Iterate over the multiport and return mutable references to individual channels.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Port<T>> {
        self.ports.iter_mut()
    }

    /// Iterate over the channels of this multiport. Returns read-only
    /// references to individual ports.
    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = &Port<T>> {
        self.into_iter()
    }

    /// Iterate over only those channels that are set (have a value).
    /// Returns a tuple with their index (not necessarily contiguous).
    pub fn enumerate_set(&self) -> impl Iterator<Item = (usize, &Port<T>)> {
        self.iter().enumerate().filter(|&(_, p)| p.is_present_now())
    }

    /// Iterate over only those channels that are set (have a value).
    /// The returned ports are not necessarily contiguous. See
    /// [Self::enumerate_set] to get access to their index.
    pub fn iterate_set(&self) -> impl Iterator<Item = &Port<T>> {
        self.iter().filter(|&p| p.is_present_now())
    }

    /// Iterate over only those channels that are set (have a value),
    /// and return a copy of the value.
    /// The returned ports are not necessarily contiguous. See
    /// [Self::enumerate_values] to get access to their index.
    pub fn iterate_values(&self) -> impl Iterator<Item = T> + '_
    where
        T: Copy,
    {
        self.iter().filter_map(|p| p.get())
    }

    /// Iterate over only those ports that are set (have a value),
    /// and return a reference to the value.
    /// The returned ports are not necessarily contiguous. See
    /// [Self::enumerate_values] to get access to their index.
    #[cfg(not(feature = "no-unsafe"))]
    pub fn iterate_values_ref(&self) -> impl Iterator<Item = &T> + '_ {
        self.iter().filter_map(|p| p.get_ref())
    }

    /// Iterate over only those channels that are set (have a value),
    /// yielding a tuple with their index in the bank and a copy of the value.
    pub fn enumerate_values(&self) -> impl Iterator<Item = (usize, T)> + '_
    where
        T: Copy,
    {
        self.iter().enumerate().filter_map(|(i, p)| p.get().map(|v| (i, v)))
    }

    /// Iterate over only those channels that are set (have a value),
    /// yielding a tuple with their index in the bank and a reference to the value.
    #[cfg(not(feature = "no-unsafe"))]
    pub fn enumerate_values_ref(&self) -> impl Iterator<Item = (usize, &T)> + '_ {
        self.iter().enumerate().filter_map(|(i, p)| p.get_ref().map(|v| (i, v)))
    }
}

impl<T: Sync> TriggerLike for Multiport<T> {
    fn get_id(&self) -> TriggerId {
        self.id
    }
}

impl<'a, T: Sync> IntoIterator for &'a mut Multiport<T> {
    type Item = &'a mut Port<T>;
    type IntoIter = std::slice::IterMut<'a, Port<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.ports.iter_mut()
    }
}

impl<'a, T: Sync> IntoIterator for &'a Multiport<T> {
    type Item = &'a Port<T>;
    type IntoIter = std::slice::Iter<'a, Port<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.ports.iter()
    }
}

impl<T: Sync> Index<usize> for Multiport<T> {
    type Output = Port<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.ports[index]
    }
}

impl<T: Sync> IndexMut<usize> for Multiport<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.ports[index]
    }
}
