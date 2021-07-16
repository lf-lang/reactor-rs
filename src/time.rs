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

use std::fmt::{Display, Formatter, Debug};
use super::{PhysicalInstant, Duration};
use std::ops::Add;

/// Type of the microsteps of a [LogicalInstant]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Hash)]
pub struct MicroStep(u64);

impl MicroStep {
    pub const ZERO: MicroStep = MicroStep(0);
}

impl Add<u64> for MicroStep {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

/// A logical instant the union of an [PhysicalInstant], ie a point
/// in time, and a microstep. An [PhysicalInstant] can be sampled with
/// [PhysicalInstant.now], which gives the current physical time. The
/// current logical instant of the application may lag behind
/// physical time. Timekeeping of the logical timeline is at
/// the core of the scheduler, and the current logical time may
/// only be accessed through a [LogicalCtx].
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

impl Default for LogicalInstant {
    #[inline]
    fn default() -> Self {
        Self::now()
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
}



#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub enum Offset {
    Asap,
    After(Duration),
}

impl Offset {
    // Duration::zero() is unstable
    const ZERO_DURATION: Duration = Duration::from_millis(0);

    #[inline]
    pub fn to_duration(&self) -> Duration {
        match self {
            Offset::Asap => Self::ZERO_DURATION,
            Offset::After(d) => d.clone()
        }
    }
}
