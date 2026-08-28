//! The contract every example in this directory follows.
//!
//! An example is a `build()` returning the piece and a `main` that hands it to
//! [`run`]. Nothing else. The piece is then reachable from the pinning test in
//! `tests/examples.rs` without running the binary, which is why the test can
//! compare notation byte for byte and hash the rendered file.
//!
//! ```ignore
//! mod common;
//! use musecode_core::prelude::*;
//!
//! pub fn build() -> Music { /* the piece */ }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     common::run("tango", &build())
//! }
//! ```
//!
//! Each example carries its own copy of `mod common;` because cargo compiles
//! every example as a separate crate; the module is shared as source, not as a
//! dependency. `examples/common/mod.rs` is not itself an example: cargo only
//! treats `examples/*.rs` and `examples/*/main.rs` as targets.

use std::error::Error;
use std::path::{Path, PathBuf};

use musecode_core::prelude::*;

/// Where an example writes its `.mid`.
///
/// Resolved from `CARGO_MANIFEST_DIR` at compile time, not from the working
/// directory. `cargo run --example tango` used to succeed from the workspace
/// root and fail with `Io(NotFound)` from inside `musecode_core/`, because the
/// path was the literal `target/tango.mid` and the directory only exists at the
/// root. The recipes in the Justfile happened to run from the root, which hid
/// it. The path is now the same wherever cargo is invoked from.
pub fn output_path(name: &str) -> PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = crate_dir.parent().unwrap_or(crate_dir);
    workspace.join("target").join(format!("{name}.mid"))
}

/// Print the notation, print the structural summary, write the `.mid`.
///
/// The three calls are the whole contract, in this order, so every example
/// reads the same way and the test knows what a run produces.
pub fn run(name: &str, music: &Music) -> Result<(), Box<dyn Error>> {
    println!("{music}\n");
    print!("{}", summary(music)?);

    let path = output_path(name);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_midi(music, &path, &MidiOptions::default())?;
    println!("\nwrote {}", path.display());
    Ok(())
}
