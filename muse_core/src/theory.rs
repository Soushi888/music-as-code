//! Music theory layer: keys, scales, modes, chords, and voicings.
//!
//! These types let you write music in terms of harmonic intent
//! (`Key::minor(pc!(F))`, `ChordQuality::HalfDiminished`) rather than
//! raw chromatic pitches. Resolution to [`ChromaticPitch`][crate::pitch::ChromaticPitch]
//! happens at render time via the accumulated `Control` context.

use serde::{Deserialize, Serialize};

use crate::pitch::PitchClass;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Mode {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    HarmonicMinor,
    MelodicMinor,
    Custom(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Key {
    pub tonic: PitchClass,
    pub mode: Mode,
}

impl Key {
    pub fn new(tonic: PitchClass, mode: Mode) -> Self {
        Self { tonic, mode }
    }

    pub fn major(tonic: PitchClass) -> Self {
        Self {
            tonic,
            mode: Mode::Major,
        }
    }

    pub fn minor(tonic: PitchClass) -> Self {
        Self {
            tonic,
            mode: Mode::Minor,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Scale {
    pub intervals: Vec<i8>,
    pub name: Option<String>,
}

impl Scale {
    pub fn major() -> Self {
        Self {
            intervals: vec![0, 2, 4, 5, 7, 9, 11],
            name: Some("major".to_string()),
        }
    }

    pub fn natural_minor() -> Self {
        Self {
            intervals: vec![0, 2, 3, 5, 7, 8, 10],
            name: Some("natural_minor".to_string()),
        }
    }

    pub fn harmonic_minor() -> Self {
        Self {
            intervals: vec![0, 2, 3, 5, 7, 8, 11],
            name: Some("harmonic_minor".to_string()),
        }
    }

    pub fn custom(intervals: Vec<i8>) -> Self {
        Self {
            intervals,
            name: None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ChordQuality {
    Major,
    Minor,
    Dominant,
    MajorSeventh,
    MinorSeventh,
    HalfDiminished,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Altered,
    Custom(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Extension {
    pub degree: u8,
    pub alter: i8,
}

impl Extension {
    pub fn new(degree: u8) -> Self {
        Self { degree, alter: 0 }
    }
    pub fn flat(degree: u8) -> Self {
        Self { degree, alter: -1 }
    }
    pub fn sharp(degree: u8) -> Self {
        Self { degree, alter: 1 }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Chord {
    pub root: PitchClass,
    pub quality: ChordQuality,
    pub extensions: Vec<Extension>,
    pub bass: Option<PitchClass>,
}

impl Chord {
    pub fn new(root: PitchClass, quality: ChordQuality) -> Self {
        Self {
            root,
            quality,
            extensions: vec![],
            bass: None,
        }
    }

    pub fn with_extension(mut self, ext: Extension) -> Self {
        self.extensions.push(ext);
        self
    }

    pub fn over(mut self, bass: PitchClass) -> Self {
        self.bass = Some(bass);
        self
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Voicing {
    Root,
    Drop2,
    Drop3,
    Rootless,
    Shell,
    Quartal,
    Custom(Vec<i8>),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PitchRange {
    pub low: i32,
    pub high: i32,
}

impl PitchRange {
    pub fn new(low: i32, high: i32) -> Self {
        Self { low, high }
    }
}
