use crate::runtime::*;


type TestInputPort<T> = Port<T, Input, FakeDeps>;
type TestOutputPort<T> = Port<T, Output, FakeDeps>;
/// Mock type of collection of dependencies
type FakeDeps = Vec<i32>;

fn set_port<T>(port: &mut OutputPort<T>, v: T) {
    port.set_impl(v, |_| {})
}

#[test]
fn a_port_is_initially_empty() {
    let port = InputPort::<i32>::new();
    assert_eq!(None, port.get()); // default value?
}

#[test]
fn binding_two_ports_should_let_values_be_read() {
    let mut upstream = OutputPort::<i32>::new();
    let mut downstream = InputPort::<i32>::new();

    assert_eq!(None, downstream.get());

    bind_ports(&mut upstream, &mut downstream);

    assert_eq!(None, downstream.get());

    set_port(&mut upstream, 5);

    assert_eq!(Some(5), downstream.get());
}

#[test]
fn a_port_can_be_upstream_of_several_ports() {
    let mut upstream = OutputPort::<i32>::new();
    let mut d1 = InputPort::<i32>::new();
    let mut d2 = InputPort::<i32>::new();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    bind_ports(&mut upstream, &mut d1);
    bind_ports(&mut upstream, &mut d2);

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
    let mut upstream = OutputPort::<i32>::new();
    let mut d1 = InputPort::<i32>::new();
    let mut d2 = InputPort::<i32>::new();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    // upstream -> d1 -> d2
    bind_ports(&mut upstream, &mut d1);
    bind_ports(&mut d1, &mut d2);

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
    let mut upstream = OutputPort::<i32>::new();
    let mut d1 = InputPort::<i32>::new();
    let mut d2 = InputPort::<i32>::new();
    let mut b1 = InputPort::<i32>::new();
    let mut b2 = InputPort::<i32>::new();

    assert_eq!(None, d1.get());
    assert_eq!(None, d2.get());

    // upstream -> d1 -> d2 -> b1
    //                   d2 -> b2

    // Note that linking the ports the other way around doesn't
    // work, we need to go in topo order

    bind_ports(&mut upstream, &mut d1);

    bind_ports(&mut d1, &mut d2);

    bind_ports(&mut d2, &mut b1);
    bind_ports(&mut d2, &mut b2);


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
fn dependency_merging() {
    let mut upstream = TestOutputPort::<i32>::new_for_test("up");
    let mut downstream = TestInputPort::<i32>::new_for_test("down");

    // pretend the downstream depends on those
    downstream.set_downstream(vec![1, 2, 3]);

    bind_ports(&mut upstream, &mut downstream);

    // actually they're bound to the same cell
    assert_eq!(Some(vec![1,2,3]), upstream.get_downstream_deps());
    assert_eq!(Some(vec![1,2,3]), downstream.get_downstream_deps());
}

#[test]
#[should_panic]
fn repeated_binding_panics() {
    //
    let mut upstream = TestOutputPort::<i32>::new_for_test("up");
    let mut downstream = TestInputPort::<i32>::new_for_test("down");

    bind_ports(&mut upstream, &mut downstream);
    bind_ports(&mut upstream, &mut downstream);
}
