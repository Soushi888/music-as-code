# API Reference

Complete public API for `musecode_core`. Import everything via `use musecode_core::prelude::*`.

---

## Table of Contents

- [pitch — Pitch types and constants](#pitch)
- [time — Rational time and duration helpers](#time)
- [music — Core ADT and smart constructors](#music)
- [control — Context modifiers](#control)
- [theory — Keys, scales, and chords](#theory)
- [attrs — Note attributes and ID types](#attrs)
- [combinators — Pure transformations on Music](#combinators)
- [resolve: from a tree to resolved events](#resolve)
- [display: textual notation](#display)
- [analysis: structural facts over events](#analysis)
- [backends/hints — Backend metadata](#backendshints)
- [phrase — Content-addressed fragments](#phrase)
- [Macros](#macros)

---

## `pitch`

### Types

#### `Letter`
```rust
pub enum Letter { C, D, E, F, G, A, B }
```
Diatonic letter name. Enharmonic spelling is preserved at the IR level.

#### `Accidental`
```rust
pub enum Accidental { DoubleFlat, Flat, Natural, Sharp, DoubleSharp }
```
Chromatic alteration applied to a `Letter`.

#### `PitchClass`
```rust
pub struct PitchClass { pub letter: Letter, pub accidental: Accidental }
```
A pitch class without octave information. C# and Db are distinct `PitchClass` values even though both return `1` from `semitones()`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `semitones` | `(&self) -> i32` | Semitones above C-natural, mod 12. |
| `natural` | `(letter: Letter) -> Self` | Construct a natural (unaltered) pitch class. |

#### `ChromaticPitch`
```rust
pub struct ChromaticPitch { pub class: PitchClass, pub octave: i8 }
```
Fully resolved pitch. C4 = middle C = MIDI 60. `octave` follows scientific pitch notation (C4 is octave 4).

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(letter, accidental, octave: i8) -> Self` | Construct directly. |
| `midi` | `(&self) -> i32` | MIDI note number: `12 * (octave + 1) + semitones`. |

#### `Degree`
```rust
pub struct Degree { pub number: u8, pub alter: i8, pub octave_shift: i8 }
```
A scale degree relative to the active `Key`/`Scale` context. `number` is 1-indexed (1 = tonic). Use the `d!` macro for concise construction.

| Method | Description |
|--------|-------------|
| `new(n)` | Unaltered degree n. |
| `flat(n)` | Lowered by one semitone. |
| `sharp(n)` | Raised by one semitone. |

#### `Interval`
```rust
pub struct Interval { pub generic: i8, pub quality: IntervalQuality }
```
An interval relative to an anchor note. `generic` is signed in scale steps.

#### `IntervalQuality`
```rust
pub enum IntervalQuality {
    DoublyDiminished, Diminished, Minor, Perfect,
    Major, Augmented, DoublyAugmented
}
```

#### `Pitch`
```rust
pub enum Pitch {
    Chromatic(ChromaticPitch),
    Degree(Degree),
    Interval(Interval),
}
```
Polymorphic pitch. All three variants convert `Into<Pitch>` automatically, so any of them can be passed to `n()`. `Degree` and `Interval` variants are resolved to `ChromaticPitch` at render time using the accumulated `Control::Key` and `Control::Scale` context.

### Constants

Pre-built `ChromaticPitch` values for three octaves. Naming convention: natural notes are `C3`/`C4`/`C5`; sharps append `S` (`CS4`); flats append `B` (`DB4`, `EB4`).

**Octave 3:** `C3 D3 E3 F3 G3 A3 B3`

**Octave 4:** `C4 CS4 DB4 D4 DS4 EB4 E4 F4 FS4 GB4 G4 GS4 AB4 A4 AS4 BB4 B4`

**Octave 5:** `C5 D5 E5 F5 G5 A5 B5`

---

## `time`

### Types

#### `Beats`
```rust
pub type Beats = num_rational::Rational32;
```
All durations are exact rational numbers. Quarter note = `q() = 1/1`, eighth = `e() = 1/2`, dotted quarter = `dot(q()) = 3/2`. Arithmetic is integer-exact with no floating-point accumulation.

#### `Tempo`
```rust
pub enum Tempo {
    Fixed(u32),                                          // beats per minute
    Ramp { from_bpm: u32, to_bpm: u32, over_beats: u32 }, // linear ramp
}
```

| Constructor | Description |
|-------------|-------------|
| `Tempo::bpm(120)` | Fixed 120 BPM. |

#### `TimeSig`
```rust
pub struct TimeSig { pub numerator: u8, pub denominator: u8 }
```

| Constructor | Description |
|-------------|-------------|
| `TimeSig::new(n, d)` | Arbitrary time signature. |
| `TimeSig::common()` | 4/4 |
| `TimeSig::cut()` | 2/2 |
| `TimeSig::waltz()` | 3/4 |

#### `Dynamics`
```rust
pub enum Dynamics { Ppp, Pp, P, Mp, Mf, F, Ff, Fff, Sfz, Fp, Crescendo, Decrescendo }
```

### Duration Functions

| Function | Returns | Musical value |
|----------|---------|---------------|
| `b(num, den)` | `Beats` | Arbitrary rational: `b(3, 8)` = dotted eighth |
| `w()` | `Beats` | Whole note (4 beats) |
| `h()` | `Beats` | Half note (2 beats) |
| `q()` | `Beats` | Quarter note (1 beat) |
| `e()` | `Beats` | Eighth note (1/2 beat) |
| `s()` | `Beats` | Sixteenth note (1/4 beat) |
| `ts()` | `Beats` | Thirty-second note (1/8 beat) |
| `dot(d)` | `Beats` | Dotted: `d * 3/2` |
| `triplet(d)` | `Beats` | Triplet: `d * 2/3` |

---

## `music`

The core module. Import `use musecode_core::prelude::*` to access everything.

### Types

#### `Note`
```rust
pub struct Note {
    pub pitch: Pitch,
    pub dur: Beats,
    pub attrs: NoteAttrs,
}
```
A single sounded event. Construct via `n(pitch, dur)` rather than directly.

#### `Music`
```rust
pub enum Music {
    Note(Note),
    Rest(Beats),
    Seq(Vec<Music>),
    Par(Vec<Music>),
    Modify(Control, Box<Music>),
}
```
The entire musical universe is a tree of these five constructors.

| Method | Signature | Description |
|--------|-----------|-------------|
| `modify` | `(self, Control) -> Music` | Wrap self in `Modify`. |
| `is_empty` | `(&self) -> bool` | True for `Seq([])` or `Par([])`. |

### Operators

| Operator | Example | Effect |
|----------|---------|--------|
| `+` | `a + b` | Sequential composition. Flattens adjacent `Seq` nodes. |
| `\|` | `melody \| bass` | Parallel composition. Flattens adjacent `Par` nodes. |
| `*` | `riff * 4` | Repeat `n` times, wrapped in a `Seq`. Requires `Music: Clone`. |

**Flattening behaviour:** `a + b + c` produces `Seq([a, b, c])`, not `Seq([Seq([a, b]), c])`. Same for `|`. This keeps tree depth proportional to musical structure rather than expression nesting depth.

### Smart Constructors

```rust
pub fn n(pitch: impl Into<Pitch>, dur: Beats) -> Music
```
Create a note. Accepts `ChromaticPitch`, `Degree`, or `Interval` as pitch.

```rust
pub fn r(dur: Beats) -> Music
```
Create a rest.

```rust
pub fn chord(pitches: impl IntoIterator<Item = ChromaticPitch>, dur: Beats) -> Music
```
All pitches sounding simultaneously for `dur` beats. Returns a `Par` of notes.

### Usage Examples

```rust
use musecode_core::prelude::*;

// Single note: C4, quarter duration
let c = n(C4, q());

// Rest of one beat
let pause = r(q());

// Chord: C major triad, half note
let cmaj = chord([C4, E4, G4], h());

// Sequential chain (+ flattens into Seq)
let scale = n(C4, q()) + n(D4, q()) + n(E4, q()) + n(F4, q())
          + n(G4, q()) + n(A4, q()) + n(B4, q()) + n(C5, h());

// Two-voice parallel (| flattens into Par)
let soprano = n(G5, h()) + n(E5, h());
let alto    = n(E4, h()) + n(C4, h());
let duet    = soprano | alto;

// Repeat four times
let ostinato = (n(C3, e()) + n(G3, e())) * 4;

// Apply context
let in_key = scale
    .modify(Control::Key(Key::major(pc!(C))))
    .modify(Control::Tempo(Tempo::bpm(120)))
    .modify(Control::TimeSignature(TimeSig::common()));

// Scale-degree spelling (resolves at render time)
let modal = seq![
    n(d!(1), q()),
    n(d!(3), q()),
    n(d!(5), h()),
].modify(Control::Key(Key::minor(pc!(A))));
```

---

## `control`

### `Control`

```rust
pub enum Control {
    Tempo(Tempo),
    TimeSignature(TimeSig),
    Key(Key),
    Scale(Scale),
    Instrument(InstrumentId),
    Transpose(i32),          // chromatic semitones
    DiatonicTranspose(i32),  // scale steps in current key
    Dynamics(Dynamics),
    Articulation(Articulation),
    Voice(VoiceId),
    Hint(BackendHint),
    User(String, String),    // arbitrary key-value escape hatch
}
```

Controls nest: wrapping a piece in `Modify(Control::Key(k), body)` changes key resolution for the entire subtree. If `body` contains an inner `Modify(Control::Key(k2), inner)`, `inner` resolves against `k2`, not `k`.

---

## `theory`

### `Mode`
```rust
pub enum Mode {
    Major, Minor, Dorian, Phrygian, Lydian, Mixolydian,
    Locrian, HarmonicMinor, MelodicMinor, Custom(u32),
}
```

```rust
impl Mode {
    pub fn intervals(self) -> Option<&'static [i8]>   // None for Custom
}
```

### `Key`
```rust
pub struct Key { pub tonic: PitchClass, pub mode: Mode }
```

| Constructor | Description |
|-------------|-------------|
| `Key::new(tonic, mode)` | Arbitrary key. |
| `Key::major(pc!(C))` | C major. |
| `Key::minor(pc!(F))` | F minor. |

### `Scale`
```rust
pub struct Scale { pub intervals: Vec<i8>, pub name: Option<String> }
```
Interval content in semitones from root. Major scale = `[0, 2, 4, 5, 7, 9, 11]`.

| Constructor | Description |
|-------------|-------------|
| `Scale::major()` | Major: `[0,2,4,5,7,9,11]` |
| `Scale::natural_minor()` | Natural minor: `[0,2,3,5,7,8,10]` |
| `Scale::harmonic_minor()` | Harmonic minor: `[0,2,3,5,7,8,11]` |
| `Scale::custom(intervals)` | Any interval set. |

### `Chord`
```rust
pub struct Chord {
    pub root: PitchClass,
    pub quality: ChordQuality,
    pub extensions: Vec<Extension>,
    pub bass: Option<PitchClass>,  // slash chord
}
```

| Method | Description |
|--------|-------------|
| `Chord::new(root, quality)` | Triad or seventh, no extensions. |
| `.with_extension(ext)` | Builder: add an extension. |
| `.over(bass)` | Builder: slash chord. |

### `ChordQuality`
```rust
pub enum ChordQuality {
    Major, Minor, Dominant, MajorSeventh, MinorSeventh,
    HalfDiminished, Diminished, Augmented, Sus2, Sus4,
    Altered, Custom(String),
}
```

### `Extension`
```rust
pub struct Extension { pub degree: u8, pub alter: i8 }
```

| Constructor | Example |
|-------------|---------|
| `Extension::new(9)` | Natural ninth |
| `Extension::flat(9)` | Flat ninth |
| `Extension::sharp(11)` | Sharp eleventh |

### `Voicing`
```rust
pub enum Voicing { Root, Drop2, Drop3, Rootless, Shell, Quartal, Custom(Vec<i8>) }
```

### `PitchRange`
```rust
pub struct PitchRange { pub low: i32, pub high: i32 }  // MIDI note numbers
```

---

## `attrs`

### ID Newtypes

| Type | Inner | Description |
|------|-------|-------------|
| `VoiceId(String)` | `String` | Identifies a voice or staff. |
| `InstrumentId(String)` | `String` | Identifies an instrument patch. |
| `BackendId(String)` | `String` | Identifies a rendering backend (for `RenderCache`). |

All derive `Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize`.

### `Articulation`
```rust
pub enum Articulation {
    Staccato, Staccatissimo, Tenuto, Accent, Marcato,
    Legato, Slur, Fermata,
    Pizzicato, Arco,
    Trill, Mordent, Turn,
    HarmonicNatural, HarmonicArtificial,
}
```

### `NoteAttrs`
```rust
pub struct NoteAttrs {
    pub velocity: Option<u8>,              // 0..=127, None = use context default
    pub articulation: Option<Articulation>,
    pub tie_to_next: bool,
    pub voice_id: Option<VoiceId>,
    pub hints: Vec<BackendHint>,
}
```
Implements `Default` (all fields absent/false). Construct via `Default::default()` then set fields as needed, or use the `NoteAttrs { ..Default::default(), velocity: Some(80) }` struct update syntax.

---

## `combinators`

All methods are on `Music` directly.

### Transposition

```rust
pub fn transpose(self, semis: i32) -> Music
```
Wraps self in `Modify(Control::Transpose(semis), _)`. Resolution is deferred to render time.

```rust
pub fn diatonic_transpose(self, steps: i32) -> Music
```
Move `steps` scale steps in the current `Key`/`Scale` context. Wraps in `Modify(Control::DiatonicTranspose(steps), _)`.

### Time Scaling

```rust
pub fn augment(self, factor: Beats) -> Music
pub fn diminish(self, factor: Beats) -> Music
```
Eagerly multiply (or divide) every `Note` duration and `Rest` duration in the tree. `diminish(f)` is `augment(f.recip())`.

```rust
// Double all durations
melody.augment(h())

// Compress to two-thirds (triplet feel)
phrase.diminish(b(3, 2))
```

### Structural Transformations

```rust
pub fn retrograde(self) -> Music
```
Recursively reverses the children of every `Seq` node. Leaves `Par` ordering unchanged. `Note` and `Rest` leaves are returned as-is.

```rust
pub fn invert(self, axis_midi: i32) -> Music
```
Pitch inversion around `axis_midi`. A note at distance `d` above the axis moves to distance `d` below. Only operates on `Chromatic` pitches; `Degree` and `Interval` pitches are passed through unchanged.

### Structural Recursion Primitives

```rust
pub fn map_notes(self, f: impl Fn(Note) -> Note + Clone) -> Music
pub fn map_rests(self, f: impl Fn(Beats) -> Beats + Clone) -> Music
```
Apply a function to every leaf `Note` or `Rest` in the tree, preserving structure. These are the building blocks for custom transforms:

```rust
// Accent every note (set velocity to 100)
melody.map_notes(|mut note| { note.attrs.velocity = Some(100); note })

// Double all rest durations
phrase.map_rests(|dur| dur * Rational32::new(2, 1))
```

### Pipeline

```rust
pub fn pipe(self, f: impl FnOnce(Music) -> Music) -> Music
```
Apply any `FnOnce(Music) -> Music` inline in a method chain:

```rust
melody
    .transpose(5)
    .augment(h())
    .pipe(canon(vec![(q(), 7), (h(), 4)]))
```

### Free Functions

```rust
pub fn swing(m: Music) -> Music          // stub: todo!()
pub fn humanize(seed: u64, amount: f32) -> impl Fn(Music) -> Music  // stub: todo!()
pub fn canon(voices: Vec<(Beats, i32)>) -> impl Fn(Music) -> Music
```

`canon(voices)` returns a closure that, given a melody, builds a `Par` of that melody offset in time and transposed. Each entry in `voices` is `(time_offset, semitones)`:

```rust
// Two-voice canon: second entry enters after one quarter, a fifth above
let two_voice_canon = canon(vec![(q(), 0), (q(), 7)]);
melody.pipe(two_voice_canon)
```

---

## `resolve`

The pass that gives the polymorphic parts of the IR their meaning. Walks a `Music` tree with an accumulated context and emits one `Event` per sounding note. Backends and the analysis layer consume `Resolved`, never `Music` directly.

### Types

```rust
pub struct Event {
    pub onset: Beats,                 // absolute, from the start of the piece
    pub dur: Beats,                   // sounding duration after articulation
    pub written_dur: Beats,           // the notated value
    pub pitch: ChromaticPitch,        // resolved, spelling preserved
    pub velocity: u8,                 // 1..=127
    pub voice: Option<VoiceId>,
    pub instrument: Option<InstrumentId>,
    pub articulation: Option<Articulation>,
    pub hints: Vec<BackendHint>,      // note hints, then enclosing Control::Hint outermost first
}

pub struct Resolved {
    pub events: Vec<Event>,           // sorted by onset, stable
    pub tempo_map: Vec<(Beats, Tempo)>,
    pub time_sigs: Vec<(Beats, TimeSig)>,
    pub total: Beats,                 // == Music::duration()
}

pub enum ResolveError {
    DegreeZero,
    DegreeWithoutKey,
    CustomModeWithoutScale { mode: Mode },
    UnanchoredInterval,
    ZeroInterval,
    InvalidIntervalQuality { generic: i8, quality: IntervalQuality },
    TieMismatch { at: Beats },
    UnspellablePitch { semitone: i32, letter: Letter },
}

pub fn resolve(music: &Music) -> Result<Resolved, ResolveError>
```

### Rules

| Input | Resolution |
|---|---|
| `Seq` / `Par` / `Modify` | Running-sum onsets / shared onset / no time change |
| `Degree` | Against the key's scale (or `Control::Scale`); degree 1 is the tonic in octave 4; `alter` inflects the scale step; seven-note scales spell by letter (b5 of F minor is Cb5) |
| `Interval` | Against the previous note in the same `Seq` branch, else the tonic anchor, else `UnanchoredInterval`; `Par` children all see the `prev` from before the `Par` |
| `Control::Transpose(n)` | Applied to the resolved pitch; multiples of 12 keep the spelling, others respell with sharps |
| `Control::DiatonicTranspose(n)` | Moves `Degree` pitches by scale steps with octave borrowing; other pitches unchanged |
| `Control::Dynamics` | Velocity table: ppp 16, pp 33, p 49, mp 64, mf 80, f 96, ff 112, fff 127, sfz 120, fp 96; hairpins leave the level unchanged; a note's own `velocity` wins |
| Articulation | Staccato halves, staccatissimo quarters the sounding duration; written duration is untouched |
| `tie_to_next` | Merges with the next same-pitch note in the branch into one event; a rest or another pitch is `TieMismatch`; a tie at the end of a branch is dropped |
| `Control::Tempo` / `TimeSignature` | Appended to the maps at the onset where they are met |

```rust
use musecode_core::prelude::*;
let triad = seq![n(d!(1), q()), n(d!(3), q()), n(d!(5), h())]
    .modify(Control::Key(Key::major(pc!(C))));
let pitches: Vec<_> = resolve(&triad)?.events.iter().map(|e| e.pitch).collect();
assert_eq!(pitches, vec![C4, E4, G4]);
```

---

## `rhythm`

Rhythm without pitch. A `Pattern` is a list of `Step::Hit(Beats)` / `Step::Rest(Beats)`; pitch is applied afterwards, so one rhythm serves any line. Re-exported from the prelude.

| Item | Signature | Notes |
|------|-----------|-------|
| `Pattern::new` / `Pattern::hits` | `(impl IntoIterator<Item = Step>)` / `(impl IntoIterator<Item = Beats>)` | `hits` makes every duration a struck note |
| `steps`, `hit_count`, `duration` | accessors | `duration` sums hits and rests |
| `then`, `repeat` | `(Pattern) -> Pattern`, `(usize) -> Pattern` | concatenate, loop (`repeat(0)` is empty) |
| `on` | `(impl Into<Pitch>) -> Music` | every hit gets the pitch; rests stay rests; result is a `Seq` |
| `with` | `(impl IntoIterator<Item = impl Into<Pitch>>) -> Music` | pitches cycle over the hits; no pitches = silence of the same length |
| `tresillo()` | q. q. q (one bar of 4/4) | nuevo tango bass, 3+3+2 |
| `habanera()` | e. s e e (half a bar) | traditional tango / milonga bass |
| `cinquillo()` | e s e s e (half a bar) | five hits over four eighths |
| `straight(n, dur)` | n equal hits | |

## `display`

`impl Display for Music` prints the notation specified in [NOTATION.md](NOTATION.md): `C4:q E4:q G4:h`, `[C4 E4 G4]:h` for a chord, `{ a | b }` for a `Par`, `key(F minor) { ... }` for a `Modify`, `r:q` for a rest, `1:q b3:e` for degrees, `+M3:q` for intervals. Every public type that appears in the notation implements `Display` in its own module (`Letter`, `Accidental`, `PitchClass`, `ChromaticPitch`, `Degree`, `Interval`, `IntervalQuality`, `Pitch`, `Tempo`, `TimeSig`, `Dynamics`, `Mode`, `Key`, `Scale`, `Articulation`, `NoteAttrs`, `Control`), and `time::duration_name(Beats) -> String` names a duration (`q`, `e.`, `h..`, `q3`, else `num/den`).

```rust
use musecode_core::prelude::*;
let m = seq![n(C4, q()), chord([E4, G4], h())].modify(Control::Tempo(Tempo::bpm(120)));
assert_eq!(m.to_string(), "tempo(120) { C4:q [E4 G4]:h }");
```

---

## `analysis`

Over `&[Event]` from `resolve`:

```rust
pub fn pitch_range(events: &[Event]) -> Option<(ChromaticPitch, ChromaticPitch)>
pub fn pitch_class_set(events: &[Event]) -> PitchClassSet        // { bits: u16, classes: Vec<PitchClass> }
pub fn interval_histogram(events: &[Event]) -> BTreeMap<i32, usize>  // signed semitones -> count, per voice
pub fn vertical_slices(events: &[Event]) -> Vec<(Beats, Vec<ChromaticPitch>)>
pub fn label_triad(pitches: &[ChromaticPitch]) -> Option<Triad>  // Triad { root: PitchClass, quality: TriadQuality }
pub fn summary(music: &Music) -> Result<String, ResolveError>
```

`TriadQuality` is `Major | Minor | Diminished | Augmented | Sus2 | Sus4`; roots are tried from the lowest sounding pitch upward, so `C D G` is `C sus2` and `G C D` is `G sus4`. Roman-numeral and functional analysis are deliberately absent.

---

## `backends/hints`

### `BackendHint`
```rust
pub enum BackendHint {
    Lilypond(LilypondHint),
    Midi(MidiHint),
    Audio(AudioHint),
}
```
Attached to `NoteAttrs.hints`. A backend that doesn't understand a hint ignores it. Multiple hints of different backend types can coexist on the same note.

### `LilypondHint`
```rust
pub enum LilypondHint {
    StemUp, StemDown,
    BeamStart, BeamEnd,
    Markup(String),       // raw \markup escape
    OmitTimeSignature,
    HiddenRest,
}
```

### `MidiHint`
```rust
pub enum MidiHint {
    ProgramChange(u8),
    Channel(u8),
    ControlChange { controller: u8, value: u8 },
}
```

### `AudioHint`
```rust
pub enum AudioHint {
    KeySwitch(u8),          // MIDI note triggering an articulation patch
    Articulation(String),   // "spiccato", "col_legno", "sul_pont"
    MicPosition(MicPos),
    RoundRobin(u8),         // cycle sample variations
}
```

### `MicPos`
```rust
pub enum MicPos { Close, Mid, Far, Mix }
```

---

## `backends/midi`

The first concrete backend: a flat map from [`Resolved`](#resolve) events to a Standard MIDI File (format 1, 480 PPQ by default). Re-exported from the prelude.

| Item | Signature | Notes |
|------|-----------|-------|
| `MidiOptions` | `{ ppq: u16, default_bpm: u32, default_program: u8 }`, `Default` = 480 / 120 / 0 | `ppq` must be `1..=32767` |
| `MidiError` | `Resolve`, `Io`, `InvalidPpq`, `ZeroTempo`, `BadTimeSignature`, `PitchOutOfMidiRange`, `TickOverflow` | `Display` + `Error`; `From<ResolveError>`, `From<io::Error>` |
| `render_midi` | `(&Music, &MidiOptions) -> Result<Vec<u8>, MidiError>` | resolves, then renders |
| `render_resolved` | `(&Resolved, &MidiOptions) -> Result<Vec<u8>, MidiError>` | when you already hold a `Resolved` |
| `write_midi` | `(&Music, impl AsRef<Path>, &MidiOptions) -> Result<(), MidiError>` | `render_midi` + `std::fs::write` |
| `program_for` | `(&InstrumentId) -> Option<u8>` | GM table by name (`"bandoneon"` = 21), bare number, or `gm:` prefix; `None` falls back to `default_program` |
| `BACKEND_ID`, `backend_id()` | `"midi"` | the `RenderCache` key half |

Layout of the file: track 0 is the conductor (tempo and time-signature meta events; 120 bpm and 4/4 are written at beat 0 when the piece states neither), then one track per voice in order of first appearance, each on the next free channel (9 is skipped). A program change is written whenever the program a note wants (a `MidiHint::ProgramChange`, else its instrument, else `default_program`) differs from what its channel last received. `MidiHint::Channel` reroutes the note it is attached to; `MidiHint::ControlChange` is written just before its note. `Tempo::Ramp` contributes `from_bpm` only in M1.

Caching: `RenderCache::render_with(&phrase, backend_id(), |m| render_midi(m, &opts).unwrap())` renders once per `(hash, backend)`.

## `phrase`

### `Phrase`
```rust
pub struct Phrase { /* inner: Arc<Music>, hash: blake3::Hash */ }
```
A reference-counted, content-hashed musical fragment.

| Method | Description |
|--------|-------------|
| `Phrase::new(m: Music)` | Serialize `m` with `bincode`, hash with Blake3, wrap in `Arc`. |
| `.hash() -> &blake3::Hash` | The content hash. |
| `.music() -> &Music` | Borrow the inner `Music`. |

Two `Phrase`s are equal if and only if their hashes are equal (`PartialEq` delegates to hash comparison).

### `RenderCache<O>`

Also `render_with(&self, &Phrase, BackendId, impl FnOnce(&Music) -> O)`: returns the cached output for the pair, rendering and storing it on a miss.
```rust
pub struct RenderCache<O> { /* DashMap<(blake3::Hash, BackendId), O> */ }
```
Concurrent map from `(content_hash, backend_id)` to rendered output `O`.

| Method | Description |
|--------|-------------|
| `RenderCache::new()` | Empty cache. |
| `.get(hash, backend)` | Look up cached output. Returns a `dashmap` read guard. |
| `.insert(hash, backend, output)` | Store rendered output. |

---

## Macros

### `seq![ ... ]`
```rust
seq![n(C4, q()), n(E4, q()), n(G4, h())]
// expands to: Music::Seq(vec![...])
```

### `par![ ... ]`
```rust
par![melody, bass, inner_voice]
// expands to: Music::Par(vec![...])
```

### `d!( ... )`
```rust
d!(1)      // Degree { number: 1, alter: 0, octave_shift: 0 }  — tonic
d!(b 3)    // Degree { number: 3, alter: -1, ... }             — flat third
d!(# 7)    // Degree { number: 7, alter: 1, ... }              — sharp seventh
```

### `pc!( ... )`
```rust
pc!(F)     // PitchClass { letter: Letter::F, accidental: Accidental::Natural }
pc!(C)     // PitchClass for C natural
```
Intended for use with `Key::new(pc!(Bb), Mode::Major)` and `Chord::new(pc!(G), ChordQuality::Dominant)`.
