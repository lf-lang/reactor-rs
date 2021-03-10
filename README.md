# Rust reactors prototype

## Setup

* Download [rustup](https://rustup.rs/) (Rust toolchain manager).
* Install the Rust nightly toolchain:
```shell
rustup install nightly
```
This is the unstable rust compiler + stdlib. We use some features that haven't been entirely stabilized yet ([function traits](https://doc.rust-lang.org/nightly/unstable-book/library-features/fn-traits.html#fn_traits))

* Run a program:
```shell
cargo +nightly run --bin reflex_game
```

### IDE

I use IntelliJ with the Rust plugin. I'm not sure how well other IDEs support Rust yet.

## Tour

A Rust artifact is called a *crate*, this one is called `rust-reactors`
* The crate is described in `Cargo.toml`, including its dependencies
* `src/lib.rs` is the top-level source file that defines the crate. Its purpose is to export the modules in the crate.

The crate contains 2 modules:
* `reactors` is the Rust-only library
  * Relevant examples are `bin/data_sharing.rs` and `bin/forwarding.rs`
  * This was mostly an initial playground, let's tear it down later
* `runtime` is the same library, but built backwards from manually written "generated code"
  * Relevant examples are 
    * `bin/reflex_game.rs`: an interactive game
      * this is the first example from which I worked out a translation strategy, it contains a description of the translation strategy
    * `bin/savina_pong.rs`: the ping/pong game from Savina benchmarks
    * `bin/struct_print.rs`: a simple send/print system




## Status

* The scheduler is single-threaded and really dumb.
* There's too much synchronization on the priority queue
* Reactors and reactions are wrapped in Arc (atomic reference counted), which perform useless synchronization too.




There's a really basic scheduler now:

```shell
$ cargo run --bin example-forwarding
Received 1
Received 2
```
See src/bin/forwarding.rs for the source of the example, and
how reactors are linked together.

* Current things to solve:
  * Values in ports may only be passed by value
  * Reactor internal states may not contain references

These will need to play with lifetimes.

* Also on the roadmap for complete support of the model:
  * Timers
  * Reactors could be parameterized with some arguments at build time
  * Multi-threaded scheduler

##### Overview

* the trait Reactor describes how the reactor behaves, including
    * What are its reactions (represented by IDs)
    * What are its subcomponents and how are they connected
    * What is its internal state
* Reactor creation is actually done by another object, the Assembler.
  This is because it's hard to make self-referential/cyclic data structures
  in safe rust. So this object manages the linking logic, and
  accumulates them into a graph
* Internal state is managed separately from the Reactor 
  instance itself. The problem is, the internal state is mutable,
  even though the reactor structure is not


A top-level reactor with no ports and no reactions is used
to wrap the entire system.

The scheduler is built from the dependency graph. The scheduler
is currently very dumb, but works:






* Example of building a reactor:
  * insert link to main.rs

* Current issues:
  * Finish implementing the assembler (before this, it's impossible to write the scheduler)
  * Thread safety: nothing is thread-safe for now. The use of RefCell
  a bit everywhere makes eg ports not Send/Sync.

#### Interesting links

* Zipper stuff: https://stackoverflow.com/questions/36167160/how-do-i-express-mutually-recursive-data-structures-in-safe-rust
* Rc: https://doc.rust-lang.org/alloc/rc/
  > A cycle between Rc pointers will never be deallocated. For this reason, Weak is used to break cycles. For example, a tree could have strong Rc pointers from parent nodes to children, and Weak pointers from children back to their parents.
  
  This should be taken care of at some point
  
* Interior mutability w/ RefCell: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
* Box vs Rc: https://abronan.com/rust-trait-objects-box-and-rc/

* The graph lib (petgraph): https://docs.rs/petgraph/0.5.1/petgraph/index.html
  
   Explanation of the idea: https://smallcultfollowing.com/babysteps/blog/2015/04/06/modeling-graphs-in-rust-using-vector-indices/
  
   Basically it's idiomatic to avoid linking nodes directly, but rather to rely on global IDs for nodes/edges. That way nodes don't need to own their neighbors. More explanations: https://featherweightmusings.blogspot.com/2015/04/graphs-in-rust.html

* Pinning: https://doc.rust-lang.org/core/pin/index.html
  * This may be necessary to ensure we refer to always the same objects

* Downcasting from trait object: https://users.rust-lang.org/t/downcast-generic-struct-instance-with-trait-object/44254/8
