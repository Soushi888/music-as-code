# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo check          # type-check without producing artifacts
cargo test           # run the test suite
cargo test <name>    # run a single test by name
just ci              # check + test in one shot
just render tango    # run musecode_core/examples/tango.rs: print notation + summary, write target/tango.mid
just play tango      # render, then play target/tango.mid through fluidsynth (just listen is an alias)
just docs            # build and serve rustdoc at http://localhost:8080/musecode_core/
cargo doc --no-deps  # build rustdoc without serving
```

## Architecture

`musecode_core` is a Rust library — the only crate in this workspace. It is a musical DSL built around a small, orthogonal intermediate representation (IR): five core ADT constructors, polymorphic pitch, rational time, and content-addressed fragments. The MIDI backend is the first renderer; the rest of the codebase is the IR, its transformations, the resolver, display and analysis.

### Seven-layer dependency graph (each layer may only import from layers below it)

| Layer | Module | What it provides |
|-------|--------|-----------------|
| 1 | `pitch` | `Letter`, `Accidental`, `PitchClass`, `ChromaticPitch`, `Degree`, `Interval`, `Pitch` |
| 2 | `time` | `Beats = Rational32`, `Tempo`, `TimeSig`, `Dynamics`, duration helpers |
| 3 | `music` + `attrs` + `control` | `Music` (5 constructors), `Note`, `NoteAttrs`, `Control` (12 variants), operator overloads |
| 4 | `theory` | `Key`, `Scale`, `Mode`, `Chord`, `ChordQuality`, `Voicing` |
| 5 | `combinators` | `transpose`, `augment`, `retrograde`, `invert`, `canon`, `map_notes` |
| 3 | `display` | `impl Display for Music`, the notation in `docs/NOTATION.md` |
| 5 | `resolve` | `resolve()`, `Event`, `Resolved`, `ResolveError` (the one pass that gives `Degree`/`Interval`/`Control` meaning) |
| 5 | `analysis` | `pitch_range`, `pitch_class_set`, `interval_histogram`, `vertical_slices`, `label_triad`, `summary` over `&[Event]` |
| 6 | `backends/hints` | `BackendHint`, `LilypondHint`, `MidiHint`, `AudioHint` |
| 6 | `backends/midi` | `render_midi`, `render_resolved`, `write_midi`, `MidiOptions`, `MidiError`, `program_for` (Standard MIDI File over `midly`) |
| 7 | `phrase` | `Phrase` (Arc + Blake3 hash), `RenderCache` |

`prelude` re-exports everything and is the intended import for users.

### The five-constructor ADT

```rust
pub enum Music {
    Note(Note),
    Rest(Beats),
    Seq(Vec<Music>),
    Par(Vec<Music>),
    Modify(Control, Box<Music>),
}
```

`Seq` and `Par` **flatten on composition**: `a + b + c` produces `Seq([a,b,c])`, not nested `Seq` nodes. `Modify` scopes context (key, tempo, transposition) to a subtree without affecting siblings.

### Polymorphic pitch

`Pitch` is an enum of `Chromatic(ChromaticPitch)`, `Degree(Degree)`, and `Interval(Interval)`. `Degree` and `Interval` variants are unresolved until `resolve::resolve` walks the tree with accumulated `Control::Key`/`Control::Scale` context; backends consume `Resolved`, never `Music`. This is what makes `.diatonic_transpose()` and key modulation work without rewriting note values. `invert()` is the only combinator that only operates on `Chromatic` pitches — `Degree`/`Interval` pitches pass through unchanged.

### Dependency constraints (never violate these)

- `pitch`, `time`, `backends/hints` are leaf modules — they must import nothing internal
- `attrs` must never import `music` or `control` (it exists specifically to break that cycle)
- `control` must never import `music`
- `combinators` may import `music` and `control` but not `phrase`
- `resolve` may import `music`, `control`, `theory`, `pitch`, `time`, `attrs`, `backends/hints`; never `phrase`, `combinators`, or a concrete backend
- `analysis` takes `&[Event]` from `resolve`; it must never import a backend (`summary(&Music)` is the one convenience that resolves a tree itself)
- `backends/midi` consumes `Resolved` from `resolve` (plus `music`, `attrs`, `backends/hints`, `time`); it must never import `combinators`, `theory`, `analysis`, or `display`
- `phrase` must never import `combinators` or `theory`

### Rational time

`Beats = Rational32` from the `num-rational` crate. Duration helpers (`q()`, `h()`, `e()`, etc.) are functions, not constants, because `Rational32::new` is not `const fn`. All duration arithmetic is exact integer arithmetic with no floating-point accumulation.

### Content addressing

`Phrase` wraps `Music` in `Arc<Music>` with a Blake3 hash (computed via `bincode` serialization). `RenderCache<O>` maps `(blake3::Hash, BackendId)` to rendered output; only changed subtrees need re-rendering.

## Open design questions

Before implementing backends, these IR questions remain unresolved (see README):

1. `Modify` with `Vec<Control>` vs single `Control` — saves tree depth vs. canonical form
2. `Par` alignment semantics for unequal-length children (truncate / loop / pad)
3. Whether first-class `Score { voices: Map<VoiceId, InstrumentId> }` is needed for engraving
4. Tie semantics across `Seq` boundaries (flag vs. tree-rewrite pass at render time)
