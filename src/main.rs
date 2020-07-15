use std::process::exit;

#[macro_use]
pub mod reactors;

#[cfg(feature = "examples")]
pub mod examples;

fn main() {
    examples::forwarding::main()
}
