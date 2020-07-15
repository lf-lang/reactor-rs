use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::fmt::{Display, Formatter};
use petgraph::Graph;
use petgraph::visit::EdgeRef;

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
        unreachable!("No instance of Nothing type")
    }
}

impl Clone for Nothing {
    fn clone(&self) -> Self {
        unreachable!("No instance of Nothing type")
    }
}

impl Copy for Nothing {}

impl Eq for Nothing {}

impl Hash for Nothing {
    fn hash<H: Hasher>(&self, _: &mut H) {
        unreachable!("No instance of Nothing type")
    }
}

impl PartialOrd for Nothing {
    fn partial_cmp(&self, _: &Self) -> Option<Ordering> {
        unreachable!("No instance of Nothing type")
    }
}

impl Ord for Nothing {
    fn cmp(&self, _: &Self) -> Ordering {
        unreachable!("No instance of Nothing type")
    }
}

impl Named for Nothing {
    fn name(&self) -> &'static str {
        unreachable!("No instance of Nothing type")
    }
}

impl Enumerated for Nothing {
    fn list() -> Vec<Self> where Self: Sized {
        vec![]
    }
}

