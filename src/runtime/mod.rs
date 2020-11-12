pub use self::actions::*;
pub use self::components::*;
pub use self::ports::*;

pub use self::scheduler::*;
pub use self::time::*;
use crate::reactors::Named;

mod scheduler;
mod ports;
mod actions;
mod time;
mod components;


mod fiddling;


#[macro_export]
macro_rules! new_reaction {
    ($rid:ident, $_rstate:ident, $name:ident) => {{
        let r = Arc::new(ReactionInvoker::new(*$rid, $_rstate.clone(), <Self::RState as ReactorDispatcher>::ReactionId::$name));
        *$rid += 1;
        r
    }};
}

/// Wrapper around the user struct for safe dispatch.
///
/// Fields are
/// 1. the user struct, and
/// 2. every logical action and port declared by the reactor.
///
pub trait ReactorDispatcher: Send + Sync {
    /// The type of reaction IDs
    type ReactionId: Copy + Named + Send + Sync;
    /// Type of the user struct
    type Wrapped;
    /// Type of the construction parameters
    type Params;

    /// Assemble the user reactor, ie produce components with
    /// uninitialized dependencies & make state variables assume
    /// their default values, or else, a value taken from the params.
    fn assemble(args: Self::Params) -> Self;

    /// Execute a single user-written reaction.
    /// Dispatches on the reaction id, and unpacks parameters,
    /// which are the reactor components declared as fields of
    /// this struct.
    fn react(&mut self, ctx: &mut LogicalCtx, rid: Self::ReactionId);
}

/// Declares dependencies of every reactor component. Also
/// initializes reaction wrappers.
///
/// Fields are
/// 1. an Arc<Mutex<Self::RState>>
/// 2. an Arc<ReactionInvoker> for every reaction declared by the reactor
///
pub trait ReactorAssembler {
    /// Type of the [ReactorDispatcher]
    type RState: ReactorDispatcher;

    /// Execute the startup reaction of the reactor
    /// This also creates physical actions.
    fn start(&mut self, ctx: PhysicalCtx);

    /// Create a new instance. The rid is a counter used to
    /// give unique IDs to reactions. The args are passed down
    /// to [ReactorDispatcher::assemble].
    ///
    /// The components of the ReactorDispatcher must be filled
    /// in with their respective dependencies (precomputed before
    /// codegen)
    fn assemble(rid: &mut i32, args: <Self::RState as ReactorDispatcher>::Params) -> Self;
}
