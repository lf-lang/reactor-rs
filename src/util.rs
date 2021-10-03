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


use std::convert::TryFrom;
use std::time::Duration;

#[macro_export]
#[doc(hidden)]
macro_rules! join_to {
    ($f:expr, $iter:expr) => {join_to!($f, $iter, ", ")};
    ($f:expr, $iter:expr, $sep:literal) => {join_to!($f, $iter, $sep, "", "")};
    ($f:expr, $iter:expr, $sep:literal, $prefix:literal, $suffix:literal) => {
        join_to!($f, $iter, $sep, $prefix, $suffix, |x| format!("{}", x))
    };
    ($f:expr, $iter:expr, $sep:literal, $prefix:literal, $suffix:literal, $display:expr) => {
        {
            crate::util::do_write($f, $iter, $sep, $prefix, $suffix, $display)
        }
    };
}

pub fn do_write<X>(f: &mut impl std::fmt::Write,
                   iter: impl Iterator<Item=X>,
                   sep: &'static str,
                   prefix: &'static str,
                   suffix: &'static str,
                   formatter: impl Fn(X) -> String) -> std::fmt::Result {
    let mut iter = iter;
    write!(f, "{}", prefix)?;
    if let Some(first) = iter.next() {
        write!(f, "{}", formatter(first))?;
    }
    for item in iter {
        write!(f, "{}", sep)?;
        write!(f, "{}", formatter(item))?;
    }
    write!(f, "{}", suffix)
}

/// Creates a [Duration] value using the same syntax as in LF.
///
/// ```
/// use std::time::Duration;
/// use reactor_rt::delay;
///
/// assert_eq!(delay!(10 ns), Duration::from_nanos(10));
/// assert_eq!(delay!(10 ms), delay!(10 msec));
/// assert_eq!(delay!(10 msec), Duration::from_millis(10));
/// assert_eq!(delay!(10 sec), Duration::from_secs(10));
/// assert_eq!(delay!(2 min), delay!(120 s));
/// ```
#[macro_export]
macro_rules! delay {
    (0)                     => { $crate::Duration::from_nanos(0) };
    ($amount:literal ns)    => { $crate::Duration::from_nanos($amount) };
    ($amount:literal nsec)  => { delay!($amount ns) };
    ($amount:literal nsecs) => { delay!($amount ns) };
    ($amount:literal us)    => { $crate::Duration::from_micros($amount) };
    ($amount:literal usec)  => { delay!($amount us) };
    ($amount:literal usecs) => { delay!($amount us) };
    ($amount:literal ms)    => { $crate::Duration::from_millis($amount) };
    ($amount:literal msec)  => { delay!($amount ms) };
    ($amount:literal msecs) => { delay!($amount ms) };
    ($amount:literal s)     => { $crate::Duration::from_secs($amount) };
    ($amount:literal sec)   => { delay!($amount s) };
    ($amount:literal secs)  => { delay!($amount s) };
    ($amount:literal second)   => { delay!($amount s) };
    ($amount:literal seconds)  => { delay!($amount s) };
    ($amount:literal min)      => { $crate::Duration::from_secs(60 * $amount) };
    ($amount:literal mins)     => { delay!($amount min) };
    ($amount:literal minute)   => { delay!($amount min) };
    ($amount:literal minutes)  => { delay!($amount min) };
    (($amount:expr) ns)    => { $crate::Duration::from_nanos($amount) };
    (($amount:expr) nsec)  => { delay!(($amount) ns) };
    (($amount:expr) nsecs) => { delay!(($amount) ns) };
    (($amount:expr) us)    => { $crate::Duration::from_micros($amount) };
    (($amount:expr) usec)  => { delay!(($amount) us) };
    (($amount:expr) usecs) => { delay!(($amount) us) };
    (($amount:expr) ms)    => { $crate::Duration::from_millis($amount) };
    (($amount:expr) msec)  => { delay!(($amount) ms) };
    (($amount:expr) msecs) => { delay!(($amount) ms) };
    (($amount:expr) s)     => { $crate::Duration::from_secs($amount) };
    (($amount:expr) sec)   => { delay!(($amount) s) };
    (($amount:expr) secs)  => { delay!(($amount) s) };
    (($amount:expr) second)   => { delay!(($amount) s) };
    (($amount:expr) seconds)  => { delay!(($amount) s) };
    (($amount:expr) min)      => { $crate::Duration::from_secs(60 * ($amount)) };
    (($amount:expr) mins)     => { delay!(($amount) min) };
    (($amount:expr) minute)   => { delay!(($amount) min) };
    (($amount:expr) minutes)  => { delay!(($amount) min) };
}

