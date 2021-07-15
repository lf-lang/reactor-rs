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

#[cfg(test)]
pub mod test;

pub use self::actions::*;
pub use self::components::*;
pub use self::ports::*;

pub use self::scheduler::*;
pub use self::time::*;
pub use self::util::*;

mod scheduler;
mod ports;
mod actions;
mod time;
mod components;
mod util;

// todo doc
#[macro_export]
macro_rules! new_reaction {
    ($reactorid:ident, $reactionid:ident, $_rstate:ident, $name:ident) => {{
        let r = ::std::sync::Arc::new(
            $crate::ReactionInvoker::new(
                $reactorid,
                $reactionid,
                $_rstate.clone(),
                <Self::RState as $crate::ReactorDispatcher>::ReactionId::$name
            )
        );
        $reactionid += 1;
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
    type ReactionId: Copy + Send + Sync;
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
    /// The logical context can be used to schedule things at
    /// the startup time of the app (time zero).
    fn start(&mut self, ctx: &mut StartupCtx);

    /// Create a new instance. The rid is a counter used to
    /// give unique IDs to reactions. The args are passed down
    /// to [ReactorDispatcher::assemble].
    ///
    /// The components of the ReactorDispatcher must be filled
    /// in with their respective dependencies (precomputed before
    /// codegen)
    fn assemble(rid: &mut ReactorId, args: <Self::RState as ReactorDispatcher>::Params) -> Self;
}


// helper for the macro below
#[macro_export]
macro_rules! reaction_ids_helper {
        (($self:expr) $t:ident :end:) => {
            if Self::$t == $self {
                ::std::stringify!($t)
            } else {
                panic!("Unreachable code")
            }
        };
        (($self:expr) $t:ident, $($ts:ident),+ :end:) => {
            if Self::$t == $self {
                ::std::stringify!($t)
            } else {
                reaction_ids_helper!(($self) $($ts),+ :end:)
            }
        }
    }

/// Declare a new type for reaction ids and derives the correct
/// traits. For example:
///
/// ```
/// # #[macro_use] extern crate reactor_rt;
/// reaction_ids!(pub enum AppReactions { Receive, Emit });
/// ```
///
/// defines that enum and derives [Named](Named)
/// and [Enumerated](Enumerated).
#[macro_export]
macro_rules! reaction_ids {
        ($viz:vis enum $typename:ident { }) => {
            #[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Hash, Copy, Clone)]
            $viz enum $typename {}

            impl reactor_rt::Named for $typename {
                fn name(&self) -> &'static str {
                    unreachable!()
                }
            }

            impl reactor_rt::Enumerated for $typename {
                fn list() -> Vec<Self> {
                    vec![]
                }
            }
        };
        ($viz:vis enum $typename:ident { $($id:ident),+$(,)? }) => {

            #[derive(Ord, PartialOrd, Eq, PartialEq, Debug, Hash, Copy, Clone)]
            $viz enum $typename {
                $($id),+
            }

            impl reactor_rt::Named for $typename {
                fn name(&self) -> &'static str {
                    let me = *self;
                    reaction_ids_helper!((me) $($id),+ :end:)
                }
            }

            impl reactor_rt::Enumerated for $typename {
                fn list() -> Vec<Self> {
                    vec![ $(Self::$id),+ ]
                }
            }
        };
}


