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


#[macro_export]
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

