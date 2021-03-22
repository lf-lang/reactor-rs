





use std::time::{Instant, Duration};
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

    pub fn offset(self, duration: Duration, mstep: MicroStep) -> Self {
        Self {
            instant: self.instant + duration,
            microstep: self.microstep + mstep,
        }
    }

    pub fn now() -> Self {
        Self {
            instant: Instant::now(),
            microstep: 0,
        }
    }
}



#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Offset {
    Asap,
    After(Duration),
}

impl Offset {
    const ZERO_DURATION: Duration = Duration::from_millis(0);

    pub fn to_duration(&self) -> Duration {
        match self {
            Offset::Asap => Self::ZERO_DURATION,
            Offset::After(d) => d.clone()
        }
    }
}
