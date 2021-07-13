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
