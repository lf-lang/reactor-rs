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

use crate::*;
#[allow(unused)]
use super::testutil::*;

#[test]
fn a_value_map_should_be_able_to_store_a_value() {
    let mut vmap = ValueMap::<i64>::default();
    let fut = LogicalInstant::now() + Duration::from_millis(500);
    assert_eq!(None, vmap.get_value(fut));
    vmap.schedule(fut, Some(2555));
    assert_eq!(Some(&2555), vmap.get_value(fut));
    assert_eq!(Some(&2555), vmap.get_value(fut));
}

#[test]
fn a_value_map_should_be_able_to_forget_a_value() {
    let mut vmap = ValueMap::<i64>::default();
    let fut = LogicalInstant::now() + Duration::from_millis(500);
    vmap.schedule(fut, Some(2555));
    assert_eq!(Some(&2555), vmap.get_value(fut));
    vmap.forget(fut);
    assert_eq!(None, vmap.get_value(fut));
}

#[test]
fn a_value_map_should_be_able_to_store_more_values() {
    let mut vmap = ValueMap::<i64>::default();
    let fut = LogicalInstant::now() + Duration::from_millis(500);
    let fut2 = LogicalInstant::now() + Duration::from_millis(540);
    let fut3 = LogicalInstant::now() + Duration::from_millis(560);

    vmap.schedule(fut, Some(1));
    // order is reversed on purpose
    vmap.schedule(fut3, Some(3));
    vmap.schedule(fut2, Some(2));

    assert_eq!(Some(&1), vmap.get_value(fut));
    assert_eq!(Some(&2), vmap.get_value(fut2));
    assert_eq!(Some(&3), vmap.get_value(fut3));

}
