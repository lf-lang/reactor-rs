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

use std::fmt::{Debug, Display, Formatter};
use std::ops::Add;


use super::{Duration, PhysicalInstant};


/// A point on the logical timeline.
///
/// Logical time is measured with the same units as physical
/// time. A LogicalInstant hence contains a [PhysicalInstant].
/// But importantly, logical time implements *superdense time*,
/// which means an infinite sequence of logical instants may
/// correspond to any physical instant. The label on this sequence
/// is called the *microstep*.
///
/// The current logical time of the application may lag behind
/// physical time. Timekeeping of the logical timeline is at
/// the core of the scheduler, and within reactions, the current
/// logical time may only be accessed through a [LogicalCtx].
///
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct LogicalInstant {
    /// This is an instant in time. Physical time is measured
    /// with the same unit.
    pub instant: PhysicalInstant,
    /// The microstep at this time.
    pub microstep: MicroStep,
}

impl Display for LogicalInstant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <_ as Debug>::fmt(self, f)
    }
}

impl LogicalInstant {
    #[inline]
    pub fn now() -> Self {
        Self {
            instant: PhysicalInstant::now(),
            microstep: MicroStep::ZERO,
        }
    }

    #[inline]
    pub fn next_microstep(&self) -> Self {
        Self {
            instant: self.instant,
            microstep: self.microstep + 1,
        }
    }
}


impl Add<Duration> for LogicalInstant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self {
            instant: self.instant + rhs,
            microstep: MicroStep::ZERO,
        }
    }
}

/// Private concrete type of a microstep.
type MS = u32;

/// Type of the microsteps of a [LogicalInstant]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct MicroStep(MS);

impl MicroStep {
    pub const ZERO: MicroStep = MicroStep(0);
    pub fn new(u: MS) -> Self {
        Self(u)
    }
}

impl Display for MicroStep {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Add<MS> for MicroStep {
    type Output = Self;
    #[inline]
    fn add(self, rhs: MS) -> Self::Output {
        Self(self.0 + rhs)
    }
}

