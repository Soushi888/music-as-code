//! Pitch representation: letters, accidentals, chromatic pitches, and the
//! polymorphic [`Pitch`] type that lets callers write music in chromatic,
//! scale-degree, or interval-relative form, all resolving to the same
//! chromatic output via a `Key`/`Scale` context.

use serde::{Deserialize, Serialize};

/// The diatonic letter name of a pitch. Enharmonic spelling is preserved.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Letter {
    /// C — do
    C,
    /// D — re
    D,
    /// E — mi
    E,
    /// F — fa
    F,
    /// G — sol
    G,
    /// A — la
    A,
    /// B — si/ti
    B,
}

/// Chromatic alteration applied to a [`Letter`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Accidental {
    /// Two semitones below natural (𝄫).
    DoubleFlat,
    /// One semitone below natural (♭).
    Flat,
    /// Unaltered (♮).
    Natural,
    /// One semitone above natural (♯).
    Sharp,
    /// Two semitones above natural (𝄪).
    DoubleSharp,
}

/// A pitch class: letter + accidental, without octave information.
///
/// Enharmonic spellings are distinct: C# and Db are different `PitchClass`
/// values even though [`semitones`][Self::semitones] returns the same integer
/// for both. This preserves the composer's harmonic intent through the IR.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PitchClass {
    /// Diatonic letter name.
    pub letter: Letter,
    /// Chromatic alteration relative to the natural.
    pub accidental: Accidental,
}

impl PitchClass {
    /// Returns the number of semitones above C-natural, in the range `0..=11`.
    ///
    /// Enharmonically equivalent pitch classes (e.g. C# and Db) return the
    /// same value. The result wraps correctly: `B#` returns `0` (= C).
    ///
    /// # Examples
    /// ```
    /// use musecode_core::prelude::*;
    /// assert_eq!(pc!(C).semitones(), 0);
    /// assert_eq!(pc!(A).semitones(), 9);
    /// ```
    pub fn semitones(&self) -> i32 {
        let base: i32 = match self.letter {
            Letter::C => 0,
            Letter::D => 2,
            Letter::E => 4,
            Letter::F => 5,
            Letter::G => 7,
            Letter::A => 9,
            Letter::B => 11,
        };
        let alter: i32 = match self.accidental {
            Accidental::DoubleFlat => -2,
            Accidental::Flat => -1,
            Accidental::Natural => 0,
            Accidental::Sharp => 1,
            Accidental::DoubleSharp => 2,
        };
        (base + alter).rem_euclid(12)
    }

    /// Construct an unaltered (natural) pitch class from a letter name.
    ///
    /// Equivalent to `PitchClass { letter, accidental: Accidental::Natural }`.
    pub fn natural(letter: Letter) -> Self {
        Self { letter, accidental: Accidental::Natural }
    }
}

/// A fully-resolved pitch with octave information.
///
/// C4 is middle C (MIDI note 60). Octaves follow scientific pitch notation:
/// the octave number increments at C, so B3 (MIDI 47) is directly below C4
/// (MIDI 48).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ChromaticPitch {
    /// The pitch class (letter + accidental). Enharmonic spelling preserved.
    pub class: PitchClass,
    /// Octave number in scientific pitch notation. C4 = middle C.
    pub octave: i8,
}

impl ChromaticPitch {
    /// Construct a chromatic pitch from its parts.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::pitch::{ChromaticPitch, Letter, Accidental};
    /// let middle_c = ChromaticPitch::new(Letter::C, Accidental::Natural, 4);
    /// assert_eq!(middle_c.midi(), 60);
    /// ```
    pub fn new(letter: Letter, accidental: Accidental, octave: i8) -> Self {
        Self { class: PitchClass { letter, accidental }, octave }
    }

