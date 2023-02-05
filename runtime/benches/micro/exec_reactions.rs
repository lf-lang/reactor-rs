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

//! Compares using a HashMap vs ExecutableReactions to iterate over levels.
//! It's kicking down in a way, since for sparse levels, the HashMap impl
//! is disadvantaged as it queries the map for each level.
//! For test cases that aren't sparse, the comparison is fairer, and shows
//! that the constant factor in VecMap is much lower than that of HashMap,
//! for our use case.
//! Also note that the loop for ExecutableReactions is in theory
//! more efficient than the actual loop of process_tag, since
//! in process_tag, we might swap the ExecutableReactions instance,
//! and invalidate the KeyRef, which makes accessing the next
//! level linear. This happens often in actual reactor programs,
//! but I don't think it happens often that the new ExecutableReactions
//! instance has a lot of levels before the actual level we fetch,
//! so the linear constant factor should realistically be very very low.

#![allow(unused, non_snake_case, non_camel_case_types)]
#[macro_use]
extern crate reactor_rt;

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use reactor_rt::internals::{new_global_rid, ExecutableReactions, GlobalIdImpl, LevelIx, ReactionLevelInfo};
use reactor_rt::{GlobalReactionId, LocalReactionId};

fn iter_batches_hashmap(reactions: &HashMap<LevelIx, HashSet<GlobalReactionId>>) {
    let mut min_level = LevelIx::ZERO;
    let mut levels_explored = 0;
    while levels_explored < reactions.len() {
        if let Some(reactions) = reactions.get(&min_level) {
            levels_explored += 1;
            black_box(reactions);
        }
        min_level = min_level.next();
    }
}

fn iter_batches_executable_reactions(reactions: &ExecutableReactions) {
    let mut next_level = reactions.first_batch();
    while let Some((level_no, rs)) = next_level {
        black_box(rs);
        next_level = reactions.next_batch(level_no);
    }
}

pub fn r(u: u32) -> GlobalReactionId {
    new_global_rid(GlobalIdImpl::try_from(u).unwrap())
}

struct TestCase(&'static str, HashMap<LevelIx, HashSet<GlobalReactionId>>);

impl TestCase {
    fn get_exec(&self) -> ExecutableReactions {
        let mut result = ExecutableReactions::new();
        for (level, hset) in &self.1 {
            for r in hset {
                result.insert(*r, *level);
            }
        }
        result
    }
}

fn test_cases() -> Vec<TestCase> {
    vec![
        TestCase(
            "single",
            HashMap::from([(LevelIx::from(0), (0..10).into_iter().map(r).collect())]),
        ),
        TestCase(
            "sparse",
            HashMap::from([
                (LevelIx::from(0), (0..10).into_iter().map(r).collect()),
                (LevelIx::from(10), (0..10).into_iter().map(r).collect()),
            ]),
        ),
        TestCase(
            "wide-compact",
            // This is compact so the hashmap fun doesn't suffer from sparsity.
            // ExecutableReaction iteration should be 75 * sparse
            (0..150)
                .into_iter()
                .map(|i| (LevelIx::from(i), (0..10).into_iter().map(r).collect()))
                .collect(),
        ),
    ]
}

fn bench_gid(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExecutableReactions");
    for test in test_cases() {
        let executable = test.get_exec();
        let hashmap = &test.1;
        group.bench_with_input(BenchmarkId::new("iter/HashMap", test.0), hashmap, |b, i| {
            b.iter(|| iter_batches_hashmap(i))
        });
        group.bench_with_input(BenchmarkId::new("iter/VecMap", test.0), &executable, |b, i| {
            b.iter(|| iter_batches_executable_reactions(i))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_gid);
criterion_main!(benches);
