//! Music theory layer: keys, scales, modes, chords, and voicings.
//!
//! These types let you write music in terms of harmonic intent
//! (`Key::minor(pc!(F))`, `ChordQuality::HalfDiminished`) rather than
//! raw chromatic pitches. Resolution to [`ChromaticPitch`][crate::pitch::ChromaticPitch]
//! happens at render time via the accumulated `Control` context.

use serde::{Deserialize, Serialize};

use crate::pitch::PitchClass;

/// Modal or tonal scale type.
///
/// Determines the interval pattern used to resolve [`Degree`][crate::pitch::Degree]
/// pitches within a [`Key`] context.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Mode {
    /// Ionian mode: W-W-H-W-W-W-H. The standard major scale.
    Major,
    /// Aeolian / natural minor: W-H-W-W-H-W-W.
    Minor,
    /// Dorian mode: W-H-W-W-W-H-W. Minor with a raised 6th (common in jazz/folk).
    Dorian,
    /// Phrygian mode: H-W-W-W-H-W-W. Minor with a lowered 2nd (flamenco, metal).
    Phrygian,
    /// Lydian mode: W-W-W-H-W-W-H. Major with a raised 4th (dreamy, bright).
    Lydian,
    /// Mixolydian mode: W-W-H-W-W-H-W. Major with a lowered 7th (blues, rock).
    Mixolydian,
    /// Aeolian mode (natural minor): same as [`Mode::Minor`].
    Aeolian,
    /// Locrian mode: H-W-W-H-W-W-W. Diminished tonic triad; rare in practice.
    Locrian,
    /// Harmonic minor: natural minor with a raised 7th, creating a leading tone.
    HarmonicMinor,
    /// Melodic minor (ascending): natural minor with raised 6th and 7th.
    MelodicMinor,
    /// User-defined mode referenced by a numeric ID. Useful for gamelan, microtonal,
    /// or other non-Western scale systems stored externally.
    Custom(u32),
}

/// A tonal centre: tonic pitch class plus mode.
///
/// Used in [`Control::Key`][crate::control::Control::Key] to establish the
/// harmonic context for resolving [`Degree`][crate::pitch::Degree] pitches.
///
/// # Examples
/// ```
/// use muse_core::prelude::*;
/// let f_minor  = Key::minor(pc!(F));
/// let c_lydian = Key::new(pc!(C), Mode::Lydian);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Key {
    /// The tonic pitch class (root of the scale).
    pub tonic: PitchClass,
    /// The modal/scale type that determines the interval pattern.
    pub mode: Mode,
}

impl Key {
    /// Construct a key from an explicit tonic and mode.
    pub fn new(tonic: PitchClass, mode: Mode) -> Self {
        Self { tonic, mode }
    }

    /// Shorthand for a major key: `Key::new(tonic, Mode::Major)`.
    pub fn major(tonic: PitchClass) -> Self {
        Self { tonic, mode: Mode::Major }
    }

    /// Shorthand for a natural minor key: `Key::new(tonic, Mode::Minor)`.
    pub fn minor(tonic: PitchClass) -> Self {
        Self { tonic, mode: Mode::Minor }
    }
}

/// An explicit scale defined by its interval content.
///
/// Used in [`Control::Scale`][crate::control::Control::Scale] to override
/// the default scale derived from a [`Key`]. Useful for exotic, synthetic,
/// or non-Western scales not covered by [`Mode`].
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Scale {
    /// Semitones above the root for each scale degree, starting with `0`.
    /// Example: major scale = `[0, 2, 4, 5, 7, 9, 11]`.
    pub intervals: Vec<i8>,
    /// Optional human-readable name (used for display and debugging only).
    pub name: Option<String>,
}

impl Scale {
    /// Major scale: `[0, 2, 4, 5, 7, 9, 11]`.
    pub fn major() -> Self {
        Self { intervals: vec![0, 2, 4, 5, 7, 9, 11], name: Some("major".into()) }
    }

    /// Natural minor scale: `[0, 2, 3, 5, 7, 8, 10]`.
    pub fn natural_minor() -> Self {
        Self { intervals: vec![0, 2, 3, 5, 7, 8, 10], name: Some("natural_minor".into()) }
    }

    /// Harmonic minor scale: `[0, 2, 3, 5, 7, 8, 11]`.
    pub fn harmonic_minor() -> Self {
        Self { intervals: vec![0, 2, 3, 5, 7, 8, 11], name: Some("harmonic_minor".into()) }
    }

    /// Construct a scale from an arbitrary interval vector. `name` will be `None`.
    pub fn custom(intervals: Vec<i8>) -> Self {
        Self { intervals, name: None }
    }
}

