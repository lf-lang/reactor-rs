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

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use crate::Duration;


/// A type whose instances have statically known names
pub trait Named {
    fn name(&self) -> &'static str;
}

/// A type that can list all its instances
pub trait Enumerated {
    fn list() -> Vec<Self> where Self: Sized;
}


/// A type with no instances.
/// Rust's bottom type, `!`, is experimental
pub enum Nothing {}

impl PartialEq for Nothing {
    fn eq(&self, _: &Self) -> bool {
        unreachable!("No instance of Nothing type")
    }
}

impl Clone for Nothing {
    fn clone(&self) -> Self {
        unreachable!("No instance of Nothing type")
    }
}

impl Copy for Nothing {}

impl Eq for Nothing {}

impl Hash for Nothing {
    fn hash<H: Hasher>(&self, _: &mut H) {
        unreachable!("No instance of Nothing type")
    }
}

impl PartialOrd for Nothing {
    fn partial_cmp(&self, _: &Self) -> Option<Ordering> {
        unreachable!("No instance of Nothing type")
    }
}

impl Ord for Nothing {
    fn cmp(&self, _: &Self) -> Ordering {
        unreachable!("No instance of Nothing type")
    }
}

impl Named for Nothing {
    fn name(&self) -> &'static str {
        unreachable!("No instance of Nothing type")
    }
}

impl Enumerated for Nothing {
    fn list() -> Vec<Self> where Self: Sized {
        vec![]
    }
}

/// Duration::zero() is unstable
pub const ZERO_DURATION: Duration = Duration::from_millis(0);
