use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::borrow::Borrow;
use std::ops::DerefMut;
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;

/// A type whose instances have statically known names
pub trait Named {
    fn name(&self) -> &'static str;
}

/// A type that can list all its instances
pub trait Enumerated {
    fn list() -> Vec<Self> where Self: Sized;
}


/// A type with no instances.
/// Rust's bottom type, `!`, is experimental
pub enum Nothing {}

impl PartialEq for Nothing {
    fn eq(&self, _: &Self) -> bool {
        panic!("No instance of this type")
    }
}

impl Clone for Nothing {
    fn clone(&self) -> Self {
        panic!("No instance of this type")
    }
}
impl Copy for Nothing {}

impl Eq for Nothing {}

impl Hash for Nothing {
    fn hash<H: Hasher>(&self, _: &mut H) {
        panic!("No instance of this type")
    }
}

impl PartialOrd for Nothing {
    fn partial_cmp(&self, _: &Self) -> Option<Ordering> {
        panic!("No instance of this type")
    }
}

impl Ord for Nothing {
    fn cmp(&self, _: &Self) -> Ordering {
        panic!("No instance of this type")
    }
}

impl Named for Nothing {
    fn name(&self) -> &'static str {
        panic!("No instance of this type")
    }
}

impl Enumerated for Nothing {
    fn list() -> Vec<Self> where Self: Sized {
        vec![]
    }
}

