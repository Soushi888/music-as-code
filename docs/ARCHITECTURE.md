# Architecture

`muse_core` is the kernel of the `muse` musical DSL. It is small by design: five core ADT constructors, a polymorphic pitch hierarchy, rational time, and content-addressed fragments. Every higher-level construct — chord voicings, canon, swing — is built from these primitives without extending the core types.

---

## Module Dependency Graph

```
                    ┌─────────────────────────────────────────┐
                    │               prelude                   │
                    │          (re-exports everything)        │
                    └───────────────────┬─────────────────────┘
                                        │ imports all
          ┌─────────────────────────────┼──────────────────────┐
          │                             │                      │
          ▼                             ▼                      ▼
      phrase ◄──── music ◄───── control ◄──── theory ◄─── pitch  (leaf)
                     │               │                         ▲
                     │               │            ┌────────────┘
                     └──────► attrs ◄┘            │
                                 │                │
                                 ▼                │
                            backends/hints        │
                            (leaf)          time  (leaf)
```

**Arrows mean "imports from."** No cycles.

| Module | Imports from |
|--------|-------------|
| `pitch` | nothing (leaf) |
| `time` | nothing (leaf) |
| `backends/hints` | nothing (leaf) |
| `attrs` | `backends/hints` |
| `theory` | `pitch` |
| `control` | `pitch`, `time`, `attrs`, `theory`, `backends/hints` |
| `music` | `pitch`, `time`, `attrs`, `control` |
| `combinators` | `music`, `control`, `time` |
| `phrase` | `music`, `attrs` |
| `prelude` | everything |

The key structural decision: `attrs` is a shared leaf module that both `control` and `music` import. This breaks the potential cycle that would arise if `Music::Modify(Control, _)` caused `music` and `control` to depend on each other.

---

## The Seven Layers

### Layer 1: Pitch (`pitch`)

The pitch hierarchy has three tiers:

1. **`PitchClass`** = `Letter` + `Accidental`. No octave. Enharmonic spelling is preserved (C# and Db are distinct values even though `semitones()` returns the same integer for both).
2. **`ChromaticPitch`** = `PitchClass` + `octave: i8`. Fully resolved. C4 = MIDI 60.
3. **`Pitch`** = polymorphic union of `Chromatic(ChromaticPitch)`, `Degree(Degree)`, and `Interval(Interval)`.

`Degree` and `Interval` are unresolved until a `Control::Key` / `Control::Scale` context is present. Backends receive only `ChromaticPitch`; resolution is the backend's first rendering pass.

### Layer 2: Time (`time`)

`Beats = Rational32` from the `num-rational` crate. Quarter note = `1/1`, eighth = `1/2`, dotted quarter = `3/2`. Duration arithmetic is exact integer arithmetic — no floating-point accumulation.

Duration helpers are functions rather than `const` items because `Rational32::new` is not a `const fn`.

### Layer 3: Core ADT (`music`, `attrs`, `control`)

The `Music` enum has exactly five constructors:

```
Music
 ├── Note(Note)                     leaf: a sounded event
 ├── Rest(Beats)                    leaf: silence
 ├── Seq(Vec<Music>)                sequential composition
 ├── Par(Vec<Music>)                parallel composition
 └── Modify(Control, Box<Music>)    context scoping
```

`Seq` and `Par` flatten on composition: the `Add` and `BitOr` operator implementations merge adjacent sequences and parallels instead of nesting them. A chain `a + b + c + d` produces `Seq([a, b, c, d])`, not `Seq([Seq([Seq([a, b]), c]), d])`. Tree depth stays proportional to actual musical structure.

`Modify` is how context propagates. Wrapping a subtree in `Modify(Control::Key(k), body)` changes how degree-pitches and diatonic transpositions resolve inside `body` without affecting sibling or parent nodes.

### Layer 4: Theory (`theory`, `control`)

`Key`, `Scale`, `Mode`, `Chord`, `ChordQuality`, `Extension`, and `Voicing` live here. These are purely data types — no resolution logic. Resolution (mapping a `Degree` in key F minor to a specific `ChromaticPitch`) belongs in the rendering backend.

`Control` is a 12-variant enum of modifiers that can be wrapped around any `Music` subtree.

### Layer 5: Combinators (`combinators`)

Pure functions on `Music`. All take a `Music` value and return a transformed one. No mutation. Designed to compose:

```rust
melody
    .transpose(5)
    .augment(h())
    .retrograde()
    .pipe(canon(vec![(q(), 7)]))
```

Methods implemented on `Music` directly:
- `transpose`, `diatonic_transpose`: wrap in `Modify`
- `augment`, `diminish`: tree walk multiplying all `Beats` values
- `retrograde`: reverses `Seq` children recursively
- `invert`: maps `ChromaticPitch` values by reflecting around an axis
- `map_notes`, `map_rests`: structural recursion primitives
- `pipe`: apply any `FnOnce(Music) -> Music` inline

Free functions (stubs pending implementation): `swing`, `humanize`, `canon`.

### Layer 6: Backend Hints (`backends/hints`)

`BackendHint` is an additive metadata enum attached to `NoteAttrs`. Three sub-enums:

- `LilypondHint` — stem direction, beam marks, raw `\markup` escapes, layout flags
- `MidiHint` — program change, channel, CC values
- `AudioHint` — keyswitch note, articulation name, mic position, round-robin index

A backend that doesn't understand a hint ignores it. The IR itself (`Music`) carries no backend-specific state.

### Layer 7: Phrases (`phrase`)

`Phrase` wraps a `Music` tree in an `Arc<Music>` and stores its Blake3 hash (computed via `bincode` serialization). Two `Phrase`s with the same hash are structurally identical.

`RenderCache<O>` maps `(blake3::Hash, BackendId)` to rendered output `O`. When a large piece changes only one section, only that section's subtree hash is new; everything else is a cache hit.

---

## Design Decisions

### Why exactly five constructors?

Hudak's "The Haskell School of Music" showed that `Note`, `Rest`, `Seq`, `Par`, and a context modifier cover the entire space of structured music. A sixth constructor would either be redundant (expressible as a tree of the five) or would embed backend-specific knowledge into the IR (violating principle 5).

### Why `Modify` with a single `Control`, not `Vec<Control>`?

Simpler canonical form. If multiple controls are needed for a section, nest two `Modify` nodes. The order of nesting becomes explicit and meaningful (inner shadows outer). This is an open question flagged in the README: `Vec<Control>` would save tree depth at the cost of canonicality.

### Why not GATs or higher-kinded types for the polymorphic pitch?

Rust's type system could express "a `Music<P>` parameterized by pitch type P" with GATs, making the resolved/unresolved distinction a compile-time guarantee. The design deliberately avoids this. The ergonomic cost of propagating a type parameter through every combinator and operator outweighs the safety benefit, especially since resolution errors surface clearly at render time.

### Why `attrs` as a separate module?

To prevent a circular dependency. Both `music` and `control` need `Articulation`, `NoteAttrs`, and `VoiceId`. If these lived in `music`, then `control` would need to import `music` — but `music` imports `control` for the `Music::Modify` constructor. `attrs` as a shared leaf breaks the cycle cleanly.

---

## Dependency Constraints (for future maintainers)

- `pitch`, `time`, `backends/hints` must never import other internal modules
- `attrs` must never import `music` or `control`
- `control` must never import `music`
- `combinators` may import `music` and `control` but not `phrase`
- `phrase` must never import `combinators` or `theory`