    /// Returns the MIDI note number for this pitch.
    ///
    /// C4 = 60. The formula is `12 * (octave + 1) + semitones_above_c`.
    /// Valid MIDI range is 0–127; values outside this range are not clamped.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::pitch::C4;
    /// assert_eq!(C4.midi(), 60);
    /// ```
    pub fn midi(&self) -> i32 {
        12 * (self.octave as i32 + 1) + self.class.semitones()
    }

    /// Construct a pitch from a MIDI note number, spelled with sharps.
    ///
    /// The inverse of [`midi`][Self::midi]: `from_midi(m).midi() == m` for every
    /// `m`, including values outside `0..=127` (negative numbers land in
    /// octave -2 and below). Black keys are spelled with sharps, so MIDI 61 is
    /// C#4, never Db4; callers that need a key-aware spelling resolve through
    /// the scale instead.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::pitch::{ChromaticPitch, Accidental, Letter, C4, CS4};
    /// assert_eq!(ChromaticPitch::from_midi(60), C4);
    /// assert_eq!(ChromaticPitch::from_midi(61), CS4);
    /// assert_eq!(ChromaticPitch::from_midi(-1), ChromaticPitch::new(Letter::B, Accidental::Natural, -2));
    /// ```
    pub fn from_midi(midi: i32) -> Self {
        let octave = midi.div_euclid(12) - 1;
        let (letter, accidental) = match midi.rem_euclid(12) {
            0 => (Letter::C, Accidental::Natural),
            1 => (Letter::C, Accidental::Sharp),
            2 => (Letter::D, Accidental::Natural),
            3 => (Letter::D, Accidental::Sharp),
            4 => (Letter::E, Accidental::Natural),
            5 => (Letter::F, Accidental::Natural),
            6 => (Letter::F, Accidental::Sharp),
            7 => (Letter::G, Accidental::Natural),
            8 => (Letter::G, Accidental::Sharp),
            9 => (Letter::A, Accidental::Natural),
            10 => (Letter::A, Accidental::Sharp),
            _ => (Letter::B, Accidental::Natural),
        };
        Self { class: PitchClass { letter, accidental }, octave: octave as i8 }
    }
}

/// A scale degree relative to the active [`Key`][crate::theory::Key] context.
///
/// `Degree` pitches are unresolved until a backend encounters a `Control::Key`
/// (and optionally `Control::Scale`) modifier. This lets you write melodies
/// that automatically follow a key change.
///
/// Prefer the [`d!`][crate::d] macro for concise construction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Degree {
    /// Scale degree number, 1-indexed (1 = tonic, 5 = dominant, 7 = leading tone).
    /// Extensions are supported: 9, 11, 13.
    pub number: u8,
    /// Chromatic alteration in semitones. `-1` = flat, `+1` = sharp, `0` = unaltered.
    pub alter: i8,
    /// Shift up or down by this many octaves after resolving the degree.
    pub octave_shift: i8,
}

impl Degree {
    /// Unaltered scale degree `n`. Equivalent to `d!(n)`.
    pub fn new(number: u8) -> Self {
        Self { number, alter: 0, octave_shift: 0 }
    }

    /// Lowered scale degree (b`n`). Equivalent to `d!(b n)`.
    pub fn flat(number: u8) -> Self {
        Self { number, alter: -1, octave_shift: 0 }
    }

    /// Raised scale degree (#`n`). Equivalent to `d!(# n)`.
    pub fn sharp(number: u8) -> Self {
        Self { number, alter: 1, octave_shift: 0 }
    }
}

/// An interval relative to an anchor note (typically the previous note).
///
/// Used when writing music in a contour-relative style rather than
/// specifying absolute pitches or scale degrees.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Interval {
    /// Signed generic span in scale steps. Positive = ascending, negative = descending.
    /// `1` = unison, `2` = second, `5` = fifth, `-3` = descending third.
    pub generic: i8,
    /// The quality of the interval (major, minor, perfect, etc.).
    pub quality: IntervalQuality,
}

