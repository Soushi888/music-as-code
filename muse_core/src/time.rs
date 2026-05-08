use num_rational::Rational32;
use serde::{Deserialize, Serialize};

/// Duration in beats. Quarter = 1, eighth = 1/2, dotted half = 3.
pub type Beats = Rational32;

pub fn b(num: i32, den: i32) -> Beats {
    Rational32::new(num, den)
}

pub fn dot(d: Beats) -> Beats {
    d * Rational32::new(3, 2)
}

pub fn triplet(d: Beats) -> Beats {
    d * Rational32::new(2, 3)
}

// Duration helpers (can't be `const` because Rational32::new is not const fn)
pub fn w() -> Beats { b(4, 1) }
pub fn h() -> Beats { b(2, 1) }
pub fn q() -> Beats { b(1, 1) }
pub fn e() -> Beats { b(1, 2) }
pub fn s() -> Beats { b(1, 4) }
pub fn ts() -> Beats { b(1, 8) }   // thirty-second

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tempo {
    Fixed(u32),
    Ramp { from_bpm: u32, to_bpm: u32, over_beats: u32 },
}

impl Tempo {
    pub fn bpm(bpm: u32) -> Self {
        Tempo::Fixed(bpm)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct TimeSig {
    pub numerator: u8,
    pub denominator: u8,
}

impl TimeSig {
    pub fn new(numerator: u8, denominator: u8) -> Self {
        Self { numerator, denominator }
    }

    pub fn common() -> Self { Self::new(4, 4) }
    pub fn cut() -> Self { Self::new(2, 2) }
    pub fn waltz() -> Self { Self::new(3, 4) }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Dynamics {
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
    Sfz,
    Fp,
    Crescendo,
    Decrescendo,
}
