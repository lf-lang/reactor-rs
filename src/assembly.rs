//! Module containing the API to initialize a reactor program.

use AssemblyErrorImpl::*;

pub use crate::ids::GlobalReactionId;
// this is where most of the stuff is implemented
pub use crate::scheduler::assembly_impl::*;
pub use crate::triggers::{TriggerId, TriggerLike};
use crate::{DebugInfoRegistry, LocalReactionId, ReactorBehavior};
pub(crate) type PortId = TriggerId;

/// Wrapper around the user struct for safe dispatch.
///
/// Fields are
/// 1. the user struct,
/// 2. ctor parameters of the reactor, and
/// 3. every logical action and port declared by the reactor.
///
pub trait ReactorInitializer: ReactorBehavior {
    /// Type of the user struct, which contains state variables of the reactor.
    /// Used by the runtime to produce debug information.
    type Wrapped;
    /// Type of the construction parameters.
    type Params;
    /// Exclusive maximum value of the `local_rid` parameter of [ReactorBehavior.react].
    const MAX_REACTION_ID: LocalReactionId;

    /// Assemble this reactor. This initializes state variables,
    /// produces internal components, assembles children reactor
    /// instances, and declares dependencies between them.
    fn assemble(args: Self::Params, assembler: AssemblyCtx<Self>) -> AssemblyResult<FinishedReactor<Self>>
    where
        Self: Sized;
}

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
    IdOverflow,
}

impl AssemblyError {
    fn display(&self, debug: &DebugInfoRegistry) -> String {
        match self.0 {
            CyclicDependency(upstream, downstream) => format!(
                "Port {} is already in the downstream of port {}",
                debug.fmt_component(upstream),
                debug.fmt_component(downstream)
            ),
            CyclicDependencyGraph => "Cyclic dependency graph".to_string(),
            CannotBind(upstream, downstream) => format!(
                "Cannot bind {} to {}, downstream is already bound",
                debug.fmt_component(upstream),
                debug.fmt_component(downstream)
            ),
            IdOverflow => "Overflow when allocating component ID".to_string(),
        }
    }
}

/// Kind of a port.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum PortKind {
    Input,
    Output,
    ChildInputReference,
    ChildOutputReference,
}
