# VecMap

A formally verified sorted sparse map implementation using `Vec` underneath.
The data structure is verified using [Creusot](https://github.com/xldenis/creusot).\
The crate is ready to use as is.

## Verification

This section describes the process of reproducing the verification of this crate.

### Preliminaries

Creusot's annotations require Rust Nightly, and further some trait restrictions that
would be extra work to implement on usage. This is why this repo contains two `VecMap`
implementations:
- `src/lib.rs`
- `creusot/src/lib.rs`

The implementation in the `creusot` subdir contains the annotations and trait
restrictions/implementations to work with Creusot. Otherwise the implementations are
identical.\
To verify this yourself, load the two files into a diff tool of your choosing and
ensure that `src/lib.rs` doesn't add anything over `creusot/src/lib.rs`. Note that
some things like `Debug`/`Display` and iterator implementations are disabled with
[conditional compilation flags](https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg-attribute)
for Creusot to work.

### Creusot

For verfication with Creusot the following things are needed:
- [Rust](https://www.rust-lang.org/tools/install) via `rustup`
- [Creusot](https://github.com/xldenis/creusot)
   - follow the instructions in the [Readme](https://github.com/xldenis/creusot#installing-creusot-as-a-user)
- Make

Instructions:
1) enter the `creusot` subdir
2) run `make`
   - This will compile the project, extract the generated specification, and open it in Why3.
3) In Why3 you can see all the individual proofs in the left column. Select the root node and press `3` on the keyboard.
   - This will automatically split and verify the proof goals.
4) Once the root node features a green check mark icon, the verification is complete.

## Licensing

This crate for use in other Rust projects is available under the MIT license.
The files in the `creusot/prelude` sub-directory are licensed under LGPLv2.1.
Also note, due to using Creusot (licensed LGPLv2.1) as a dependency for verification,
the compilation artefacts in `creusot` are also licensed LGPLv2.1.
See the respective licensing files for details.
