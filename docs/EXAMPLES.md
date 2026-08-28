# Examples

`musecode_core/examples/` is where each facet of the DSL is demonstrated and pinned. Every example is a piece of music you can hear, and every example's output is held still by a test, so a change to a renderer shows up as a failing assertion naming the example rather than as a surprise the next time somebody listens.

## The contract

An example is two things: a `build()` that returns the piece, and a `main` that hands it to `common::run`.

```rust
//! What this example demonstrates, in a sentence.

mod common;

use musecode_core::prelude::*;

/// The piece. `tests/examples.rs` pins what this returns.
pub fn build() -> Music {
    // ...
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::run("name", &build())
}
```

`common::run` does exactly three things, in this order: print the notation, print the structural `summary`, write the `.mid`. Nothing else belongs in an example's `main`. Keeping the piece in `build()` is what lets the test reach it without running the binary.

Each example carries its own `mod common;` line. Cargo compiles every example as a separate crate, so the module is shared as source rather than as a dependency. `examples/common/mod.rs` is not itself an example: cargo only treats `examples/*.rs` and `examples/*/main.rs` as targets.

## Where the file goes

`common::output_path` resolves against `CARGO_MANIFEST_DIR` at compile time, not against the working directory, so the `.mid` lands in the workspace `target/` no matter where cargo was invoked from.

This was a real bug before the contract existed. The example wrote the literal relative path `target/tango.mid`, which exists at the workspace root and not inside `musecode_core/`, so `cargo run --example tango` succeeded from one directory and failed with `Io(NotFound)` from the other, after printing the entire summary. The Justfile recipes happened to run from the root, which hid it.

## Running one

```bash
cargo run --example tango      # print and write, from anywhere in the workspace
just render tango              # the same
just play tango                # render, then play it through fluidsynth
just gain=2 play tango         # louder; overrides go before the recipe name
```

## The pinning test

`musecode_core/tests/examples.rs` holds one test per example. Each pulls the example in through its source path, so the test calls the same `build()` the binary calls and there is one definition of each piece rather than a copy living in a test.

Three things are pinned per example:

| What | Against | Catches |
|---|---|---|
| the notation | `tests/snapshots/<name>.txt`, byte for byte | anything `Display` can see: pitches, durations, controls, layout |
| the `.mid` | a sha256 in the test | renderer changes the notation cannot see, ticks and program numbers for instance |
| a few `analysis` facts | event count, pitch range, total duration | gives a failing hash a hint, so the diff is not two hex strings |

Both halves are live and were checked by breaking them on purpose: changing the tempo trips the snapshot, changing the default PPQ trips the hash, and each names the example.

## Changing an example on purpose

```bash
UPDATE_SNAPSHOTS=1 cargo test --test examples
```

That rewrites the notation snapshots. Read the diff before committing it: a snapshot you did not look at pins nothing.

The `.mid` hashes are deliberately not auto-updated. A renderer change should cost a moment's thought, so the new hash goes in by hand from the assertion's `left` value.

## Adding one

1. Write `examples/<name>.rs` following the contract above.
2. Add the module and a test to `tests/examples.rs`.
3. Run `UPDATE_SNAPSHOTS=1 cargo test --test examples` to create the snapshot, then read it.
4. Run the test again, take the hash from the failure, and paste it in.
5. Commit the snapshot along with the example.
