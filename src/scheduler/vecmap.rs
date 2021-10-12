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

use std::fmt::{Debug, Formatter};

/// Mostly copied from https://crates.io/crates/vec_map
///
#[derive(Default)]
pub struct VecMap<V> {
    v: Vec<Option<V>>,
    /// Number of non-None values
    size: usize,
}

#[allow(unused)]
impl<V> VecMap<V> {
    pub fn new() -> Self {
        Self { v: Vec::new(), size: 0 }
    }

    pub fn reserve_len(&mut self, len: usize) {
        let cur_len = self.v.len();
        if len >= cur_len {
            self.v.reserve(len - cur_len);
        }
    }

    fn trim(&mut self) {
        if let Some(idx) = self.v.iter().rposition(Option::is_some) {
            self.v.truncate(idx + 1);
        } else {
            self.v.clear();
        }
    }

    pub fn get(&self, key: usize) -> Option<&V> {
        if key < self.v.len() {
            self.v[key].as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, key: usize) -> Option<&mut V> {
        if key < self.v.len() {
            self.v[key].as_mut()
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: usize, value: V) -> Option<V> {
        let len = self.v.len();
        if len <= key {
            self.v.extend((0..key - len + 1).map(|_| None));
        }
        let was = std::mem::replace(&mut self.v[key], Some(value));
        if was.is_none() {
            self.size += 1;
        }
        was
    }

    pub fn remove(&mut self, key: usize) -> Option<V> {
        if key >= self.v.len() {
            return None;
        }
        let result = &mut self.v[key];
        let was = result.take();
        if was.is_some() {
            self.size -= 1;
        }
        self.trim(); // remove trailing None
        was
    }

    pub fn iter_from(&self, min_key: usize) -> impl Iterator<Item=(usize, &V)> + '_ {
        self.v.iter().enumerate().skip(min_key).filter_map(|(k, v)| v.as_ref().map(|v| (k, v)))
    }

    pub fn capacity(&self) -> usize {
        self.v.capacity()
    }

    pub fn max_key(&self) -> usize {
        self.v.len()
    }
}

impl<V: Clone> Clone for VecMap<V> {
    fn clone(&self) -> Self {
        VecMap {
            v: self.v.clone(),
            size: self.size,
        }
    }
}


impl<V: Debug> Debug for VecMap<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.iter_from(0).collect::<Vec<(usize, &V)>>().fmt(f)
    }
}
