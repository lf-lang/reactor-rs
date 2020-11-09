use std::rc::Rc;
use std::cell::{Cell, Ref};
use crate::runtime::{ReactionInvoker, Dependencies};
use std::marker::PhantomData;
use std::ops::Deref;
use std::cell::RefCell;
use std::fmt::*;
use std::time::Duration;
use crate::reactors::Named;

pub struct Action {
    pub(in super) delay: Duration,
    pub(in super) logical: bool,
    pub(in super) downstream: Dependencies,
    name: &'static str,
}

impl Action {
    pub fn set_downstream(&mut self, r: Dependencies) {
        self.downstream = r
    }

    pub fn new(
        min_delay: Option<Duration>,
        is_logical: bool,
        name: &'static str) -> Self {
        Action {
            delay: min_delay.unwrap_or(Duration::new(0, 0)),
            logical: is_logical,
            downstream: Default::default(),
            name,
        }
    }
}

impl Named for Action {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
    }
}
