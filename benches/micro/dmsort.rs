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

//! Compares sorting algorithms for sets of reaction ids.
//! This benchmark revealed that dmsort is less performant
//! than Vec::sort for typical sizes of reaction sets.

#![allow(unused, non_snake_case, non_camel_case_types)]
#[macro_use]
extern crate reactor_rt;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use reactor_rt::internals::*;
use reactor_rt::GlobalReactionId;

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct GID_raw {
    i: GlobalIdImpl,
}

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct GID_split {
    a: ReactorIdImpl,
    b: ReactionIdImpl,
}

#[derive(Eq, PartialEq, Copy, Clone)]
struct GID_split_custom_h(GID_split);

impl Hash for GID_split_custom_h {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let as_impl: &GlobalIdImpl = unsafe { std::mem::transmute(self) };
        Hash::hash(as_impl, state);
    }
}

fn dmsort_sort(mut up: Vec<GlobalReactionId>) {
    dmsort::sort(&mut up);
    black_box(up);
}
fn vec_sort(mut up: Vec<GlobalReactionId>) {
    up.sort();
    black_box(up);
}

pub fn r(u: u32) -> GlobalReactionId {
    new_global_rid(GlobalIdImpl::try_from(u).unwrap())
}

struct TestCase(&'static str, Vec<GlobalReactionId>);

fn bench_gid(c: &mut Criterion) {
    let mut group = c.benchmark_group("Global ID implementation");
    let mut large = (0..40).map(r).collect::<Vec<_>>();
    large[24] = r(65); // random misplaced element

    let test_cases = vec![
        TestCase("single", vec![r(0)]),
        TestCase("twosorted", vec![r(0), r(1)]),
        TestCase("insertionintwo", vec![r(0), r(5), r(1)]),
        TestCase("large", large),
    ];
    for test in test_cases.into_iter() {
        group.bench_with_input(BenchmarkId::new("dmsort", test.0), &test.1, |b, i| {
            b.iter(|| dmsort_sort(i.clone()))
        });
        group.bench_with_input(BenchmarkId::new("Vec::sort", test.0), &test.1, |b, i| {
            b.iter(|| vec_sort(i.clone()))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_gid);
criterion_main!(benches);
