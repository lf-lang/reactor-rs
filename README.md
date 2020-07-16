# Rust reactors prototype

## Status

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
