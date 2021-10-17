use std::fmt::{Debug, Display, Formatter};
use AssemblyErrorImpl::*;

use crate::PortId;

/// An error occurring during initialization of the reactor program.
/// Should never occur unless the graph is built by hand, and not
/// by a Lingua Franca compiler.
pub struct AssemblyError(pub(crate) AssemblyErrorImpl);

pub(crate) enum AssemblyErrorImpl {
    CyclicDependency(PortId, PortId),
    CyclicDependencyGraph,
    CannotBind(PortId, PortId),
}

impl Debug for AssemblyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            CyclicDependency(upstream, downstream) => write!(f, "Port {} is already in the downstream of port {}", upstream, downstream),
            CyclicDependencyGraph => write!(f, "Cyclic dependency graph"),
            CannotBind(upstream, downstream) => write!(f, "Cannot bind {} to {}, downstream is already bound", upstream, downstream),
        }
    }
}

impl Display for AssemblyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}
