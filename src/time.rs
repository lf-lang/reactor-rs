





use std::time::{Instant, Duration};
use std::fmt::{Display, Formatter, Debug};


pub(in super) type MicroStep = u128;

/// Logical time is the union of an [Instant], ie a point in time
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct LogicalTime {
    /// This is an instant in time. Physical time is measured
    /// with the same unit.
    pub(in super) instant: Instant,
    /// The microstep at this time.
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



#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Offset {
    Asap,
    After(Duration),
}

impl Offset {
    // Duration::zero() is unstable
    const ZERO_DURATION: Duration = Duration::from_millis(0);

    pub fn to_duration(&self) -> Duration {
        match self {
            Offset::Asap => Self::ZERO_DURATION,
            Offset::After(d) => d.clone()
        }
    }
}
