# Rust reactors prototype

#### Status

* I made a first prototype which I scrapped because it became
too messy. But it helped figure out the basic API and core classes.

* To get an overview: 8ce8381
  * This presents the core classes before I fleshed it out
* Example of building a reactor

* Core model classes like Action, Reaction, etc, are workable.
You'll notice that they're super stripped-down and not so convenient
to work with (eg a reactor doesn't know its parent reactor). This
is to make the layout simple for now, because it's already pretty
complicated to track who references whom. 
   
   In safe Rust it's complicated to build recursive/self-referential 
data structures that are mutable. Smart pointers in smart pointers in smart pointers...


* The assembler is still in a larval stage, to be useful it should
  1. compute the topology graph, taking reaction priority into account.
  This is used by the scheduler to schedule instantaneous reactions to SET
  2. (later) Validate during construction (this needs more structure
  than currently)
  3. also it needs tests

   This is what I should focus on right now, before item 1 is fixed there
   is no way to write the scheduler.


I also don't know if the trick of sharing the RefCell for 
output & input ports can stay. The issue I see is thread-safety.
We could be setting the value while another reaction is 
executing concurrently -> BorrowMutError



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
  * This will be necessary to ensure we refer to always the same objects

* Downcasting from trait object: https://users.rust-lang.org/t/downcast-generic-struct-instance-with-trait-object/44254/8
