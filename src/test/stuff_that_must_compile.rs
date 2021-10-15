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

//! A collection of code that must remain compilable, even if it's functionally useless.
//! That code looks like what's generated by the code generator.

#![allow(unused)]

use crate::{AssemblyCtx, CleanupCtx, LogicalAction, PhysicalAction, PhysicalActionRef, Port, ReactionCtx, ReadablePort, WritablePort};
use crate::Offset::Asap;

fn actions_get(ctx: &mut ReactionCtx, act_mut: &mut LogicalAction<u32>, act: &LogicalAction<u32>) {
    assert!(ctx.get(act_mut).is_some());
    assert!(ctx.get(act_mut).is_some());
    assert!(ctx.get(act).is_some());
    assert!(ctx.get(act).is_some());
}

fn actions_use_ref_mut(ctx: &mut ReactionCtx, act: &mut LogicalAction<u32>) {
    // the duplication is useful here, we're testing that `act` is
    // not moved in the first statement, which would make the
    // second be uncompilable
    assert!(ctx.use_ref(act, |v| v.is_some()));
    assert!(ctx.use_ref(act, |v| v.is_some()));
}

fn actions_use_ref(ctx: &mut ReactionCtx, act: &LogicalAction<u32>) { // act is not &mut
    assert!(ctx.use_ref(act, |v| v.is_some()));
    assert!(ctx.use_ref(act, |v| v.is_some()));
}

fn port_get(ctx: &mut ReactionCtx, port: &ReadablePort<u32>) {
    assert!(ctx.get(port).is_some());
}

fn port_set(ctx: &mut ReactionCtx, mut port: WritablePort<u32>) {
    assert_eq!(ctx.set(port, 3), ());
}

fn physical_spawn_elided(ctx: &mut ReactionCtx, mut action: PhysicalActionRef<u32>) {
    use std::thread;

    let physical = ctx.spawn_physical_thread(move |link| {
        link.schedule_physical(&action, Asap)
    });
}

// fn port_is_send(ctx: &mut AssemblyCtx, port: Port<u32>) {
//     struct FooReactor {
//         port: Port<u32>,
//     }
//     let foo: &dyn Sync = &FooReactor { port };
// }

fn physical_action_ref_is_send(ctx: &mut AssemblyCtx, port: PhysicalActionRef<u32>) {
    let foo: &dyn Send = &port;
}

fn action_is_send<K: Sync>(ctx: &mut AssemblyCtx, action: LogicalAction<K>) {
    struct FooReactor<K: Sync> {
        action: LogicalAction<K>,
    }
    let foo: &dyn Sync = &FooReactor { action };
}

fn cleanup(
    ctx: &mut CleanupCtx,
    action: &mut LogicalAction<u32>,
    phys_action: &mut PhysicalActionRef<u32>,
    port: &mut Port<u32>
) {
    ctx.cleanup_logical_action(action);
    ctx.cleanup_physical_action(phys_action);
    ctx.cleanup_port(port);
}