/// Quality of a musical interval.
///
/// Combined with the generic size in [`Interval`] to fully specify an interval.
/// Example: `(5, Perfect)` = P5, `(3, Major)` = M3, `(7, Minor)` = m7.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum IntervalQuality {
    /// Doubly diminished (two semitones below diminished).
    DoublyDiminished,
    /// Diminished (one semitone below minor/perfect).
    Diminished,
    /// Minor (applies to 2nds, 3rds, 6ths, 7ths).
    Minor,
    /// Perfect (applies to unisons, 4ths, 5ths, octaves).
    Perfect,
    /// Major (applies to 2nds, 3rds, 6ths, 7ths).
    Major,
    /// Augmented (one semitone above major/perfect).
    Augmented,
    /// Doubly augmented (two semitones above major/perfect).
    DoublyAugmented,
}

/// Polymorphic pitch: chromatic, scale-degree, or interval-relative.
///
/// All three variants accept `Into<Pitch>` conversions and can be passed
/// directly to [`n()`][crate::music::n]. `Degree` and `Interval` are resolved
/// to [`ChromaticPitch`] at render time using the accumulated `Control` context.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// // All three style produce the same chromatic output in C major
/// let chromatic  = n(C4,    q());    // explicit
/// let by_degree  = n(d!(1), q());    // resolves to C in C major
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Pitch {
    /// An explicitly-spelled, fully-resolved chromatic pitch.
    Chromatic(ChromaticPitch),
    /// A scale degree resolved against the enclosing `Control::Key` context.
    Degree(Degree),
    /// An interval relative to the previous note, resolved at render time.
    Interval(Interval),
}

impl From<ChromaticPitch> for Pitch {
    fn from(p: ChromaticPitch) -> Self {
        Pitch::Chromatic(p)
    }
}

impl From<Degree> for Pitch {
    fn from(d: Degree) -> Self {
        Pitch::Degree(d)
    }
}

impl From<Interval> for Pitch {
    fn from(i: Interval) -> Self {
        Pitch::Interval(i)
    }
}

// === Chromatic pitch constants ===

/// C4 — middle C, MIDI 60.
pub const C4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::C, accidental: Accidental::Natural }, octave: 4 };
/// C#4 / enharmonic Db4, MIDI 61.
pub const CS4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::C, accidental: Accidental::Sharp }, octave: 4 };
/// Db4 / enharmonic C#4, MIDI 61.
pub const DB4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::D, accidental: Accidental::Flat }, octave: 4 };
/// D4, MIDI 62.
pub const D4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::D, accidental: Accidental::Natural }, octave: 4 };
/// D#4 / enharmonic Eb4, MIDI 63.
pub const DS4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::D, accidental: Accidental::Sharp }, octave: 4 };
/// Eb4 / enharmonic D#4, MIDI 63.
pub const EB4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::E, accidental: Accidental::Flat }, octave: 4 };
/// E4, MIDI 64.
pub const E4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::E, accidental: Accidental::Natural }, octave: 4 };
/// F4, MIDI 65.
pub const F4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::F, accidental: Accidental::Natural }, octave: 4 };
/// F#4 / enharmonic Gb4, MIDI 66.
pub const FS4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::F, accidental: Accidental::Sharp }, octave: 4 };
/// Gb4 / enharmonic F#4, MIDI 66.
pub const GB4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::G, accidental: Accidental::Flat }, octave: 4 };
/// G4, MIDI 67.
pub const G4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::G, accidental: Accidental::Natural }, octave: 4 };
/// G#4 / enharmonic Ab4, MIDI 68.
pub const GS4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::G, accidental: Accidental::Sharp }, octave: 4 };
/// Ab4 / enharmonic G#4, MIDI 68.
pub const AB4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::A, accidental: Accidental::Flat }, octave: 4 };
/// A4 — concert A, 440 Hz, MIDI 69.
pub const A4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::A, accidental: Accidental::Natural }, octave: 4 };
/// A#4 / enharmonic Bb4, MIDI 70.
pub const AS4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::A, accidental: Accidental::Sharp }, octave: 4 };
/// Bb4 / enharmonic A#4, MIDI 70.
pub const BB4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::B, accidental: Accidental::Flat }, octave: 4 };
/// B4, MIDI 71.
pub const B4: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::B, accidental: Accidental::Natural }, octave: 4 };

