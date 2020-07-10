

#### Interesting links

* Zipper stuff: https://stackoverflow.com/questions/36167160/how-do-i-express-mutually-recursive-data-structures-in-safe-rust
* Rc: https://doc.rust-lang.org/alloc/rc/
  > A cycle between Rc pointers will never be deallocated. For this reason, Weak is used to break cycles. For example, a tree could have strong Rc pointers from parent nodes to children, and Weak pointers from children back to their parents.
  
  This should be taken care of at some point
  
* Interior mutability w/ RefCell: https://doc.rust-lang.org/book/ch15-05-interior-mutability.html
* Box vs Rc: https://abronan.com/rust-trait-objects-box-and-rc/

* The graph lib (petgraph): https://docs.rs/petgraph/0.5.1/petgraph/index.html
  * Explanation of the idea: https://smallcultfollowing.com/babysteps/blog/2015/04/06/modeling-graphs-in-rust-using-vector-indices/
  
    Basically it's idiomatic to avoid linking nodes directly,
    but rather to rely on global IDs for nodes/edges.
  * Explanation of the problem with graphs in rust: https://featherweightmusings.blogspot.com/2015/04/graphs-in-rust.html
