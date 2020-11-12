#![allow(clippy::needless_lifetimes)]
#![allow(unused_variables)]

/*//

use std::time::Instant;
use std::fmt::Display;
use crate::runtime::{InputPort, OutputPort, bind_ports};
use std::cell::{RefCell, Ref};
use std::marker::PhantomData;

// A struct that defines two fields
struct LogicalTime { instant: Instant, step: i32 }

struct URL;

trait Steppable {
    type NextStep;
    // type declaration
    fn step(&self) -> Self::NextStep; // method declaration
}

impl Steppable for LogicalTime {
    type NextStep = Self;

    fn step(&self) -> Self::NextStep {
        self.next_step()
    }
}

impl LogicalTime { // impl block
fn println(&self) {
    // println!("Logical time(instant={}, step={})", self.instant, self.step);
}

    // method declaration
    fn next_step(&self) -> LogicalTime {
        // last expression is returned
        LogicalTime {
            instant: self.instant, // implicit copy of instant
            step: self.step + 1,
        }
    }
}

// An enum with two variants
enum Request {
    Put { id: URL, content: String },
    Get { id: URL },
}

fn fun(time: LogicalTime) {
    let time2: LogicalTime = time; // move time into time2

    // time.println();  // error, time was moved
    time2.println(); // ok
}

struct Hello {
    name: &'static str,
    value: i32,
}

fn hello() {
    let mut out: OutputPort<Hello> = OutputPort::new();
    let mut input: InputPort<Hello> = InputPort::new();

    bind_ports(&mut out, &mut input);
}

fn stuff() {
    type SensorData = [i32; 32];

    let mut sensor_data: OutputPort<SensorData> = OutputPort::new();
    let mut sensor_input: InputPort<SensorData> = InputPort::new();

    bind_ports(&mut sensor_data, &mut sensor_input);
}

fn k<T>(r: RefCell<T>) -> T where T: Copy {
    *r.borrow()
}

// 'w: lifetime of the wave
struct WaveCtx<'w> {
    _p: PhantomData<&'w str>
}

struct Port<T> {
    _p: PhantomData<T>
}

//     fn set<T>(&mut self, port: &mut Port<T>, v: T) where T: Copy + 'w {}

// 'w: lifetime of the wave
impl<T> Port<T> where T: Copy {
    fn get<T>(&self) -> T {
        // where could the value be? obviously the port
        panic!()
    }

    fn set<T>(&self, t: T) {
        // where could the value be? obviously the port
        panic!()
    }
}

// request exclusive ownership of the value
fn consume(owned: String) {}

fn readonly_borrow(shared: &String) {}

fn mutable_borrow(mutable: &mut String) {
    // we can read from this mutable reference
    readonly_borrow(mutable);

    let my_str = String::from("123");
    // write to it
    *mutable = my_str;
    // but this moves the value
    consume(my_str); // error: use of moved value
}

fn clone<T>(some_ref: &T) -> T {}

fn get_copy<T>(port: &Port<T>) -> T {
    let r: &T = get_ref(port);
    return clone(r);
}

fn set<T>(port: &mut OutputPort<T>, value: T) {}

// fn get<T>(port: &InputPort<T>) -> Option<T> where T: Copy {}

fn with_ref<T, R>(port: &InputPort<T>,
                  closure: impl FnOnce(&T) -> R) -> Option<R> {}

// todo
fn get_ref<T>(port: &Port<T>) -> &T {}


fn react_startup(out: &mut OutputPort<Hello>) {
    // Create our Hello struct
    let mut hello = Hello { name: "Venus", value: 42 };
    hello.name = "Earth";
    // implicitly moved
    set(out, hello);
    // hello.name = "Mars"; // error!
}

fn example<T: Clone>(owned: T,
                     shared: &T,
                     mutable: &mut T, ) {}

fn react_no_copy(port: &Port<[i32; 64]>) {
    let inner_ref : &[i32; 64] = get_ref(port);
    // sum values, no need to copy
    let sum: i32 = inner_ref.iter().sum();
}

fn react_copy(port: &Port<i32>) {
    // copies the value out
    let value: i32 = get_copy(port);
}

fn elevation<T>(t_ref: &T) {
    let t: T = *t_ref;
}

fn elevation_ok<T>(t_ref: &T) {
    let t: T = clone(t_ref);
}

fn example2() {
    let str: String = String::new();

    let mut str2: String = str; // move

    let shared_ref1: &String = &str2; // borrow
    let shared_ref2: &String = &str2; //

    let mutable_borrow: &mut String = &mut str2;

    readonly_borrow(&str); // ok
    readonly_borrow(shared_ref2); // ok

    let str2: String = str; // move into str
    readonly_borrow(&str2); // ok
    readonly_borrow(shared_ref1);  // error, value was moved

    consume(str2); // move str2 into function
// now both str2 and str have been moved,
// we can't do anything anymore.
}

fn react_get(input: &Port<&i32>) {
    let ref_: &i32 = get_copy(input);
    let x: i32 = *ref_;
    assert_eq!(4, x);
}

fn react_set(output: &mut Port<&i32>) {
    let some_int: i32 = 2;
    let some_ref: &i32 = &some_int;
    // error: "some_int does not live long enough"
    set(output, some_ref);
}


fn invoke<'w>(ctx: &mut WaveCtx<'w>) {
    // let mut ctx = Ctxx { _p: PhantomData };
    let i = Port::<&mut i32> { _p: PhantomData };
    react_transform(ctx, &mut i);
}

//
// pub struct Input;
//
// pub struct Output;
//
// pub type InputPort<T> = Port<T, Input>;
// pub type OutputPort<T> = Port<T, Output>;
//
// pub struct Port<T, Kind> {
//     // ...
// }
//
// // an impl block for behavior shared between output & input ports
// impl<T, Kind> Port<T, Kind> {
//     fn get(&self) -> Option<T> {
//         // ...
//     }
// }
//
// // an impl block only for output ports
// impl<T> OutputPort<T> {
//     fn set(&self, _value: T) {
//         // ...
//     }
// }

*/
