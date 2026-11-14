//! Platform-specific pieces of the runtime: the clock, locks,
//! hash maps and the channel used by asynchronous (physical) events.
//!
//! Everything else in the crate is written against `core` and `alloc`,
//! and imports these items from here. Exactly one backend is active:
//! - `std` (the default): the types used by the runtime on an OS.
//! - `embassy`: a `no_std` backend for microcontrollers, built on
//!   `embassy-time` and `embassy-sync`.
//!

cfg_if! {
    if #[cfg(feature = "std")] {
        mod hosted;
        pub use self::hosted::*;
    } else if #[cfg(feature = "embassy")] {
        mod embassy;
        pub use self::embassy::*;
    } else {
        compile_error!("reactor_rt needs a platform: enable either the `std` or the `embassy` feature");
    }
}
