use std::ops::{Add, BitOr, Mul};

use serde::{Deserialize, Serialize};

use crate::attrs::NoteAttrs;
use crate::control::Control;
use crate::pitch::{ChromaticPitch, Pitch};
use crate::time::Beats;

// === Core ADT ===

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Note {
    pub pitch: Pitch,
    pub dur: Beats,
    pub attrs: NoteAttrs,
}

/// The five-constructor music ADT. All musical structure is a tree of these.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Music {
    /// A sounded event.
    Note(Note),
    /// Silence.
    Rest(Beats),
    /// Sequential composition. Semigroup: associative, identity is empty Seq.
    Seq(Vec<Music>),
    /// Parallel composition. Semigroup: associative, identity is empty Par.
    Par(Vec<Music>),
    /// Context modifier applied to a subtree.
    Modify(Control, Box<Music>),
}

impl Music {
    pub fn modify(self, control: Control) -> Music {
        Music::Modify(control, Box::new(self))
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Music::Seq(v) | Music::Par(v) => v.is_empty(),
            _ => false,
        }
    }
}

// === Operator overloading ===

/// `a + b` = sequential composition with Seq flattening.
impl Add for Music {
    type Output = Music;
    fn add(self, other: Music) -> Music {
        match (self, other) {
            (Music::Seq(mut a), Music::Seq(b)) => {
                a.extend(b);
                Music::Seq(a)
            }
            (Music::Seq(mut a), b) => {
                a.push(b);
                Music::Seq(a)
            }
            (a, Music::Seq(mut b)) => {
                b.insert(0, a);
                Music::Seq(b)
            }
            (a, b) => Music::Seq(vec![a, b]),
        }
    }
}

/// `a | b` = parallel composition with Par flattening.
impl BitOr for Music {
    type Output = Music;
    fn bitor(self, other: Music) -> Music {
        match (self, other) {
            (Music::Par(mut a), Music::Par(b)) => {
                a.extend(b);
                Music::Par(a)
            }
            (Music::Par(mut a), b) => {
                a.push(b);
                Music::Par(a)
            }
            (a, Music::Par(mut b)) => {
                b.insert(0, a);
                Music::Par(b)
            }
            (a, b) => Music::Par(vec![a, b]),
        }
    }
}

/// `m * n` = repeat n times.
impl Mul<usize> for Music {
    type Output = Music;
    fn mul(self, n: usize) -> Music {
        Music::Seq(std::iter::repeat(self).take(n).collect())
    }
}

// === Smart constructors ===

/// Create a note from anything that converts to `Pitch`.
pub fn n(pitch: impl Into<Pitch>, dur: Beats) -> Music {
    Music::Note(Note { pitch: pitch.into(), dur, attrs: Default::default() })
}

/// Create a rest.
pub fn r(dur: Beats) -> Music {
    Music::Rest(dur)
}

/// Create a chord: all pitches sounding simultaneously for `dur` beats.
pub fn chord(pitches: impl IntoIterator<Item = ChromaticPitch>, dur: Beats) -> Music {
    Music::Par(pitches.into_iter().map(|p| n(p, dur)).collect())
}

/// Sequential composition macro: `seq![a, b, c]`
#[macro_export]
macro_rules! seq {
    [$($m:expr),* $(,)?] => { $crate::music::Music::Seq(vec![$($m),*]) };
}

/// Parallel composition macro: `par![a, b, c]`
#[macro_export]
macro_rules! par {
    [$($m:expr),* $(,)?] => { $crate::music::Music::Par(vec![$($m),*]) };
}
