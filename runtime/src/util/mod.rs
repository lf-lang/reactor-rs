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
    ($f:expr, $iter:expr) => {
        join_to!($f, $iter, ", ")
    };
    ($f:expr, $iter:expr, $sep:literal) => {
        join_to!($f, $iter, $sep, "", "")
    };
    ($f:expr, $iter:expr, $sep:literal, $prefix:literal, $suffix:literal) => {
        join_to!($f, $iter, $sep, $prefix, $suffix, |x| format!("{}", x))
    };
    ($f:expr, $iter:expr, $sep:literal, $prefix:literal, $suffix:literal, $display:expr) => {{
        $crate::util::do_write($f, $iter, $sep, $prefix, $suffix, $display)
    }};
}

pub(crate) fn do_write<X>(
    f: &mut impl std::fmt::Write,
    iter: impl Iterator<Item = X>,
    sep: &'static str,
    prefix: &'static str,
    suffix: &'static str,
    formatter: impl Fn(X) -> String,
) -> std::fmt::Result {
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

/// Shorthand for using [After](crate::Offset::After) together with [delay].
///
/// ```
/// use std::time::Duration;
/// use reactor_rt::{after, Offset::After};
///
/// assert_eq!(after!(10 ns), After(Duration::from_nanos(10)));
/// assert_eq!(after!(2 min), After(Duration::from_secs(120)));
/// ```
#[macro_export]
macro_rules! after {
    ($amount:tt $unit:tt) => { $crate::Offset::After($crate::delay!($amount $unit)) }
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
/// assert_eq!(delay!(0), Duration::from_secs(0));
///
/// let x = 2;
/// assert_eq!(delay!(x min), delay!(120 s));
/// assert_eq!(delay!((1+2) min), delay!(180 s));
///
/// // more verbose aliases
/// assert_eq!(delay!(2 min), delay!(2 minutes));
/// assert_eq!(delay!(2 h), delay!(2 hours));
/// assert_eq!(delay!(1 week), delay!(7 days));
///
/// ```
#[macro_export]
macro_rules! delay {
    (0)                   => { $crate::Duration::from_nanos(0) };
    ($amount:tt ns)       => { $crate::Duration::from_nanos($amount) };
    ($amount:tt nsec)     => { delay!($amount ns) };
    ($amount:tt nsecs)    => { delay!($amount ns) };
    ($amount:tt us)       => { $crate::Duration::from_micros($amount) };
    ($amount:tt usec)     => { delay!($amount us) };
    ($amount:tt usecs)    => { delay!($amount us) };
    ($amount:tt ms)       => { $crate::Duration::from_millis($amount) };
    ($amount:tt msec)     => { delay!($amount ms) };
    ($amount:tt msecs)    => { delay!($amount ms) };
    ($amount:tt s)        => { $crate::Duration::from_secs($amount) };
    ($amount:tt sec)      => { delay!($amount s) };
    ($amount:tt secs)     => { delay!($amount s) };
    ($amount:tt second)   => { delay!($amount s) };
    ($amount:tt seconds)  => { delay!($amount s) };
    ($amount:tt min)      => { $crate::Duration::from_secs(60 * $amount) };
    ($amount:tt mins)     => { delay!($amount min) };
    ($amount:tt minute)   => { delay!($amount min) };
    ($amount:tt minutes)  => { delay!($amount min) };
    ($amount:tt h)        => { delay!((3600 * $amount) s) };
    ($amount:tt hour)     => { delay!($amount h) };
    ($amount:tt hours)    => { delay!($amount h) };
    ($amount:tt d)        => { delay!((24*$amount) h) };
    ($amount:tt day)      => { delay!($amount d) };
    ($amount:tt days)     => { delay!($amount d) };
    ($amount:tt week)     => { delay!((7*$amount) d) };
    ($amount:tt weeks)    => { delay!($amount week) };
    ($amount:tt $i:ident) => { compile_error!(concat!("Unknown time unit `", stringify!($i), "`")) };
}

/// Convenient macro to assert equality of the current tag.
/// This is just shorthand for using `assert_eq!` with the
/// syntax of [tag].
///
/// ```no_run
/// # use reactor_rt::{assert_tag_is, delay, ReactionCtx};
/// # let ctx : ReactionCtx = unimplemented!();
/// # struct Foo { i: u32 }
/// # let foo = Foo { i: 0 };
///
/// assert_tag_is!(ctx, T0 + 20 ms);
/// assert_tag_is!(ctx, T0 + 60 ms);
/// assert_tag_is!(ctx, T0);
/// // with a microstep, add parentheses
/// assert_tag_is!(ctx, (T0, 1));
/// assert_tag_is!(ctx, (T0 + 3 sec, 1));
/// assert_tag_is!(ctx, (T0 + 3 sec, foo.i));
/// ```
#[macro_export]
macro_rules! assert_tag_is {
    ($ctx:tt, T0)                          => {assert_tag_is!($ctx, (T0 + 0 sec, 0))};
    ($ctx:tt, (T0, $microstep:expr))       => {assert_tag_is!($ctx, (T0 + 0 sec, $microstep))};
    ($ctx:tt, T0 + $amount:tt $unit:ident) => {assert_tag_is!($ctx, (T0 + $amount $unit, 0))};
    ($ctx:tt, (T0 + $amount:tt $unit:ident, $microstep:expr)) => {
        assert_eq!(
            $crate::tag!(T0 + $amount $unit, $microstep),
            $ctx.get_tag()
        )
    };
}

/// Convenient macro to [create a tag](crate::EventTag).
/// This is just a shorthand for using the constructor together
/// with the syntax of [delay].
///
/// ```no_run
/// use reactor_rt::{tag, delay};
///
/// tag!(T0 + 20 ms);
/// tag!(T0 + 60 ms);
/// tag!(T0); // the origin tag
/// // with a microstep:
/// tag!(T0, 1);
/// tag!(T0 + 3 sec, 1);
/// ```
#[macro_export]
macro_rules! tag {
    (T0)                          => {$crate::EventTag::ORIGIN};
    (T0, $microstep:expr)         => {tag!(T0 + 0 sec, $microstep)};
    (T0 + $amount:tt $unit:ident) => {tag!(T0 + $amount $unit, 0)};
    (T0 + $amount:tt $unit:ident, $microstep:expr) => {
        $crate::EventTag::offset($crate::delay!($amount $unit), $microstep)
    };
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
            _ => return Err(()),
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
            TimeUnit::DAY => Duration::from_secs(60 * 60 * 24 * magnitude),
        }
    }
}

/// Parse a duration from a string. This is used for CLI
/// parameter parsing in programs generated by LFC, specifically,
/// to parse main parameters with `time` type, and scheduler
/// options with time type.
///
/// ### Tests
///
/// ```
/// use reactor_rt::try_parse_duration;
/// use std::time::Duration;
///
/// assert_eq!(try_parse_duration("3 ms"),   Ok(Duration::from_millis(3)));
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
        let magnitude: u64 = t[0..*num_end].parse::<u64>().map_err(|e| format!("{}", e))?;

        let unit = t[*num_end..].trim();

        let duration = match TimeUnit::try_from(unit) {
            Ok(unit) => unit.to_duration(magnitude),
            Err(_) => return Err(format!("unknown time unit '{}'", unit)),
        };
        Ok(duration)
    } else if t != "0" {
        // no unit
        if !t.is_empty() {
            Err("time unit required".into())
        } else {
            Err("cannot parse empty string".into())
        }
    } else {
        Ok(Duration::from_secs(0))
    }
}
