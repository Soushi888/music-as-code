//! Rational time: the [`Beats`] type alias and duration helpers.
//!
//! All durations are exact rational numbers (`num_rational::Rational32`).
//! A quarter note is `q() = 1/1`, an eighth is `e() = 1/2`, a dotted quarter
//! is `dot(q()) = 3/2`. No floating-point drift.

use std::fmt;

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
/// `dot(q())` = dotted quarter (1.5 beats). Note that `dot(dot(h()))` is
/// `2 * 9/4 = 4.5` beats, not the notational double-dotted half (3.5 beats,
/// which is `h() * b(7, 4)`); [`duration_name`] names the latter `h..`.
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

/// The short name of a duration in the notation of `docs/NOTATION.md`.
///
/// `w h q e s t` for whole to thirty-second, a trailing `.` per dot, a
/// trailing `3` for the triplet of a value, and `num/den` for anything else.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// assert_eq!(duration_name(q()), "q");
/// assert_eq!(duration_name(dot(e())), "e.");
/// assert_eq!(duration_name(h() * b(7, 4)), "h..");
/// assert_eq!(duration_name(triplet(q())), "q3");
/// assert_eq!(duration_name(b(5, 4)), "5/4");
/// ```
/// A named duration helper, for the table in [`duration_name`].
type Named = (&'static str, fn() -> Beats);

pub fn duration_name(d: Beats) -> String {
    const NAMES: [Named; 6] = [("w", w), ("h", h), ("q", q), ("e", e), ("s", s), ("t", ts)];
    for (name, base) in NAMES {
        let base = base();
        if d == base {
            return name.to_string();
        }
        if d == dot(base) {
            return format!("{name}.");
        }
        if d == base * Rational32::new(7, 4) {
            return format!("{name}..");
        }
        if d == triplet(base) {
            return format!("{name}3");
        }
    }
    if d.is_integer() {
        format!("{}/1", d.numer())
    } else {
        format!("{}/{}", d.numer(), d.denom())
    }
}

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

impl fmt::Display for Tempo {
    /// `96` for a fixed tempo, `96->120:8` for a ramp over 8 beats.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tempo::Fixed(bpm) => write!(f, "{bpm}"),
            Tempo::Ramp { from_bpm, to_bpm, over_beats } => write!(f, "{from_bpm}->{to_bpm}:{over_beats}"),
        }
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

impl fmt::Display for TimeSig {
    /// `4/4`, `6/8`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
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

impl fmt::Display for Dynamics {
    /// `ppp` through `fff`, `sfz`, `fp`; hairpins are `<` and `>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Dynamics::Ppp => "ppp",
            Dynamics::Pp => "pp",
            Dynamics::P => "p",
            Dynamics::Mp => "mp",
            Dynamics::Mf => "mf",
            Dynamics::F => "f",
            Dynamics::Ff => "ff",
            Dynamics::Fff => "fff",
            Dynamics::Sfz => "sfz",
            Dynamics::Fp => "fp",
            Dynamics::Crescendo => "<",
            Dynamics::Decrescendo => ">",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_helper_prints_its_short_name() {
        let cases = [
            (w(), "w"), (h(), "h"), (q(), "q"), (e(), "e"), (s(), "s"), (ts(), "t"),
            (dot(w()), "w."), (dot(h()), "h."), (dot(q()), "q."), (dot(e()), "e."), (dot(s()), "s."), (dot(ts()), "t."),
            (q() * b(7, 4), "q.."), (dot(dot(q())), "9/4"), (triplet(h()), "h3"), (triplet(q()), "q3"), (triplet(e()), "e3"), (triplet(ts()), "t3"),
            (b(5, 4), "5/4"), (b(11, 1), "11/1"), (b(0, 1), "0/1"), (b(9, 8), "9/8"),
        ];
        for (d, name) in cases {
            assert_eq!(duration_name(d), name, "{d}");
        }
        // dot(q()) == 3/2 and is reported by its name, not as a fraction.
        assert_eq!(duration_name(b(3, 2)), "q.");
    }

    #[test]
    fn tempo_and_time_signature_print_compactly() {
        assert_eq!(Tempo::bpm(96).to_string(), "96");
        assert_eq!(Tempo::Ramp { from_bpm: 96, to_bpm: 120, over_beats: 8 }.to_string(), "96->120:8");
        assert_eq!(TimeSig::common().to_string(), "4/4");
        assert_eq!(Dynamics::Crescendo.to_string(), "<");
    }
}
