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

use super::testutil::*;

static mut DUMMY_ID: GlobalId = GlobalId::first_id();


fn new_port() -> Port<i32> {
    let id = unsafe {
        let i = DUMMY_ID;
        DUMMY_ID = DUMMY_ID.next_id();
        i
    };

    Port::<i32>::new(id, true)
}

#[test]
fn a_port_is_initially_empty() {
    let port = new_port();
    assert_eq!(None, port.get()); // default value?
}

#[test]
fn binding_two_ports_should_let_values_be_read() {
    let mut upstream = new_port();
    let mut downstream = new_port();

    assert_eq!(None, downstream.get());

    bind_ports(&mut upstream, &mut downstream).unwrap();

    assert_eq!(None, downstream.get());

    set_port(&mut upstream, 5);

    assert_eq!(Some(5), downstream.get());
}

#[test]
fn a_port_can_be_upstream_of_several_ports() {
    let mut upstream = new_port();
    let mut d1 = new_port();
    let mut d2 = new_port();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    bind_ports(&mut upstream, &mut d1).unwrap();
    bind_ports(&mut upstream, &mut d2).unwrap();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    set_port(&mut upstream, 5);

    assert_eq!(Some(5), d1.get());
    assert_eq!(Some(5), d2.get());

    set_port(&mut upstream, 6);

    assert_eq!(Some(6), d1.get());
    assert_eq!(Some(6), d2.get());
}

#[test]
fn transitive_binding_should_let_values_flow() {
    let mut upstream = new_port();
    let mut d1 = new_port();
    let mut d2 = new_port();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    // upstream -> d1 -> d2
    bind_ports(&mut upstream, &mut d1).unwrap();
    bind_ports(&mut d1, &mut d2).unwrap();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    set_port(&mut upstream, 5);

    assert_eq!(Some(5), d1.get());
    assert_eq!(Some(5), d2.get());

    set_port(&mut upstream, 6);

    assert_eq!(Some(6), d1.get());
    assert_eq!(Some(6), d2.get());
}


#[test]
fn transitive_binding_in_topo_order_is_ok() {
    let mut upstream = new_port();
    let mut d1 = new_port();
    let mut d2 = new_port();
    let mut b1 = new_port();
    let mut b2 = new_port();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    // upstream -> d1 -> d2 -> b1
    //                   d2 -> b2

    // Note that linking the ports the other way around doesn't
    // work, we need to go in topo order

    bind_ports(&mut upstream, &mut d1).unwrap();

    bind_ports(&mut d1, &mut d2).unwrap();

    bind_ports(&mut d2, &mut b1).unwrap();
    bind_ports(&mut d2, &mut b2).unwrap();


    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    set_port(&mut upstream, 5);

    assert_eq!(Some(5), d1.get());
    assert_eq!(Some(5), d2.get());
    assert_eq!(Some(5), b1.get());
    assert_eq!(Some(5), b2.get());

    set_port(&mut upstream, 6);

    assert_eq!(Some(6), d1.get());
    assert_eq!(Some(6), d2.get());
    assert_eq!(Some(6), b2.get());
    assert_eq!(Some(6), b1.get());
}

#[test]
fn transitive_binding_in_non_topo_order_is_ok() {
    let mut a = new_port();
    let mut b = new_port();
    let mut c = new_port();

    assert_matches!(bind_ports(&mut b, &mut c), Ok(_));
    assert_matches!(bind_ports(&mut a, &mut b), Ok(_));

    assert_eq!(None, b.get());
    assert_eq!(None, c.get());

    set_port(&mut a, 1);

    assert_eq!(Some(1), b.get());
    assert_eq!(Some(1), c.get());
}


#[test]
fn repeated_binding_panics() {
    let mut upstream = new_port();
    let mut downstream = new_port();

    assert_matches!(bind_ports(&mut upstream, &mut downstream), Ok(_));
    assert_matches!(bind_ports(&mut upstream, &mut downstream), Err(AssemblyError(AssemblyErrorImpl::CannotBind(_, _))));
}