/// The harmonic quality of a chord, used in [`Chord`].
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ChordQuality {
    /// Major triad: root, M3, P5.
    Major,
    /// Minor triad: root, m3, P5.
    Minor,
    /// Dominant seventh: root, M3, P5, m7.
    Dominant,
    /// Major seventh: root, M3, P5, M7.
    MajorSeventh,
    /// Minor seventh: root, m3, P5, m7.
    MinorSeventh,
    /// Half-diminished (minor 7 flat 5): root, m3, d5, m7.
    HalfDiminished,
    /// Fully diminished seventh: root, m3, d5, d7.
    Diminished,
    /// Augmented triad: root, M3, A5.
    Augmented,
    /// Suspended 2nd: root, M2, P5 (no third).
    Sus2,
    /// Suspended 4th: root, P4, P5 (no third).
    Sus4,
    /// Altered dominant: root, M3, and any combination of b5/s5/b9/s9.
    Altered,
    /// User-defined quality described by a string (for parsing, display, or
    /// qualities not covered by the typed variants).
    Custom(String),
}

/// A chord extension beyond the basic triad or seventh.
///
/// Extensions are added to a [`Chord`] via [`Chord::with_extension`].
/// Common examples: natural 9th (`Extension::new(9)`), sharp 11th
/// (`Extension::sharp(11)`), flat 13th (`Extension::flat(13)`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Extension {
    /// The extension degree number: typically 9, 11, or 13.
    pub degree: u8,
    /// Chromatic alteration: `-1` = flat, `0` = natural, `+1` = sharp.
    pub alter: i8,
}

impl Extension {
    /// Unaltered extension degree (e.g. `Extension::new(9)` = natural ninth).
    pub fn new(degree: u8) -> Self {
        Self { degree, alter: 0 }
    }

    /// Flattened extension (e.g. `Extension::flat(9)` = b9, `Extension::flat(13)` = b13).
    pub fn flat(degree: u8) -> Self {
        Self { degree, alter: -1 }
    }

    /// Sharpened extension (e.g. `Extension::sharp(11)` = #11, Lydian dominant color).
    pub fn sharp(degree: u8) -> Self {
        Self { degree, alter: 1 }
    }
}

/// A chord symbol: root, quality, extensions, and optional bass note.
///
/// Chord symbols can be used to generate voiced [`Music`][crate::music::Music]
/// trees via a voicing function (pending implementation in `combinators`).
///
/// # Examples
/// ```
/// use muse_core::prelude::*;
/// // Cmaj7#11
/// let chord = Chord::new(pc!(C), ChordQuality::MajorSeventh)
///     .with_extension(Extension::sharp(11));
/// // G7/B (slash chord)
/// let slash = Chord::new(pc!(G), ChordQuality::Dominant)
///     .over(pc!(B));
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Chord {
    /// The root pitch class of the chord.
    pub root: PitchClass,
    /// The basic quality (triad + seventh type).
    pub quality: ChordQuality,
    /// Additional extensions beyond the seventh (9, 11, 13 with alterations).
    pub extensions: Vec<Extension>,
    /// Optional bass note for slash chords (e.g. G7/B). `None` means root in bass.
    pub bass: Option<PitchClass>,
}

impl Chord {
    /// Construct a chord with no extensions and root in bass.
    pub fn new(root: PitchClass, quality: ChordQuality) -> Self {
        Self { root, quality, extensions: vec![], bass: None }
    }

    /// Builder: add an extension and return the modified chord.
    ///
    /// Can be chained: `.with_extension(Extension::sharp(11)).with_extension(Extension::flat(13))`.
    pub fn with_extension(mut self, ext: Extension) -> Self {
        self.extensions.push(ext);
        self
    }

    /// Builder: set a slash-chord bass note and return the modified chord.
    ///
    /// `Chord::new(pc!(G), ChordQuality::Dominant).over(pc!(B))` = G7/B.
    pub fn over(mut self, bass: PitchClass) -> Self {
        self.bass = Some(bass);
        self
    }
}

/// A chord voicing strategy, describing how pitches are distributed across octaves.
///
/// Passed to voicing functions (pending implementation) that convert a [`Chord`]
/// symbol into a concrete [`Music`][crate::music::Music] tree.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Voicing {
    /// Root position, close voicing: pitches stacked as close together as possible.
    Root,
    /// Drop-2 voicing: second voice from the top moved down an octave.
    Drop2,
    /// Drop-3 voicing: third voice from the top moved down an octave.
    Drop3,
    /// Rootless voicing: omit the root (common in jazz piano comping).
    Rootless,
    /// Shell voicing: root, third, and seventh only.
    Shell,
    /// Quartal voicing: pitches stacked in fourths.
    Quartal,
    /// Explicit semitone offsets from the root, allowing fully custom voicings.
    Custom(Vec<i8>),
}

/// A range of pitches expressed as MIDI note numbers.
///
/// Used to constrain voicing algorithms to the practical range of an instrument.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PitchRange {
    /// Lowest pitch of the range (inclusive), as a MIDI note number.
    pub low: i32,
    /// Highest pitch of the range (inclusive), as a MIDI note number.
    pub high: i32,
}

impl PitchRange {
    /// Construct a pitch range from MIDI note numbers.
    ///
    /// `PitchRange::new(40, 88)` covers the practical range of a cello.
    pub fn new(low: i32, high: i32) -> Self {
        Self { low, high }
    }
}
