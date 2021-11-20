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


#![allow(unused, non_snake_case, non_camel_case_types)]
#[macro_use]
extern crate reactor_rt;

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use reactor_rt::{GlobalReactionId, LocalReactionId};
use reactor_rt::internals::{ExecutableReactions, ReactionLevelInfo, new_global_rid, GlobalIdImpl};
use reactor_rt::internals::LevelIx;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, black_box};

fn iter_batches_hashmap(reactions: &HashMap<LevelIx, HashSet<GlobalReactionId>>) {
    let mut min_level = LevelIx::ZERO;
    let mut levels_explored = 0;
    while levels_explored < reactions.len() {
        if let Some(reactions) = reactions.get(&min_level) {
            levels_explored += 1;
            for n in reactions {  //* \label{process_tag:inner_loop}
                black_box(n);
            }
        }
        min_level = min_level.next();
    }
}

fn iter_batches_executable_reactions(reactions: &ExecutableReactions) {
    let mut min_level = LevelIx::ZERO;
    while let Some((level_no, reactions)) = reactions.next_batch(min_level) {
        min_level = level_no.next();
        for n in reactions {  //* \label{process_tag:inner_loop}
            black_box(n);
        }
    }
}






pub fn r(u: u32) -> GlobalReactionId {
    new_global_rid(GlobalIdImpl::from(u))
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
            HashMap::from([
                (LevelIx::from(0), (0..10).into_iter().map(r).collect())
            ]),
        ), TestCase(
            "sparse",
            HashMap::from([
                (LevelIx::from(0), (0..10).into_iter().map(r).collect()),
                (LevelIx::from(10), (0..10).into_iter().map(r).collect()),
            ]),
        )
    ]
}

fn bench_gid(c: &mut Criterion) {
    let mut group = c.benchmark_group("ExecutableReactions");
    for test in test_cases() {
        let executable = test.get_exec();
        let hashmap = &test.1;
        // let level_fun: ReactionLevelInfo = todo!();
        // let set: HashSet<GlobalReactionId> = todo!();
        group.bench_with_input(BenchmarkId::new("iter/HashMap", test.0), hashmap, |b, i| b.iter(|| iter_batches_hashmap(i)));
        group.bench_with_input(BenchmarkId::new("iter/VecMap", test.0), &executable, |b, i| b.iter(|| iter_batches_executable_reactions(i)));
    }
    group.finish();
}


criterion_group!(benches, bench_gid);
criterion_main!(benches);
