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

Main differences between the two:
* The `runtime` module assumes most of the structural checks have been performed by the code generator and hence elides them. 
* There is no dependency graph structure in `runtime`; instead the list of downstream reactions is precomputed (at gen time) for each port and action, and hardcoded in the initialization routine (`ReactorAssembler::assemble`).
* The `runtime` module layers reactors into several structs, which is cumbersome to write by hand but is ok for a generator. This achieves separation of internal state and meta-information about components.


## Status

The `runtime` module design is by no means definitive.

* The scheduler is single-threaded and really dumb.
* There's too much contention for the priority queue
* Reactors and reactions needs to be wrapped in Arc (atomic reference counted).
  This smart pointer performs synchronization on access to their value. This is probably mostly useless, because synchronization can in theory be limited to just the event queue.
* Maybe using `async/await` would be nice?
* More doc should be migrated from the `reactors` module to `runtime`
* There are no tests yet... but to test is to doubt right
