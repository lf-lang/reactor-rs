use std::rc::Rc;
use std::cell::{Cell, Ref};
use crate::runtime::{ReactionInvoker, Dependencies, LogicalTime, MicroStep};
use std::marker::PhantomData;
use std::ops::Deref;
use std::cell::RefCell;
use std::fmt::*;
use std::time::{Duration, Instant};
use crate::reactors::Named;

pub struct Logical;

pub struct Physical;

pub type LogicalAction = Action<Logical>;
pub type PhysicalAction = Action<Physical>;

pub struct Action<Logical> {
    pub(in super) delay: Duration,
    pub(in super) downstream: Dependencies,
    name: &'static str,
    is_logical: bool,
    _logical: PhantomData<Logical>,
}

impl<T> Action<T> {
    pub fn set_downstream(&mut self, r: Dependencies) {
        self.downstream = r
    }

    pub fn make_eta(&self, now: LogicalTime, micro: MicroStep, additional_delay: Duration) -> LogicalTime {
        let min_delay = self.delay + additional_delay;
        let mut instant = now.instant + min_delay;
        if !self.is_logical {
            // physical actions are adjusted to physical time if needed
            instant = Instant::max(instant, Instant::now());
        }

        LogicalTime {
            instant,
            microstep: micro,
        }
    }

    fn new_impl(
        min_delay: Option<Duration>,
        is_logical: bool,
        name: &'static str) -> Self {
        Action {
            delay: min_delay.unwrap_or(Duration::new(0, 0)),
            is_logical,
            downstream: Default::default(),
            name,
            _logical: PhantomData,
        }
    }
}

impl LogicalAction {
    pub fn new(min_delay: Option<Duration>, name: &'static str) -> Self {
        Self::new_impl(min_delay, true, name)
    }
}

impl PhysicalAction {
    pub fn new(min_delay: Option<Duration>, name: &'static str) -> Self {
        Self::new_impl(min_delay, false, name)
    }
}

impl<T> Named for Action<T> {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl<T> Display for Action<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Display>::fmt(&self.name(), f)
    }
}
