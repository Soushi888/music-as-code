//! Pitch representation: letters, accidentals, chromatic pitches, and the
//! polymorphic [`Pitch`] type that lets callers write music in chromatic,
//! scale-degree, or interval-relative form — all resolving to the same
//! chromatic output via a `Key`/`Scale` context.

use serde::{Deserialize, Serialize};

/// The diatonic letter name of a pitch. Enharmonic spelling is preserved.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Letter {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

/// Chromatic alteration applied to a [`Letter`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Accidental {
    DoubleFlat,
    Flat,
    Natural,
    Sharp,
    DoubleSharp,
}

/// A pitch class: letter + accidental, without octave information.
/// Enharmonic spellings are distinct: C# and Db are different `PitchClass` values
/// even though they map to the same MIDI semitone.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PitchClass {
    pub letter: Letter,
    pub accidental: Accidental,
}

impl PitchClass {
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

    pub fn natural(letter: Letter) -> Self {
        Self {
            letter,
            accidental: Accidental::Natural,
        }
    }
}

/// Fully-resolved pitch. C4 = middle C, MIDI 60.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ChromaticPitch {
    pub class: PitchClass,
    pub octave: i8,
}

impl ChromaticPitch {
    pub fn new(letter: Letter, accidental: Accidental, octave: i8) -> Self {
        Self {
            class: PitchClass { letter, accidental },
            octave,
        }
    }

    pub fn midi(&self) -> i32 {
        12 * (self.octave as i32 + 1) + self.class.semitones()
    }
}

/// A scale degree in some scale context. Needs `Key` + `Scale` to resolve.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Degree {
    pub number: u8,
    pub alter: i8,
    pub octave_shift: i8,
}

impl Degree {
    pub fn new(number: u8) -> Self {
        Self {
            number,
            alter: 0,
            octave_shift: 0,
        }
    }

    pub fn flat(number: u8) -> Self {
        Self {
            number,
            alter: -1,
            octave_shift: 0,
        }
    }

    pub fn sharp(number: u8) -> Self {
        Self {
            number,
            alter: 1,
            octave_shift: 0,
        }
    }
}

/// Interval relative to an anchor.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Interval {
    pub generic: i8,
    pub quality: IntervalQuality,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum IntervalQuality {
    DoublyDiminished,
    Diminished,
    Minor,
    Perfect,
    Major,
    Augmented,
    DoublyAugmented,
}

/// Polymorphic pitch. Resolves to ChromaticPitch via context.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Pitch {
    Chromatic(ChromaticPitch),
    Degree(Degree),
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
// Octave 4 (middle octave)
pub const C4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::C,
        accidental: Accidental::Natural,
    },
    octave: 4,
};
pub const CS4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::C,
        accidental: Accidental::Sharp,
    },
    octave: 4,
};
pub const DB4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::D,
        accidental: Accidental::Flat,
    },
    octave: 4,
};
pub const D4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::D,
        accidental: Accidental::Natural,
    },
    octave: 4,
};
pub const DS4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::D,
        accidental: Accidental::Sharp,
    },
    octave: 4,
};
pub const EB4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::E,
        accidental: Accidental::Flat,
    },
    octave: 4,
};
pub const E4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::E,
        accidental: Accidental::Natural,
    },
    octave: 4,
};
pub const F4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::F,
        accidental: Accidental::Natural,
    },
    octave: 4,
};
pub const FS4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::F,
        accidental: Accidental::Sharp,
    },
    octave: 4,
};
pub const GB4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::G,
        accidental: Accidental::Flat,
    },
    octave: 4,
};
pub const G4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::G,
        accidental: Accidental::Natural,
    },
    octave: 4,
};
pub const GS4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::G,
        accidental: Accidental::Sharp,
    },
    octave: 4,
};
pub const AB4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::A,
        accidental: Accidental::Flat,
    },
    octave: 4,
};
pub const A4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::A,
        accidental: Accidental::Natural,
    },
    octave: 4,
};
pub const AS4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::A,
        accidental: Accidental::Sharp,
    },
    octave: 4,
};
pub const BB4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::B,
        accidental: Accidental::Flat,
    },
    octave: 4,
};
pub const B4: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::B,
        accidental: Accidental::Natural,
    },
    octave: 4,
};

// Octave 3
pub const C3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::C,
        accidental: Accidental::Natural,
    },
    octave: 3,
};
pub const D3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::D,
        accidental: Accidental::Natural,
    },
    octave: 3,
};
pub const E3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::E,
        accidental: Accidental::Natural,
    },
    octave: 3,
};
pub const F3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::F,
        accidental: Accidental::Natural,
    },
    octave: 3,
};
pub const G3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::G,
        accidental: Accidental::Natural,
    },
    octave: 3,
};
pub const A3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::A,
        accidental: Accidental::Natural,
    },
    octave: 3,
};
pub const B3: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::B,
        accidental: Accidental::Natural,
    },
    octave: 3,
};

// Octave 5
pub const C5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::C,
        accidental: Accidental::Natural,
    },
    octave: 5,
};
pub const D5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::D,
        accidental: Accidental::Natural,
    },
    octave: 5,
};
pub const E5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::E,
        accidental: Accidental::Natural,
    },
    octave: 5,
};
pub const F5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::F,
        accidental: Accidental::Natural,
    },
    octave: 5,
};
pub const G5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::G,
        accidental: Accidental::Natural,
    },
    octave: 5,
};
pub const A5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::A,
        accidental: Accidental::Natural,
    },
    octave: 5,
};
pub const B5: ChromaticPitch = ChromaticPitch {
    class: PitchClass {
        letter: Letter::B,
        accidental: Accidental::Natural,
    },
    octave: 5,
};

// Macros for concise pitch construction
#[macro_export]
macro_rules! pc {
    (C) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::C)
    };
    (D) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::D)
    };
    (E) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::E)
    };
    (F) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::F)
    };
    (G) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::G)
    };
    (A) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::A)
    };
    (B) => {
        $crate::pitch::PitchClass::natural($crate::pitch::Letter::B)
    };
}

/// Scale degree macro: `d!(1)` = tonic, `d!(b 3)` = flat third, `d!(#7)` = raised seventh
#[macro_export]
macro_rules! d {
    ($n:literal) => {
        $crate::pitch::Degree {
            number: $n,
            alter: 0,
            octave_shift: 0,
        }
    };
    (b $n:literal) => {
        $crate::pitch::Degree {
            number: $n,
            alter: -1,
            octave_shift: 0,
        }
    };
    (# $n:literal) => {
        $crate::pitch::Degree {
            number: $n,
            alter: 1,
            octave_shift: 0,
        }
    };
}
