use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::borrow::Borrow;
use std::ops::DerefMut;

/// A type whose instances have statically known names
pub trait Named {
    fn name(&self) -> &'static str;
}

/// A type that can list all its instances
pub trait Enumerated {
    fn list() -> Vec<Self> where Self: Sized;
}


pub fn borrow_mut<T>(cell: &Rc<RefCell<T>>) -> RefMut<T> {
    cell.borrow_mut()
}
