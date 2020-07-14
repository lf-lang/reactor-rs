pub mod world;
mod reaction;
pub mod action;
pub mod ports;
pub mod assembler;
pub mod framework;
mod flowgraph;
pub mod util;
mod id;



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
/// defines that enum and derives [Named] and [Enumerated].
#[macro_export]
macro_rules! reaction_ids {
    ($viz:vis enum $typename:ident { $($id:ident),+ }) => {

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
