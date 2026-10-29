//! The `embassy` backend, for `no_std` targets.
//!
//! The user's binary must provide a global allocator and an
//! embassy time driver (usually through their HAL crate).

use core::fmt::{Debug, Formatter};
use core::hash::{Hash, Hasher};
use core::ops::{Add, Sub};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use atomic_refcell::AtomicRefCell;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::TICK_HZ;

pub use hashbrown::hash_map::Entry as HashMapEntry;
#[allow(unused_imports)]
pub use hashbrown::{HashMap, HashSet};

const NANOS_PER_SEC: u128 = 1_000_000_000;

/// A point in physical time, read from the embassy time driver.
///
/// This mirrors the parts of `std::time::Instant` that the runtime
/// and generated code use.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Instant(embassy_time::Instant);

impl Hash for Instant {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_ticks().hash(state)
    }
}

impl Instant {
    /// The current time.
    #[inline]
    pub fn now() -> Self {
        Self(embassy_time::Instant::now())
    }

    /// The amount of time elapsed from `earlier` to `self`,
    /// or zero if `earlier` is later than `self`.
    #[inline]
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or(Duration::ZERO)
    }

    /// The amount of time elapsed from `earlier` to `self`,
    /// or `None` if `earlier` is later than `self`.
    #[inline]
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        let ticks = self.0.as_ticks().checked_sub(earlier.0.as_ticks())?;
        Some(ticks_to_duration(ticks))
    }

    /// The amount of time elapsed since this instant.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        Self::now().duration_since(*self)
    }

    /// Returns `Some(t)` where `t` is `self + duration`, or `None` on overflow.
    #[inline]
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.0.as_ticks().checked_add(duration_to_ticks(duration)?).map(|t| Self(embassy_time::Instant::from_ticks(t)))
    }

    /// Returns `Some(t)` where `t` is `self - duration`, or `None` on underflow.
    #[inline]
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.0.as_ticks().checked_sub(duration_to_ticks(duration)?).map(|t| Self(embassy_time::Instant::from_ticks(t)))
    }

    /// Wrap an embassy instant.
    #[inline]
    pub fn from_embassy(instant: embassy_time::Instant) -> Self {
        Self(instant)
    }

    /// The underlying embassy instant.
    #[inline]
    pub fn to_embassy(self) -> embassy_time::Instant {
        self.0
    }
}

/// Convert driver ticks to a duration (rounds down).
fn ticks_to_duration(ticks: u64) -> Duration {
    let nanos = ticks as u128 * NANOS_PER_SEC / TICK_HZ as u128;
    Duration::new((nanos / NANOS_PER_SEC) as u64, (nanos % NANOS_PER_SEC) as u32)
}

/// Convert a duration to driver ticks (rounds up, so that
/// sleeping until `t0 + d` never wakes up early).
fn duration_to_ticks(d: Duration) -> Option<u64> {
    let ticks = (d.as_nanos() * TICK_HZ as u128).div_ceil(NANOS_PER_SEC);
    u64::try_from(ticks).ok()
}

impl Add<Duration> for Instant {
    type Output = Instant;

    #[inline]
    fn add(self, rhs: Duration) -> Self::Output {
        self.checked_add(rhs).expect("overflow when adding duration to instant")
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    #[inline]
    fn sub(self, rhs: Duration) -> Self::Output {
        self.checked_sub(rhs).expect("overflow when subtracting duration from instant")
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    /// Like `std`, this saturates to zero if `rhs` is later than `self`.
    #[inline]
    fn sub(self, rhs: Instant) -> Self::Output {
        self.duration_since(rhs)
    }
}

impl Debug for Instant {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "Instant({:?} since boot)", ticks_to_duration(self.0.as_ticks()))
    }
}

/// A lock around a value, used to share physical actions
/// between the scheduler and embassy tasks or interrupts.
///
/// This never blocks: on a single core, a lock that is
/// already taken means we interrupted its holder, and
/// waiting would deadlock. In that case [Self::with] fails.
pub(crate) struct Mutex<T>(AtomicRefCell<T>);

