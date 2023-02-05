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

use std::borrow::Cow;

use crate::assembly::{PortKind, TriggerId};
use crate::*;

struct TestAssembler {
    debug: DebugInfoRegistry,
    cur_id: TriggerId,
    reactor_id: ReactorId,
}

impl Default for TestAssembler {
    fn default() -> Self {
        Self {
            debug: DebugInfoRegistry::new(),
            cur_id: TriggerId::FIRST_REGULAR,
            reactor_id: ReactorId::new(0),
        }
    }
}

impl TestAssembler {
    pub fn new_port<T: Sync>(&mut self, name: &'static str) -> Port<T> {
        let id = self.cur_id.get_and_incr().unwrap();
        self.debug.record_trigger(id, Cow::Borrowed(name));
        Port::new(id, PortKind::Input)
    }

    fn ready(mut self) -> TestFixture {
        self.debug
            .set_id_range(self.reactor_id, TriggerId::FIRST_REGULAR..self.cur_id);
        self.debug.record_reactor(self.reactor_id, ReactorDebugInfo::test());
        TestFixture { debug: self.debug }
    }
}

struct TestFixture {
    debug: DebugInfoRegistry,
}

type TestResult = Result<(), String>;

impl TestFixture {
    pub fn bind<T: Sync>(&self, upstream: &mut Port<T>, downstream: &mut Port<T>) -> TestResult {
        upstream.forward_to(downstream).map_err(|e| e.lift(&self.debug))
    }

    pub fn set<T: Sync>(&self, port: &mut Port<T>, value: T) -> TestResult {
        port.set_impl(Some(value));
        Ok(())
    }

    pub fn ok(self) -> TestResult {
        Ok(())
    }
}

#[test]
fn a_port_is_initially_empty() -> TestResult {
    let mut test = TestAssembler::default();
    let port = test.new_port::<i32>("p");
    let test = test.ready();
    assert_eq!(None, port.get()); // default value?

    test.ok()
}

#[test]
fn binding_two_ports_should_let_values_be_read() -> TestResult {
    let mut test = TestAssembler::default();
    let mut upstream = test.new_port("up");
    let mut downstream = test.new_port("down");
    let test = test.ready();

    assert_eq!(None, downstream.get());

    test.bind(&mut upstream, &mut downstream)?;

    assert_eq!(None, downstream.get());

    test.set(&mut upstream, 5)?;

    assert_eq!(Some(5), downstream.get());

    test.ok()
}

#[test]
fn a_port_can_be_upstream_of_several_ports() -> TestResult {
    let mut test = TestAssembler::default();
    let mut upstream = test.new_port("up");
    let mut d1 = test.new_port("d1");
    let mut d2 = test.new_port("d2");
    let test = test.ready();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    test.bind(&mut upstream, &mut d1)?;
    test.bind(&mut upstream, &mut d2)?;

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    test.set(&mut upstream, 5)?;

    assert_eq!(Some(5), d1.get());
    assert_eq!(Some(5), d2.get());

    test.set(&mut upstream, 6)?;

    assert_eq!(Some(6), d1.get());
    assert_eq!(Some(6), d2.get());

    test.ok()
}

#[test]
fn transitive_binding_should_let_values_flow() -> TestResult {
    let mut test = TestAssembler::default();
    let mut upstream = test.new_port("up");
    let mut d1 = test.new_port("d1");
    let mut d2 = test.new_port("d2");
    let test = test.ready();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    // upstream -> d1 -> d2
    test.bind(&mut upstream, &mut d1)?;
    test.bind(&mut d1, &mut d2)?;

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    test.set(&mut upstream, 5)?;

    assert_eq!(Some(5), d1.get());
    assert_eq!(Some(5), d2.get());

    test.set(&mut upstream, 6)?;

    assert_eq!(Some(6), d1.get());
    assert_eq!(Some(6), d2.get());
    test.ok()
}

#[test]
fn transitive_binding_in_topo_order_is_ok() -> TestResult {
    let mut test = TestAssembler::default();
    let mut upstream = test.new_port("up");
    let mut d1 = test.new_port("d1");
    let mut d2 = test.new_port("d2");
    let mut b1 = test.new_port("b1");
    let mut b2 = test.new_port("b2");
    let test = test.ready();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    // upstream -> d1 -> d2 -> b1
    //                   d2 -> b2

    // Note that linking the ports the other way around doesn't
    // work, we need to go in topo order

    test.bind(&mut upstream, &mut d1)?;

    test.bind(&mut d1, &mut d2)?;

    test.bind(&mut d2, &mut b1)?;
    test.bind(&mut d2, &mut b2)?;

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    test.set(&mut upstream, 5)?;

    assert_eq!(Some(5), d1.get());
    assert_eq!(Some(5), d2.get());
    assert_eq!(Some(5), b1.get());
    assert_eq!(Some(5), b2.get());

    test.set(&mut upstream, 6)?;

    assert_eq!(Some(6), d1.get());
    assert_eq!(Some(6), d2.get());
    assert_eq!(Some(6), b2.get());
    assert_eq!(Some(6), b1.get());

    test.ok()
}

#[test]
fn transitive_binding_in_non_topo_order_is_ok() -> TestResult {
    let mut test = TestAssembler::default();
    let mut a = test.new_port("a");
    let mut b = test.new_port("b");
    let mut c = test.new_port("c");
    let test = test.ready();

    test.bind(&mut b, &mut c)?;
    test.bind(&mut a, &mut b)?;

    assert_eq!(None, b.get());
    assert_eq!(None, c.get());

    test.set(&mut a, 1)?;

    assert_eq!(Some(1), b.get());
    assert_eq!(Some(1), c.get());

    test.ok()
}

#[test]
fn repeated_binding_panics() -> TestResult {
    let mut test = TestAssembler::default();
    let mut upstream: Port<u32> = test.new_port("up");
    let mut downstream = test.new_port("down");
    let test = test.ready();

    test.bind(&mut upstream, &mut downstream)?;

    assert_eq!(
        Err("Cannot bind /up to /down, downstream is already bound".into()),
        test.bind(&mut upstream, &mut downstream)
    );

    test.ok()
}
