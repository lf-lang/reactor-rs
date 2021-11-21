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

/// A sparse map representation over a totally ordered key type.
///
/// Used in [crate::ExecutableReactions]
///
pub struct VecMap<K, V>
where
    K: Eq + Ord,
{
    v: Vec<(K, V)>,
}

impl<K, V> VecMap<K, V>
where
    K: Eq + Ord,
{
    pub fn new() -> Self {
        Self { v: Vec::new() }
    }

    pub fn entry(&mut self, key: K) -> Entry<K, V> {
        match self.find_k(&key) {
            Ok(index) => Entry::Occupied(key, &mut self.v[index].1),
            Err(index) => Entry::Vacant(VacantEntry { map: self, index, key }),
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.find_k(&key) {
            Ok(index) => Some(std::mem::replace(&mut self.v[index].1, value)),
            Err(index) => {
                self.v.insert(index, (key, value));
                None
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        match self.find_k(key) {
            Ok(index) => Some(self.v.remove(index).1),
            Err(_) => None,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        match self.find_k(key) {
            Ok(index) => Some(&self.v[index].1),
            Err(_) => None,
        }
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.find_k(key).is_ok()
    }

    /// Produces the first mapping that follows the given key
    /// in the ascending order on keys.
    /// This function expects an exact keyref, which is only
    /// checked in debug mode.
    ///
    /// Note that the keyref must have been produced by a
    /// VecMap with the same internal structure.
    pub fn next_mapping(&self, key: KeyRef<&K>) -> Option<(KeyRef<&K>, &V)> {
        debug_assert!(key.key == &self.v[key.min_idx].0, "Expecting an exact keyref");
        let idx = key.min_idx + 1;
        self.v.get(idx).map(move |(key, v)| (KeyRef { min_idx: idx, key }, v))
    }

    fn check_valid_keyref(&self, key: &KeyRef<&K>) {
        let from = key.min_idx;
        if cfg!(debug_assertions) {
            assert!(from == 0 || from < self.v.len(), "KeyRef is invalid for this vecmap");
            if let Some((k, _)) = self.v.get(from) {
                assert!(k <= key.key, "KeyRef is invalid for this vecmap");
            }
        }
    }

    pub fn iter_from<'a>(&'a self, min_key: KeyRef<&'a K>) -> impl Iterator<Item = (KeyRef<&'a K>, &V)> + 'a {
        self.check_valid_keyref(&min_key);
        let from = min_key.min_idx;

        self.v[from..]
            .iter()
            .enumerate()
            .skip_while(move |(_, (k, _))| k < &min_key.key)
            .map(move |(idx, (k, v))| (KeyRef { min_idx: from + idx, key: k }, v))
    }

    pub fn iter(&self) -> impl Iterator<Item = &(K, V)> + '_ {
        self.v.iter()
    }

    pub fn min_entry(&self) -> Option<(KeyRef<&K>, &V)> {
        self.v.first().map(|(key, v)| (KeyRef { key, min_idx: 0 }, v))
    }

    pub fn max_key(&self) -> Option<&K> {
        self.v.last().map(|e| &e.0)
    }

    fn find_k(&self, key: &K) -> Result<usize, usize> {
        self.v.binary_search_by_key(&key, |(k, _)| k)
    }

    fn insert_internal(&mut self, idx: usize, key: K, value: V) {
        self.v.insert(idx, (key, value))
    }
}

impl<K: Clone + Eq + Ord, V: Clone> Clone for VecMap<K, V> {
    fn clone(&self) -> Self {
        VecMap { v: self.v.clone() }
    }
}

impl<K: Ord + Eq + Debug, V: Debug> Debug for VecMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.v.iter().collect::<Vec<&(K, V)>>().fmt(f)
    }
}

impl<K: Ord + Eq, V> Default for VecMap<K, V> {
    fn default() -> Self {
        Self { v: Vec::new() }
    }
}

/// A view into a single entry in a map, which may either be vacant or occupied.
pub enum Entry<'a, K, V>
where
    K: Ord + Eq,
{
    /// A vacant Entry
    Vacant(VacantEntry<'a, K, V>),

    /// An occupied Entry
    Occupied(K, &'a mut V),
}

/// A vacant Entry.
pub struct VacantEntry<'a, K, V>
where
    K: Ord + Eq,
{
    map: &'a mut VecMap<K, V>,
    key: K,
    index: usize,
}

impl<K, V> VacantEntry<'_, K, V>
where
    K: Ord + Eq,
{
    /// Sets the value of the entry with the VacantEntry's key,
    /// and returns a mutable reference to it.
    pub fn insert(self, value: V) {
        let index = self.index;
        self.map.insert_internal(index, self.key, value)
    }
}

/// A key zipped with its internal index in this map.
/// For some operations, like manually implemented iteration,
/// the index can be used for optimisation.
#[derive(Copy, Clone)]
pub struct KeyRef<K> {
    pub key: K,
    /// This is a lower bound on the actual index of key K,
    /// it doesn't need to be the index (though it usually will be).
    min_idx: usize,
}

impl<K: Clone> KeyRef<&K> {
    #[inline]
    pub fn cloned(self) -> KeyRef<K> {
        KeyRef { min_idx: self.min_idx, key: self.key.clone() }
    }
}

impl<K> KeyRef<K> {
    #[inline]
    pub fn as_ref(&self) -> KeyRef<&K> {
        KeyRef { min_idx: self.min_idx, key: &self.key }
    }
}

impl<K: Ord> KeyRef<K> {
    pub fn next(self, key: K) -> KeyRef<K> {
        debug_assert!(key > self.key);
        KeyRef { min_idx: self.min_idx, key }
    }
}

impl<K: Display> Display for KeyRef<K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
    }
}

impl<K> From<K> for KeyRef<K> {
    fn from(key: K) -> Self {
        Self { key, min_idx: 0 }
    }
}