/// A unit of time, used in LF.
#[derive(Debug)]
pub enum TimeUnit {
    NANO,
    MICRO,
    MILLI,
    SEC,
    MIN,
    HOUR,
    DAY,
}

impl TryFrom<&str> for TimeUnit {
    type Error = ();

    /// This recognizes the same strings as LF
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let u = match value {
            "day" | "days" => Self::DAY,
            "h" | "hour" | "hours" => Self::HOUR,
            "min" | "minute" | "minutes" => Self::MIN,
            "s" | "sec" | "secs" => Self::SEC,
            "ms" | "msec" | "msecs" => Self::MILLI,
            "us" | "usec" | "usecs" => Self::MICRO,
            "ns" | "nsec" | "nsecs" => Self::NANO,
            _ => return Err(())
        };
        Ok(u)
    }
}

impl TimeUnit {
    pub fn to_duration(&self, magnitude: u64) -> Duration {
        match *self {
            TimeUnit::NANO => Duration::from_nanos(magnitude),
            TimeUnit::MICRO => Duration::from_micros(magnitude),
            TimeUnit::MILLI => Duration::from_millis(magnitude),
            TimeUnit::SEC => Duration::from_secs(magnitude),
            TimeUnit::MIN => Duration::from_secs(60 * magnitude),
            TimeUnit::HOUR => Duration::from_secs(60 * 60 * magnitude),
            TimeUnit::DAY => Duration::from_secs(60 * 60 * 24 * magnitude)
        }
    }
}

/// Parse a duration from a string.
///
/// ### Tests
///
/// ```
/// use reactor_rt::try_parse_duration;
/// use std::time::Duration;
///
/// assert_eq!(try_parse_duration("3ms"),    Ok(Duration::from_millis(3)));
/// assert_eq!(try_parse_duration("5us"),    Ok(Duration::from_micros(5)));
/// assert_eq!(try_parse_duration("30ns"),   Ok(Duration::from_nanos(30)));
/// assert_eq!(try_parse_duration("30nsec"), Ok(Duration::from_nanos(30)));
/// assert_eq!(try_parse_duration("30secs"), Ok(Duration::from_secs(30)));
/// // unit is not required for zero
/// assert_eq!(try_parse_duration("0"), Ok(Duration::from_secs(0)));
///
/// assert_eq!(try_parse_duration(""), Err("cannot parse empty string".into()));
/// assert_eq!(try_parse_duration("30"), Err("time unit required".into()));
/// assert_eq!(try_parse_duration("30000000000000000000000ns"), Err("number too large to fit in target type".into()));
///
/// ```
///
pub fn try_parse_duration(t: &str) -> Result<Duration, String> {
    // note: we parse this manually to avoid depending on regex
    let mut chars = t.char_indices().skip_while(|(_, c)| c.is_numeric());

    if let Some((num_end, _)) = &chars.next() {
        let magnitude: u64 = (&t)[0..*num_end].parse::<u64>().map_err(|e| format!("{}", e))?;

        let unit = &t[*num_end..];

        let duration = match TimeUnit::try_from(unit) {
            Ok(unit) => unit.to_duration(magnitude),
            Err(_) => return Err(format!("unknown time unit '{}'", unit))
        };
        Ok(duration)
    } else if t != "0" { // no unit
        if t.len() > 0 {
            Err("time unit required".into())
        } else {
            Err("cannot parse empty string".into())
        }
    } else {
        Ok(Duration::from_secs(0))
    }
}
