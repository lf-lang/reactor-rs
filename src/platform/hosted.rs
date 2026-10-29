//! The `std` backend. These are the same types the runtime has
//! always used; they're only re-exported from here.

pub use std::collections::hash_map::Entry as HashMapEntry;
#[allow(unused_imports)]
pub use std::collections::{HashMap, HashSet};
pub use std::time::Instant;

pub(crate) use crossbeam_channel::reconnectable::{unbounded, Receiver, RecvTimeoutError, SendError, Sender};

/// A lock around a value, used to share physical actions
/// between the scheduler and asynchronous threads.
pub(crate) struct Mutex<T>(std::sync::Mutex<T>);

impl<T> Mutex<T> {
    pub(crate) fn new(t: T) -> Self {
        Self(std::sync::Mutex::new(t))
    }

    /// Run `f` with the locked value. Fails if the lock is poisoned.
    #[inline]
    pub(crate) fn with<O>(&self, f: impl FnOnce(&mut T) -> O) -> Result<O, ()> {
        let mut guard = self.0.lock().map_err(|_| ())?;
        Ok(f(&mut guard))
    }
}
