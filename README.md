# Rust reactors prototype

#### Status

* I made a first prototype which I scrapped because it became
too messy. But it helped figure out the basic API and core classes.

* To get an overview: 8ce8381
  * This presents the core classes before I fleshed it out

* Basically:
  * the trait Reactor describes how the reactor behaves, including
    * What are its reactions (represented by IDs)
    * What are its subcomponents and how are they connected
    * What is its internal state
  * Reactor creation is actually done by another object, the Assembler.
  This is because it's hard to make self-referential/cyclical data structures
  in safe rust. So this object manages the linking logic, and outputs
  a data structure which stores the links (RunnableReactor)
  * Internal state is managed separately from the Reactor 
  instance itself. The problem is, the internal state is mutable,
  even though the reactor structure is not
  * The assembler is not totally implemented yet

* Example of building a reactor:
  * insert link to main.rs

* Current issues/ WIP:
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
