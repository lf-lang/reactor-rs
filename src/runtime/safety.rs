use std::cell::{Cell, Ref};
use std::cell::RefCell;
use std::fmt::*;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::reactors::Named;
use crate::runtime::{Ctx, ReactorDispatcher};


struct In;

struct Out;

struct Port<T, K, L> {
    _p: PhantomData<(T, K, L)>
}

type InputPort<T, L> = Port<T, In, L>;
type OutputPort<T, L> = Port<T, Out, L>;

impl<T, UK, UL> Port<T, UK, UL> {
    fn forward_to<DK>(&self, down: &Port<T, DK, AllowedBinding<UK, DK, UL>>)
        where UK: ComputeBind<UL, DK> {
        TODO!()
    }
}

// down

// struct Binder<T, DK>
//
// fn binder<T, UK, UL>(u: Port<T, UK, UL>)
//     ->
//
type AllowedBinding<UK, DK, UL> = <UK as ComputeBind<UL, DK>>::DL;

trait ComputeBind<UL, DK> {
    type DL;
}

// one can bind In of self to Out of self
impl<T> ComputeBind<T, Out> for In {
    type DL = T;
}

// one can bind In of self to In of child
impl<T> ComputeBind<T, In> for In {
    type DL = Child<T>;
}

// one can bind Out of child to Out of self
impl<T> ComputeBind<Child<T>, Out> for Out {
    type DL = T;
}

// one can bind Out of child to In of other child
// note: this allows a binding
impl<T> ComputeBind<Child<T>, In> for Out {
    type DL = Child<T>;
}


fn test<P>(u : InputPort<i32, P>, d: OutputPort<i32, P>) {
    u.forward_to(&d);
}
//
// trait Sub<I> {
//     type T;
// }
//

struct Root;

struct Child<P> {
    _p: PhantomData<(P)>
}
