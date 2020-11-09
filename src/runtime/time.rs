use std::rc::Rc;
use std::cell::{Cell, Ref};
use crate::runtime::{ReactionInvoker, Dependencies};
use std::marker::PhantomData;
use std::ops::Deref;
use std::cell::RefCell;
use std::time::Instant;
use std::fmt::{Display, Formatter, Debug};


pub(in super) type MicroStep = u128;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct LogicalTime {
    pub(in super) instant: Instant,
    pub(in super) microstep: MicroStep,
}

impl Display for LogicalTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Debug>::fmt(self, f)
    }
}

impl Default for LogicalTime {
    fn default() -> Self {
        Self::now()
    }
}

impl LogicalTime {
    pub fn to_instant(&self) -> Instant {
        self.instant
    }
    pub fn now() -> Self {
        Self {
            instant: Instant::now(),
            microstep: 0,
        }
    }
}
