# Benchmarking


Refer to the [Criterion user guide](https://bheisler.github.io/criterion.rs/book/index.html)

## Running benchmarks

```
# runs all benchmarks
$ cargo bench --features benchmarking
# filter benchmark IDS with a regex
$ cargo bench --features benchmarking -- 'ID.*Struct'
```

Note that just executing `cargo bench` might show compile errors like the following:
```
33 | use reactor_rt::internals::{new_global_rid, ExecutableReactions, GlobalIdImpl, LevelIx, ReactionLevelInfo};
   |                 ^^^^^^^^^ could not find `internals` in `reactor_rt`
```
This is why the `--features benchmarking` is for; when it is enabled, an `internals` module gives access to internal implementation details to be able to benchmark them.

## Adding a new benchmark

Add a benchmark file, and don't forget to update `Cargo.toml` (`[[benches]]`)
