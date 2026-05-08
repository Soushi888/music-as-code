//! The core five-constructor [`Music`] ADT and smart constructors.
//!
//! Everything playable decomposes into a tree of five node types: [`Music::Note`],
//! [`Music::Rest`], [`Music::Seq`], [`Music::Par`], and [`Music::Modify`].
//! Sequential composition uses `+`, parallel uses `|`, repeat uses `* n`.

use std::ops::{Add, BitOr, Mul};

use serde::{Deserialize, Serialize};

use crate::attrs::NoteAttrs;
use crate::control::Control;
use crate::pitch::{ChromaticPitch, Pitch};
use crate::time::Beats;

// === Core ADT ===

/// A single sounded event with pitch, duration, and performance attributes.
///
/// Construct via [`n()`] rather than directly:
/// `n(C4, q())` is clearer than `Music::Note(Note { pitch: C4.into(), dur: q(), attrs: Default::default() })`.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Note {
    /// The pitch of this note. May be chromatic, a scale degree, or an interval.
    pub pitch: Pitch,
    /// Written duration in beats.
    pub dur: Beats,
    /// Optional per-note performance attributes (velocity, articulation, hints, etc.).
    pub attrs: NoteAttrs,
}

/// The five-constructor music ADT. All musical structure is a tree of these.
///
/// # Composition operators
///
/// | Expression | Result |
/// |------------|--------|
/// | `a + b` | Sequential: `b` starts when `a` ends. |
/// | `a \| b` | Parallel: `a` and `b` start simultaneously. |
/// | `m * n` | Repeat `m` exactly `n` times. |
///
/// Both `+` and `|` flatten adjacent nodes of the same type, so
/// `a + b + c` produces `Seq([a, b, c])`, not nested `Seq`s.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Music {
    /// A sounded event: a single note with pitch, duration, and attributes.
    Note(Note),
    /// Silence for the given duration.
    Rest(Beats),
    /// Sequential composition: children play one after another.
    ///
    /// Semigroup: `Seq([]) + m == m == m + Seq([])`.
    Seq(Vec<Music>),
    /// Parallel composition: all children start simultaneously.
    ///
    /// Semigroup: `Par([]) | m == m == m | Par([])`.
    Par(Vec<Music>),
    /// Apply a context modifier to a subtree.
    ///
    /// The `Control` value takes effect for the entire contained `Music` tree.
    /// Controls nest: an inner `Modify` with the same control type shadows the outer one.
    Modify(Control, Box<Music>),
}

impl Music {
    /// Wrap `self` in a `Modify` node, applying the given `control` to this subtree.
    ///
    /// Equivalent to `Music::Modify(control, Box::new(self))`. Designed for chaining:
    ///
    /// ```
    /// use musecode_core::prelude::*;
    /// let piece = seq![n(C4, q()), n(E4, q())]
    ///     .modify(Control::Key(Key::major(pc!(C))))
    ///     .modify(Control::Tempo(Tempo::bpm(120)));
    /// ```
    pub fn modify(self, control: Control) -> Music {
        Music::Modify(control, Box::new(self))
    }

    /// Returns `true` if this node is an empty `Seq` or empty `Par`.
    ///
    /// An empty `Seq` or `Par` is the identity element for `+` and `|` respectively.
    pub fn is_empty(&self) -> bool {
        match self {
            Music::Seq(v) | Music::Par(v) => v.is_empty(),
            _ => false,
        }
    }
}

// === Operator overloading ===

/// Sequential composition: `a + b` plays `b` immediately after `a`.
///
/// Adjacent `Seq` nodes are flattened: `Seq([a, b]) + Seq([c, d])` = `Seq([a, b, c, d])`.
impl Add for Music {
    type Output = Music;
    fn add(self, other: Music) -> Music {
        match (self, other) {
            (Music::Seq(mut a), Music::Seq(b)) => { a.extend(b); Music::Seq(a) }
            (Music::Seq(mut a), b)             => { a.push(b);   Music::Seq(a) }
            (a, Music::Seq(mut b))             => { b.insert(0, a); Music::Seq(b) }
            (a, b)                             => Music::Seq(vec![a, b]),
        }
    }
}

/// Parallel composition: `a | b` starts both `a` and `b` at the same time.
///
/// Adjacent `Par` nodes are flattened: `Par([a, b]) | Par([c, d])` = `Par([a, b, c, d])`.
impl BitOr for Music {
    type Output = Music;
    fn bitor(self, other: Music) -> Music {
        match (self, other) {
            (Music::Par(mut a), Music::Par(b)) => { a.extend(b); Music::Par(a) }
            (Music::Par(mut a), b)             => { a.push(b);   Music::Par(a) }
            (a, Music::Par(mut b))             => { b.insert(0, a); Music::Par(b) }
            (a, b)                             => Music::Par(vec![a, b]),
        }
    }
}

/// Repetition: `m * n` plays `m` exactly `n` times in sequence.
///
/// Implemented as `Seq` of `n` clones of `self`.
/// `m * 0` produces `Seq([])` (silence with zero duration).
impl Mul<usize> for Music {
    type Output = Music;
    fn mul(self, n: usize) -> Music {
        Music::Seq(std::iter::repeat(self).take(n).collect())
    }
}

// === Smart constructors ===

/// Create a [`Music::Note`] from a pitch and duration.
///
/// `pitch` can be a [`ChromaticPitch`], a [`Degree`][crate::pitch::Degree],
/// or an [`Interval`][crate::pitch::Interval]:
/// anything implementing `Into<Pitch>`. Performance attrs default to empty.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// let c4_quarter  = n(C4,    q());
/// let tonic_half  = n(d!(1), h());
/// ```
pub fn n(pitch: impl Into<Pitch>, dur: Beats) -> Music {
    Music::Note(Note { pitch: pitch.into(), dur, attrs: Default::default() })
}

/// Create a [`Music::Rest`] of the given duration.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// let quarter_rest = r(q());
/// let bar_rest     = r(w());
/// ```
pub fn r(dur: Beats) -> Music {
    Music::Rest(dur)
}

/// Create a [`Music::Par`] chord: all `pitches` sounding simultaneously for `dur`.
///
/// Each pitch becomes a separate `Note` inside a `Par` node. Only accepts
/// [`ChromaticPitch`] to avoid ambiguous degree resolution across voices.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// let cmaj = chord([C4, E4, G4], h());
/// ```
pub fn chord(pitches: impl IntoIterator<Item = ChromaticPitch>, dur: Beats) -> Music {
    Music::Par(pitches.into_iter().map(|p| n(p, dur)).collect())
}

/// Build a [`Music::Seq`] from a list of expressions.
///
/// `seq![a, b, c]` expands to `Music::Seq(vec![a, b, c])`. Trailing commas are allowed.
#[macro_export]
macro_rules! seq {
    [$($m:expr),* $(,)?] => { $crate::music::Music::Seq(vec![$($m),*]) };
}

/// Build a [`Music::Par`] from a list of expressions.
///
/// `par![melody, bass]` expands to `Music::Par(vec![melody, bass])`. Trailing commas are allowed.
#[macro_export]
macro_rules! par {
    [$($m:expr),* $(,)?] => { $crate::music::Music::Par(vec![$($m),*]) };
}
