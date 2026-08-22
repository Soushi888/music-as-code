# MuseCode

**Music as Code.** A Rust DSL for composing, transforming, and rendering music from source.

Write a Piazzolla-flavored ii-V-I in F minor, render it to LilyPond PDF, MIDI, and audio, all from the same source file. Change the key, reharmonize, invert the melody, add a canon voice. The same tree produces every output.

---

## Design Principles

1. **The IR is small and orthogonal.** Five core constructors. Everything else is sugar built from them.
2. **Pitch is polymorphic but resolves to chromatic.** Scale degrees and intervals exist for composition, but every backend sees fully-resolved chromatic pitches with preserved enharmonic spelling.
3. **Time is rational.** No floating-point drift. A 7:5 polyrhythm over 32 bars is exact.
4. **Subtrees are content-addressable.** Every fragment has a Blake3 hash. Cache lookup is O(1). Re-rendering only touches changed subtrees.
5. **Backend hints live alongside, not inside.** The IR is what the music *is*. Hints are how a specific backend should interpret it.
6. **No clever encoding tricks at the type level.** Enums for ADTs, traits for backends, no GAT acrobatics.

---

## Quick Start

```rust
use musecode_core::prelude::*;

// Three ways to write the same C major triad
let explicit   = seq![n(C4, q()), n(E4, q()), n(G4, h())];
let by_degree  = seq![n(d!(1), q()), n(d!(3), q()), n(d!(5), h())];
let by_chord   = chord([C4, E4, G4], h());

// Operators: + chains, | stacks voices, * repeats
let melody = n(C4, q()) + n(E4, q()) + n(G4, h());
let bass   = n(C3, h()) + n(G3, h());
let duo    = melody | bass;           // two voices, simultaneous
let twice  = melody.clone() * 2;     // repeat twice

// Wrap a section in context modifiers
let piece = duo
    .modify(Control::Key(Key::minor(pc!(F))))
    .modify(Control::TimeSignature(TimeSig::common()))
    .modify(Control::Tempo(Tempo::bpm(96)));
```

### Macros

```rust
seq![n(C4, q()), n(E4, q()), n(G4, h())]   // sequential composition
par![melody, bass, inner_voice]              // parallel composition

d!(1)       // tonic
d!(b 3)     // flat third (modal mixture)
d!(# 7)     // raised seventh

pc!(F)      // F natural PitchClass, for use in Key / Chord
```

### Combinators

```rust
melody
    .transpose(5)                   // up a perfect fourth
    .diatonic_transpose(2)          // up two scale steps
    .augment(h())                   // double all durations
    .retrograde()                   // time-reverse
    .invert(C4.midi())              // pitch-invert around C4
    .pipe(canon(vec![(q(), 7)]))    // add a canon voice at a fifth
```