impl<T> Mutex<T> {
    pub(crate) fn new(t: T) -> Self {
        Self(AtomicRefCell::new(t))
    }

    /// Run `f` with the locked value. Fails if the value is
    /// currently in use elsewhere.
    #[inline]
    pub(crate) fn with<O>(&self, f: impl FnOnce(&mut T) -> O) -> Result<O, ()> {
        let mut guard = self.0.try_borrow_mut().map_err(|_| ())?;
        Ok(f(&mut guard))
    }
}

/// Returned when an event could not be sent to the scheduler,
/// because it has shut down or its queue is full. Contains the
/// value that could not be sent.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct SendError<T>(pub T);

impl<T> Debug for SendError<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("SendError(..)")
    }
}

/// A fixed-capacity queue of events that interrupt handlers and
/// embassy tasks can push into. It must be stored in a `static`.
///
/// It keeps track of how many [Sender]s are alive, so that the
/// scheduler can tell when no more events can arrive (like a
/// disconnected channel with `std`).
pub(crate) struct EventChannel<T, const N: usize> {
    queue: Channel<CriticalSectionRawMutex, T, N>,
    senders: AtomicUsize,
    /// Signalled when the last sender is dropped.
    disconnected: Signal<CriticalSectionRawMutex, ()>,
}

impl<T, const N: usize> EventChannel<T, N> {
    pub(crate) const fn new() -> Self {
        Self {
            queue: Channel::new(),
            senders: AtomicUsize::new(0),
            disconnected: Signal::new(),
        }
    }
}

/// The scheduler's end of an [EventChannel].
pub(crate) struct Receiver<T: 'static, const N: usize>(&'static EventChannel<T, N>);

impl<T, const N: usize> Receiver<T, N> {
    /// Connect to the channel. Any events left over from an
    /// earlier run are dropped.
    pub(crate) fn new(channel: &'static EventChannel<T, N>) -> Self {
        channel.queue.clear();
        Self(channel)
    }

    pub(crate) fn new_sender(&self) -> Sender<T, N> {
        self.0.senders.fetch_add(1, Ordering::SeqCst);
        Sender(self.0)
    }

    /// Take all events that are ready, without waiting.
    pub(crate) fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        core::iter::from_fn(move || self.0.queue.try_receive().ok())
    }

    /// Wait for the next event. Returns `None` once there
    /// are no senders left and the queue is empty.
    pub(crate) async fn recv(&self) -> Option<T> {
        loop {
            if let Ok(t) = self.0.queue.try_receive() {
                return Some(t);
            }
            if self.0.senders.load(Ordering::SeqCst) == 0 {
                return None;
            }
            self.0.disconnected.reset();
            // a sender may have been dropped between the check above and the reset
            if self.0.senders.load(Ordering::SeqCst) == 0 {
                continue;
            }
            match select(self.0.queue.receive(), self.0.disconnected.wait()).await {
                Either::First(t) => return Some(t),
                Either::Second(()) => continue,
            }
        }
    }
}

/// A handle that can push events into an [EventChannel].
/// It is cheap to clone, and can be used from interrupts.
pub(crate) struct Sender<T: 'static, const N: usize>(&'static EventChannel<T, N>);

impl<T, const N: usize> Sender<T, N> {
    /// Push an event without waiting. Fails if the queue is full.
    pub(crate) fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.0.queue.try_send(t).map_err(|embassy_sync::channel::TrySendError::Full(t)| SendError(t))
    }
}

impl<T, const N: usize> Clone for Sender<T, N> {
    fn clone(&self) -> Self {
        self.0.senders.fetch_add(1, Ordering::SeqCst);
        Self(self.0)
    }
}

impl<T, const N: usize> Drop for Sender<T, N> {
    fn drop(&mut self) {
        if self.0.senders.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.0.disconnected.signal(());
        }
    }
}
