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
//! This benchmark verified that the hashing function provided
//! by #\[derive(Hash)] is slower than transmuting to u32 and
//! writing that into the hasher.
//! With --features wide-ids you can run the same benchmark
//! with ids that are twice as wide, if your machine supports
//! it.

#![allow(unused, non_snake_case, non_camel_case_types)]
#[macro_use]
extern crate reactor_rt;

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use reactor_rt::internals::*;

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

fn gid_clone(up: GlobalIdImpl) -> HashMap<GID_raw, GID_raw> {
    let mut x = HashMap::<GID_raw, GID_raw>::new();
    for i in 0..up {
        let i = i as GlobalIdImpl;
        x.entry(GID_raw { i }).or_insert(GID_raw { i });
    }
    x
}

fn gid_clone_split(up: GlobalIdImpl) -> HashMap<GID_split, GID_split> {
    let mut x = HashMap::<GID_split, GID_split>::new();
    for i in 0..up {
        let i = i as GlobalIdImpl;
        x.entry(split_u32(i)).or_insert(split_u32(i + 1));
    }
    x
}

fn gid_clone_split_custom_h(up: GlobalIdImpl) -> HashMap<GID_split_custom_h, GID_split_custom_h> {
    let mut x = HashMap::<GID_split_custom_h, GID_split_custom_h>::new();
    for i in 0..up {
        let i = i as GlobalIdImpl;
        x.entry(GID_split_custom_h(split_u32(i)))
            .or_insert(GID_split_custom_h(split_u32(i + 1)));
    }
    x
}

fn bench_gid(c: &mut Criterion) {
    let mut group = c.benchmark_group("Global ID implementation");
    for i in [1000, 10000].iter() {
        let i = &(*i as GlobalIdImpl);
        group.bench_with_input(BenchmarkId::new("Raw u32", i), i, |b, i| b.iter(|| gid_clone(*i)));
        group.bench_with_input(BenchmarkId::new("Struct ", i), i, |b, i| b.iter(|| gid_clone_split(*i)));
        group.bench_with_input(BenchmarkId::new("Struct custom h", i), i, |b, i| {
            b.iter(|| gid_clone_split_custom_h(*i))
        });
    }
    group.finish();
}

fn split_u32(i: GlobalIdImpl) -> GID_split {
    GID_split {
        a: (i >> ReactionIdImpl::BITS) as ReactorIdImpl,
        b: i as ReactionIdImpl,
    }
}

criterion_group!(benches, bench_gid);
criterion_main!(benches);