/// C3, MIDI 48.
pub const C3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::C, accidental: Accidental::Natural }, octave: 3 };
/// D3, MIDI 50.
pub const D3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::D, accidental: Accidental::Natural }, octave: 3 };
/// E3, MIDI 52.
pub const E3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::E, accidental: Accidental::Natural }, octave: 3 };
/// F3, MIDI 53.
pub const F3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::F, accidental: Accidental::Natural }, octave: 3 };
/// G3, MIDI 55.
pub const G3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::G, accidental: Accidental::Natural }, octave: 3 };
/// A3, MIDI 57.
pub const A3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::A, accidental: Accidental::Natural }, octave: 3 };
/// B3, MIDI 59.
pub const B3: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::B, accidental: Accidental::Natural }, octave: 3 };

/// C5, MIDI 72.
pub const C5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::C, accidental: Accidental::Natural }, octave: 5 };
/// D5, MIDI 74.
pub const D5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::D, accidental: Accidental::Natural }, octave: 5 };
/// E5, MIDI 76.
pub const E5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::E, accidental: Accidental::Natural }, octave: 5 };
/// F5, MIDI 77.
pub const F5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::F, accidental: Accidental::Natural }, octave: 5 };
/// G5, MIDI 79.
pub const G5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::G, accidental: Accidental::Natural }, octave: 5 };
/// A5, MIDI 81.
pub const A5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::A, accidental: Accidental::Natural }, octave: 5 };
/// B5, MIDI 83.
pub const B5: ChromaticPitch = ChromaticPitch { class: PitchClass { letter: Letter::B, accidental: Accidental::Natural }, octave: 5 };

/// Construct a natural [`PitchClass`] from a letter token.
///
/// `pc!(F)` expands to `PitchClass::natural(Letter::F)`. Useful for constructing
/// keys and chords: `Key::minor(pc!(F))`, `Chord::new(pc!(G), ChordQuality::Dominant)`.
#[macro_export]
macro_rules! pc {
    (C) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::C) };
    (D) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::D) };
    (E) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::E) };
    (F) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::F) };
    (G) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::G) };
    (A) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::A) };
    (B) => { $crate::pitch::PitchClass::natural($crate::pitch::Letter::B) };
}

/// Construct a [`Degree`] from a degree number with optional accidental.
///
/// - `d!(1)` — tonic (unaltered degree 1)
/// - `d!(b 3)` — flat third (modal mixture, minor third in a major context)
/// - `d!(# 7)` — raised seventh (leading tone in harmonic minor)
#[macro_export]
macro_rules! d {
    ($n:literal) => {
        $crate::pitch::Degree { number: $n, alter: 0, octave_shift: 0 }
    };
    (b $n:literal) => {
        $crate::pitch::Degree { number: $n, alter: -1, octave_shift: 0 }
    };
    (# $n:literal) => {
        $crate::pitch::Degree { number: $n, alter: 1, octave_shift: 0 }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_midi_round_trips_over_the_extended_range() {
        for m in -24..=150 {
            let p = ChromaticPitch::from_midi(m);
            assert_eq!(p.midi(), m, "midi {m} became {p:?}");
            assert_ne!(p.class.accidental, Accidental::Flat, "from_midi spells with sharps only");
        }
    }

    #[test]
    fn from_midi_matches_the_named_constants() {
        assert_eq!(ChromaticPitch::from_midi(48), C3);
        assert_eq!(ChromaticPitch::from_midi(59), B3);
        assert_eq!(ChromaticPitch::from_midi(60), C4);
        assert_eq!(ChromaticPitch::from_midi(66), FS4);
        assert_eq!(ChromaticPitch::from_midi(83), B5);
    }
}