`swing` and `humanize` are declared but not implemented: calling either panics with `todo!()`. They are scheduled for milestone M3 (see [Roadmap](#roadmap)).

---

## Architecture

The codebase is organized in seven conceptual layers, each building on the previous:

| Layer | Module | What it provides |
|-------|--------|-----------------|
| 1. Pitch | `pitch` | `Letter`, `Accidental`, `PitchClass`, `ChromaticPitch`, `Degree`, `Interval`, `Pitch` |
| 2. Time | `time` | `Beats = Rational32`, `Tempo`, `TimeSig`, `Dynamics`, duration helpers |
| 3. Core ADT | `music` | `Music` (5 constructors), `Note`, `NoteAttrs`, operator overloads |
| 4. Theory | `theory` | `Key`, `Scale`, `Mode`, `Chord`, `ChordQuality`, `Voicing` |
| 5. Combinators | `combinators` | `transpose`, `augment`, `retrograde`, `invert`, `canon`, `map_notes` |
| 6. Backend hints | `backends/hints` | `BackendHint`, `LilypondHint`, `MidiHint`, `AudioHint` |
| 7. Phrases | `phrase` | `Phrase` (Arc + Blake3 hash), `RenderCache` |

Supporting modules: `attrs` (shared ID newtypes and `NoteAttrs`), `control` (`Control` enum), `prelude` (re-exports everything).

### The five-constructor ADT

Everything playable is a tree of exactly these five node types:

```rust
pub enum Music {
    Note(Note),              // a sounded event
    Rest(Beats),             // silence
    Seq(Vec<Music>),         // play in order
    Par(Vec<Music>),         // play simultaneously
    Modify(Control, Box<Music>),  // apply context to a subtree
}
```

`Seq` and `Par` flatten on composition: `a + b + c + d` produces `Seq([a,b,c,d])`, not a nested tree. `Modify` propagates context (key, tempo, transposition) down through its subtree without touching siblings.

### Polymorphic pitch

```rust
pub enum Pitch {
    Chromatic(ChromaticPitch),   // C4, Eb5: explicit
    Degree(Degree),              // 1, b3, #7: relative to Key context
    Interval(Interval),          // M3, P5: relative to previous note
}
```

A melody written with `Degree` pitches reharmonizes freely: wrap the same `Music` tree in `Modify(Control::Key(new_key), _)` and the melody moves with it. Backends always receive resolved `ChromaticPitch` values.

---

## Worked Example: ii-V-I in F minor

A Piazzolla-flavored four-bar sketch showing bass, comping, and melody together:

```rust
use musecode_core::prelude::*;

fn main() {
    let key = Key::minor(pc!(F));

    // Tango bass: dotted-eighth + 16th + 8th + 8th + quarter + quarter
    let tango = |root: ChromaticPitch| seq![
        n(root, dot(e())), n(root, s()),
        n(root, e()),      n(root, e()),
        n(root, q()),      n(root, q()),
    ];

    // Shell-voiced comping on beats 2 and 4
    let comp_rest = r(q()) + r(dot(q()));

    // Melody in scale-degree space: descends 5 b5 4 b3
    let melody = seq![
        n(d!(5), q()), n(d!(b 5), e()), n(d!(4), e()), n(d!(b 3), h()),
    ];

    // Assemble bars: bass | comp | melody
    let bar1 = tango(G3) | comp_rest.clone() | melody.clone();
    let bar2 = tango(C3) | comp_rest.clone() | melody.diatonic_transpose(-1);
    let bar3 = tango(F3) | comp_rest        | seq![n(d!(1), w())];

    let piece = seq![bar1, bar2, bar3.clone(), bar3]
        .modify(Control::Key(key))
        .modify(Control::TimeSignature(TimeSig::common()))
        .modify(Control::Tempo(Tempo::bpm(96)));

    // Same source tree, three outputs (backends not yet implemented):
    // LilypondBackend::default().render(&piece) -> score PDF
    // MidiBackend::default().render(&piece)     -> .mid file
    // FluidsynthBackend::new("FluidR3.sf2").render(&piece) -> .wav
}
```

The melody is in scale-degree space: change `pc!(F)` to `pc!(C)` and the `tango` bass notes are the only thing that needs manual updating. The descending line follows automatically.

---

## Crate Layout

```
musecode_core/src/
├── lib.rs            crate root, module declarations
├── prelude.rs        re-exports the full public API
├── pitch.rs          Layer 1: pitch types and constants
├── time.rs           Layer 2: Beats, Tempo, TimeSig, Dynamics
├── attrs.rs          shared: VoiceId, InstrumentId, Articulation, NoteAttrs
├── music.rs          Layer 3: Music ADT, Note, smart constructors, operators
├── control.rs        Layer 4 support: Control enum
├── theory.rs         Layer 4: Key, Scale, Mode, Chord, Voicing
├── combinators.rs    Layer 5: transpose, augment, retrograde, invert, canon
├── phrase.rs         Layer 7: Phrase (content-hashed), RenderCache
└── backends/
    ├── mod.rs
    └── hints.rs      Layer 6: BackendHint, LilypondHint, MidiHint, AudioHint
```

---

## Open Design Questions

These are unresolved choices that affect the API surface before backends are written:

1. **`Modify` with multiple controls?** Currently `Modify(Control, body)` and you nest. Simpler: `Modify(Vec<Control>, body)`. Saves tree depth at a slight loss of canonical form.

2. **`Par` alignment semantics.** *Resolved (M1, ADR-003):* a `Par` lasts as long as its longest child and shorter children are padded with silence, never truncated or looped. `Music::duration()` implements this. Truncate and loop can arrive later as explicit combinators without changing the constructor.

3. **First-class voices.** `Control::Voice(VoiceId)` handles most cases. Is it enough for engraving (where "the violin part" must stay contiguous), or do we need a top-level `Score { voices: Map<VoiceId, InstrumentId> }` type?

4. **Tie semantics.** `tie_to_next: bool` on `NoteAttrs` works for simple cases. Ties across `Seq` boundaries and into `Par` branches may need a tree-rewrite pass at render time instead of a stored flag.

5. **Persistent data structures.** `Phrase` content-addresses immutable trees. Editing means rebuilding, which is fine for batch rendering but expensive for live editing of long pieces. `im::Vector` is worth exploring later.

---

## What This IR Does Not Yet Handle

Honest list of deferred scope:

- **Microtonal pitches.** Adding `Microtonal { cents: f32 }` to `PitchClass` is straightforward but propagates through every backend.
- **Continuous tempo curves.** `Tempo::Ramp` is sketched in but LilyPond mostly wants discrete markings.
- **Conditional / generative structures.** `Music::Choose(Vec<(Weight, Music)>)` or a `Composer` monad.
- **Lyrics.** Vocal music needs syllable-to-note alignment.
- **Notation-only events.** Caesuras, breath marks, double barlines: probably a `Music::Mark` variant.
- **Absolute timestamps.** The IR uses rational beats; backends convert to milliseconds. Live applications would want a separate "performed" representation.

---

## Roadmap

Work is tracked on the [MuseCode project board](https://github.com/users/Soushi888/projects/6) in three milestones:

- **M1 "Hear the tango":** `duration()`, the resolver, `Display`, structural analysis, MIDI export, and the tango example playing through `fluidsynth`.
- **M2:** LilyPond backend, parsed text syntax, the open design questions below.
- **M3:** `swing`, `humanize`, voicings, chord-symbol parser, in-process audio.

The original list, each a natural weekend project:

1. **Property tests.** Associativity of `Seq`/`Par`, identity laws, `transpose(0)` is identity, `augment(h()).diminish(h())` is identity.
2. **LilyPond backend.** Tree walk producing text. Resolve scale degrees. Handle ties, articulations, basic dynamics. Target: the Piazzolla sketch above renders to a readable PDF.
3. **Theory layer.** Chord-symbol parser (`nom` crate). Voicings as functions `Chord -> Voicing -> i8 -> Music`. Round-trip test: music to chord analysis to symbol and back.
4. **MIDI backend.** `midly` crate handles file format. Resolve pitches to MIDI numbers, durations to ticks.
5. **Tier-1 audio.** `oxisynth` + a soundfont. The feedback loop (edit, render, hear) is now closed.

---

## License

TBD.
