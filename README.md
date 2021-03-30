# Rust reactors prototype

## Setup

* Download [rustup](https://rustup.rs/) (Rust toolchain manager).
* Run a program:
```shell
cargo run --bin reflex_game
```


## Tour

* See `src/runtime/scheduler.rs` for the scheduler implementation.
* See `benches` for some benchmarks.
* See `src/bin` for some example programs.

The runtime assumes we have a code generator that works as described in the header of `src/bin/reflex_game.rs`. It assumes most of the structural checks have been performed by the code generator and hence elides them.

> **Note:** the crate used to contain a module that does not assume a code generator.
This was scrapped in 11a3ad5. Check it out for ideas about how to implement eg runtime checks.


## Benchmarking

```shell
cargo bench --feature benchmarking
```
You can then find plots in `target/criterion/$BENCHMARK_NAME/report` (see [Criterion report structure](https://bheisler.github.io/criterion.rs/book/user_guide/plots_and_graphs.html)).

Note: The `--feature` flag is used by conditional compilation directives (eg to remove some log statements).

## Tests

```shell
cargo test
```
Note that the code sample included in documentation comments are also tests.
See [rustdoc](https://doc.rust-lang.org/rustdoc/documentation-tests.html) reference.

Tests are organised into
* a `test` module for unit tests
* a `/tests` directory for integration tests (TODO)

Note the `#[cfg(test)]` attribute in some places, which means that an item is only compiled when running a test target.

## Status

* The scheduler is single-threaded and really dumb.
* There's too much contention for the priority queue
* Reactors and reactions needs to be wrapped in Arc (atomic reference counted).
  This smart pointer performs synchronization on access to their value. This is probably mostly useless, because synchronization can in theory be limited to just the event queue.
* Maybe using `async/await` would be nice?
* There are no tests yet... but to test is to doubt right


## Profiling a binary

Compile the binary with debug symbols (`-g`) and optimisations (`--release`)
```shell
cargo rustc --release --bin savina_pong_bin -- -g
```
(you can use whatever binary)

Make sure Oprofile is installed
```shell
sudo apt-get install linux-tools-generic oprofile
```

Run the profiler
```
$ operf target/release/savina_pong_bin

operf: Profiler started
Iteration: 1	 Duration: 540 ms

Iteration: 2	 Duration: 546 ms

Iteration: 3	 Duration: 538 ms

Iteration: 4	 Duration: 540 ms

Iteration: 5	 Duration: 554 ms

Exec summary
Best time:	538 ms
Worst time:	554 ms
Median time:	540 ms
Shutting down scheduler, channel timed out after 2 ms
* * * * WARNING: Profiling rate was throttled back by the kernel * * * *
The number of samples actually recorded is less than expected, but is
probably still statistically valid.  Decreasing the sampling rate is the
best option if you want to avoid throttling.

Profiling done.
```

Inspect results
```
$ opannotate --source | less
```

### Results

An examination of the results as obtained above for `savina_pong_bin` yields the following:
- 14% samples are malloc/free/realloc
- 16% samples are pthread_mutex_lock/unlock
- 2.5% samples are Arc::clone
- Only 0.008% samples are reaction execution (payload)
  - the mutex around the reactor is acquired in every reaction.
  Consumes 100x more time as the payload (0.6%).
  - creating the closure (=boxing) also takes a lot of time? (2%)
  ```rust
   484  0.4882 :        Self::new_from_closure(reactor_id, reaction_priority, move |ctx: &mut LogicalCtx| { /* _PATH_ReactionInvoker3new_MANGLING    506  0.5104, _PATH_ReactionInvoker3new_MANGLING   1393  1.4051, total:   1899  1.9154 */
   254  0.2562 :            let mut ref_mut = reactor.lock().unwrap();
               :            let r1: &mut T = &mut *ref_mut;
     8  0.0081 :            T::react(r1, ctx, rid)
   326  0.3288 :        }) // note: this sample count is for the release of the lock
               :    }
  ```
