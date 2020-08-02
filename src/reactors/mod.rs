use std::hash::Hash;

#[doc(inline)]
pub use self::action::*;
#[doc(inline)]
pub use self::assembler::*;
#[doc(inline)]
pub use self::ports::*;
#[doc(inline)]
pub use self::scheduler::*;
pub use self::util::*;
#[doc(inline)]
pub use self::world::*;

mod scheduler;
mod world;
mod reaction;
mod action;
mod ports;
mod assembler;
mod flowgraph;
mod util;
mod id;


/// Describes the structure of a reactor.
///
/// Instances are created by cooperating with an [Assembler].
/// Sub-components of the reactor (ports, sub-reactors, etc)
/// are created by the assembler and should be stored in
/// instances. They're immutable.
///
/// Mutable state variables are split into a [State](Reactor::State)
/// variable, that is managed by the framework.
pub trait Reactor {
    /// Enumerates the reactions available for this reactor.
    /// This is used as input to the [`react`](Self::react) method.
    ///
    /// # Examples
    ///
    /// ```
    /// // Use this macro to derive Named + Enumerated on an enum
    /// reaction_ids!(pub enum MyReactions { Emit, Receive })
    ///
    /// impl Reactor for MyReactor {
    ///
    ///     type ReactionId = MyReactions;
    ///
    ///     // Handle each reaction in react
    ///     fn react(...) {
    ///         match reaction_id {
    ///             MyReactions::Emit =>    { ... }
    ///             MyReactions::Receive => { ... }
    ///         }
    ///     }
    /// }
    ///
    /// ```
    type ReactionId: Ord + Eq + Hash + Enumerated + Named + Sized + Copy;

    /// The type for the internal state of the reactor.
    ///
    /// The self instance should not contain the internal state variables.
    /// It couldn't use it anyway, since the [`react`](Self::react) method
    /// take a `self` argument as an immutable reference.
    ///
    /// Override [`initial_state`](Self::initial_state) to provide the initial
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// type State = ();  // For a stateless reactor
    /// type State = i32; // A single state variable
    /// ```
    ///
    type State: Sized;

    /// Produce the initial state.
    fn initial_state() -> Self::State where Self: Sized;

    /// Initializes the structure of this reactor.
    /// This should create subcomponents and link them using the [Assembler].
    ///
    /// TODO would be nice if this "constructor" could take additional parameters
    fn assemble<'g>(assembler: &mut Assembler<'_, 'g, Self>) -> Result<Self, AssemblyError> where Self: Sized;

    /// Execute a reaction of this reactor.
    ///
    /// # Parameters
    ///
    /// - reactor: The assembled reactor
    /// - state: A mutable reference to the internal reactor state
    /// - reaction_id: ID of the reaction to execute
    /// - scheduler: Scheduler instance, that can make the reaction affect the event queue
    ///
    fn react<'g>(
        reactor: &RunnableReactor<'g, Self>,
        state: &mut Self::State,
        reaction_id: Self::ReactionId,
        ctx: &mut ReactionCtx<'_, 'g>,
    ) where Self: Sized + 'g; // todo this could return a Result
}


// helper for the macro below
#[macro_export]
macro_rules! reaction_ids_helper {
        (($self:expr) $t:ident) => {
            if Self::$t == $self {
                ::std::stringify!($t)
            }
        };
        (($self:expr) $t:ident :end:) => {
            if Self::$t == $self {
                ::std::stringify!($t)
            } else {
                panic!("Unreachable code")
            }
        };
        (($self:expr) $t:ident, $($ts:ident),+ :end:) => {
            name_match!(($self) $t)
            else name_match!(($self) $($ts),+)
        }
    }

/// Declare a new type for reaction ids and derives the correct
/// traits. For example:
///
/// ```
/// reaction_ids!(pub enum AppReactions { Receive, Emit })
/// ```
///
/// defines that enum and derives [Named](crate::reactors::util::Named)
/// and [Enumerated](crate::reactors::util::Enumerated).
#[macro_export]
macro_rules! reaction_ids {
        ($viz:vis enum $typename:ident { $($id:ident),+$(,)? }) => {

            #[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Hash, Copy, Clone)]
            $viz enum $typename {
                $($id),+
            }

            impl Named for $typename {
                fn name(&self) -> &'static str {
                    let me = *self;
                    reaction_ids_helper!((me) $($id),+ :end:)
                }
            }

            impl Enumerated for $typename {
                fn list() -> Vec<Self> {
                    vec![ $(Self::$id),+ ]
                }
            }
        };

}
