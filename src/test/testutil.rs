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

//! Test utilities.

use std::sync::Arc;

use crate::*;
use crate::Output;

/// Set a port to a value
pub fn set_port<T>(port: &mut OutputPort<T>, v: T) {
    port.set_impl(v, |_| {})
}

fn make_deps(container: ReactorId, ids: Vec<u32>) -> ReactionSet {
    let mut result = Vec::new();
    for id in ids.iter() {
        let r = ReactionInvoker::new_from_closure(container, *id, |_| {});
        result.push(Arc::new(r));
    }
    result
}

/// Set the given port's downstream dependencies as a set of
/// fake reactions whose ids are exactly the given `local_ids`,
/// taken to represent reactions of the given reactor.
pub fn set_fake_downstream<T, K>(container: ReactorId, ids: Vec<u32>, port: &mut Port<T, K>) {
    port.set_downstream(make_deps(container, ids))
}

/// Assert that the given port's recorded downstream dependencies
/// have exactly the ids contained in the given `local_ids`,
/// taken to represent reactions of the given reactor.
pub fn assert_deps_eq<T>(container: ReactorId, local_ids: Vec<u32>, port: &Port<T, Output>) {
    let expected =
        local_ids.into_iter()
            .map(|loc| container.make_reaction_id(loc))
            .collect::<Vec<_>>();

    let actual =
        port.get_downstream_deps().iter()
            .map(|r| r.id())
            .collect::<Vec<_>>();


    assert_eq!(expected, actual, "Reaction IDs do not match");
}
