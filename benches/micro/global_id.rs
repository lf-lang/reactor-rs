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

//! Compares the implementation of global ID based on u32
//! vs a struct with two u16.

#![allow(unused, non_snake_case, non_camel_case_types)]
#![feature(bench_black_box)]
#[macro_use]
extern crate reactor_rt;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

type GID_impl = u64;
type HalfGID = u32;
#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct GID_raw {
    i: GID_impl,
}

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
struct GID_split {
    a: HalfGID,
    b: HalfGID,
}

#[derive(Eq, PartialEq, Copy, Clone)]
struct GID_split_custom_h(GID_split);

impl Hash for GID_split_custom_h {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let as_impl: &GID_impl = unsafe { std::mem::transmute(self) };
        Hash::hash(as_impl, state);
    }
}

fn gid_clone(up: GID_impl) -> HashMap<GID_raw, GID_raw> {
    let mut x = black_box(HashMap::<GID_raw, GID_raw>::new());
    for i in 0..up {
        x.entry(GID_raw { i }).or_insert(GID_raw { i });
    }
    black_box(x)
}

fn gid_clone_split(up: GID_impl) -> HashMap<GID_split, GID_split> {
    let mut x = black_box(HashMap::<GID_split, GID_split>::new());
    for i in 0..up {
        x.entry(split_u32(i)).or_insert(split_u32(i + 1));
    }
    black_box(x)
}

fn gid_clone_split_custom_h(up: GID_impl) -> HashMap<GID_split_custom_h, GID_split_custom_h> {
    let mut x = black_box(HashMap::<GID_split_custom_h, GID_split_custom_h>::new());
    for i in 0..up {
        x.entry(GID_split_custom_h(split_u32(i)))
            .or_insert(GID_split_custom_h(split_u32(i + 1)));
    }
    black_box(x)
}

fn bench_gid(c: &mut Criterion) {
    let mut group = c.benchmark_group("Global ID implementation");
    for i in [1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("Raw u32", i), i, |b, i| b.iter(|| gid_clone(*i)));
        group.bench_with_input(BenchmarkId::new("Struct ", i), i, |b, i| b.iter(|| gid_clone_split(*i)));
        group.bench_with_input(BenchmarkId::new("Struct custom h", i), i, |b, i| {
            b.iter(|| gid_clone_split_custom_h(*i))
        });
    }
    group.finish();
}

fn split_u32(i: GID_impl) -> GID_split {
    GID_split { a: (i >> 16) as HalfGID, b: i as HalfGID }
}

criterion_group!(benches, bench_gid);
criterion_main!(benches);
