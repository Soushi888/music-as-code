//! Rational time: the [`Beats`] type alias and duration helpers.
//!
//! All durations are exact rational numbers (`num_rational::Rational32`).
//! A quarter note is `q() = 1/1`, an eighth is `e() = 1/2`, a dotted quarter
//! is `dot(q()) = 3/2`. No floating-point drift.

use num_rational::Rational32;
use serde::{Deserialize, Serialize};

/// Duration in beats, represented as an exact rational number.
///
/// Quarter note = `1/1`, half = `2/1`, eighth = `1/2`, sixteenth = `1/4`.
/// Use the helper functions ([`q()`], [`h()`], [`e()`], etc.) for common values,
/// or [`b(num, den)`][b] for arbitrary ratios.
pub type Beats = Rational32;

/// Construct an arbitrary rational [`Beats`] value.
///
/// `b(3, 8)` = dotted eighth, `b(7, 4)` = 7 sixteenth-note triplets.
///
/// # Panics
/// Panics if `den` is zero.
pub fn b(num: i32, den: i32) -> Beats {
    Rational32::new(num, den)
}

/// Return the dotted version of a duration: `d * 3/2`.
///
/// `dot(q())` = dotted quarter (1.5 beats).
/// `dot(dot(h()))` = double-dotted half (3.5 beats).
pub fn dot(d: Beats) -> Beats {
    d * Rational32::new(3, 2)
}

/// Return the triplet version of a duration: `d * 2/3`.
///
/// `triplet(q())` = quarter-note triplet (2/3 of a beat).
/// Three `triplet(q())` values sum to exactly `h()`.
pub fn triplet(d: Beats) -> Beats {
    d * Rational32::new(2, 3)
}

/// Whole note: 4 beats.
pub fn w() -> Beats { b(4, 1) }
/// Half note: 2 beats.
pub fn h() -> Beats { b(2, 1) }
/// Quarter note: 1 beat (the fundamental unit).
pub fn q() -> Beats { b(1, 1) }
/// Eighth note: 1/2 beat.
pub fn e() -> Beats { b(1, 2) }
/// Sixteenth note: 1/4 beat.
pub fn s() -> Beats { b(1, 4) }
/// Thirty-second note: 1/8 beat.
pub fn ts() -> Beats { b(1, 8) }

/// A tempo specification for a `Control::Tempo` modifier.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tempo {
    /// Constant tempo in beats per minute.
    Fixed(u32),
    /// Linear tempo ramp from `from_bpm` to `to_bpm` over `over_beats` beats.
    Ramp {
        /// Starting tempo in BPM.
        from_bpm: u32,
        /// Ending tempo in BPM.
        to_bpm: u32,
        /// Duration of the ramp in beats.
        over_beats: u32,
    },
}

impl Tempo {
    /// Construct a [`Tempo::Fixed`] from a BPM value.
    ///
    /// `Tempo::bpm(120)` is equivalent to `Tempo::Fixed(120)`.
    pub fn bpm(bpm: u32) -> Self {
        Tempo::Fixed(bpm)
    }
}

/// A time signature: beats per measure over a note-value denominator.
///
/// Stored as a pair of integers rather than a fraction; `4/4` and `2/2` are
/// distinct values even though they span the same duration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct TimeSig {
    /// Number of beats per measure (top number).
    pub numerator: u8,
    /// Note value that receives one beat (bottom number): 4 = quarter, 8 = eighth.
    pub denominator: u8,
}

impl TimeSig {
    /// Construct a time signature from numerator and denominator.
    ///
    /// `TimeSig::new(6, 8)` = 6/8 time.
    pub fn new(numerator: u8, denominator: u8) -> Self {
        Self { numerator, denominator }
    }

    /// Common time: 4/4.
    pub fn common() -> Self { Self::new(4, 4) }

    /// Cut time (alla breve): 2/2.
    pub fn cut() -> Self { Self::new(2, 2) }

    /// Waltz time: 3/4.
    pub fn waltz() -> Self { Self::new(3, 4) }
}

/// Dynamic level or a hairpin directive.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Dynamics {
    /// Pianississimo — extremely soft.
    Ppp,
    /// Pianissimo — very soft.
    Pp,
    /// Piano — soft.
    P,
    /// Mezzo-piano — moderately soft.
    Mp,
    /// Mezzo-forte — moderately loud.
    Mf,
    /// Forte — loud.
    F,
    /// Fortissimo — very loud.
    Ff,
    /// Fortississimo — extremely loud.
    Fff,
    /// Sforzando — suddenly forced/accented.
    Sfz,
    /// Forte-piano — loud then immediately soft.
    Fp,
    /// Gradual increase in volume (hairpin open).
    Crescendo,
    /// Gradual decrease in volume (hairpin close).
    Decrescendo,
}
