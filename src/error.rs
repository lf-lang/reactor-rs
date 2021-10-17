
use AssemblyErrorImpl::*;

use crate::{DebugInfoRegistry, PortId};

pub type AssemblyResult<T = ()> = Result<T, AssemblyError>;

/// An error occurring during initialization of the reactor program.
/// Should never occur unless the graph is built by hand, and not
/// by a Lingua Franca compiler.
pub struct AssemblyError(pub(crate) AssemblyErrorImpl);

impl AssemblyError {
    pub(crate) fn lift(self, debug: &DebugInfoRegistry) -> String {
        self.display(debug)
    }
}

pub(crate) enum AssemblyErrorImpl {
    CyclicDependency(PortId, PortId),
    CyclicDependencyGraph,
    CannotBind(PortId, PortId),
}

impl AssemblyError {
    fn display(&self, debug: &DebugInfoRegistry) -> String {
        match self.0 {
            CyclicDependency(upstream, downstream) => format!("Port {} is already in the downstream of port {}", debug.fmt_component(upstream), debug.fmt_component(downstream)),
            CyclicDependencyGraph => format!("Cyclic dependency graph"),
            CannotBind(upstream, downstream) => format!("Cannot bind {} to {}, downstream is already bound", debug.fmt_component(upstream), debug.fmt_component(downstream)),
        }
    }
}


