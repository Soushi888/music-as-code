//! Euclidean rhythms: `k` hits spread as evenly as possible over `n` equal
//! steps, by Bjorklund's algorithm as described by Godfried Toussaint
//! ("The Euclidean Algorithm Generates Traditional Musical Rhythms", 2005).
//!
//! The generator produces a [`Pattern`], so everything in [`crate::rhythm`]
//! applies: pitch it with `on`/`with`, chain it, repeat it. Two transforms
//! live here because they are what one does with a grid: [`Pattern::rotate`]
//! moves the downbeat, [`Pattern::legato`] lets each hit ring until the next
//! one, which is how a grid becomes a bass line. E(3,8) legato *is* the
//! tresillo:
//!
//! ```
//! use musecode_core::prelude::*;
//! let e38 = euclid(3, 8, e()).unwrap();
//! assert_eq!(e38.grid(), "x..x..x.");
//! assert_eq!(e38.legato(), tresillo());
//! ```

use std::fmt;

use crate::rhythm::{Pattern, Step};
use crate::time::Beats;

/// Why a Euclidean rhythm could not be built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EuclidError {
    /// `steps` was zero; a rhythm needs at least one step.
    ZeroSteps,
    /// More hits than steps.
    TooManyHits {
        /// Requested hits.
        hits: usize,
        /// Available steps.
        steps: usize,
    },
}

impl fmt::Display for EuclidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EuclidError::ZeroSteps => write!(f, "a Euclidean rhythm needs at least one step"),
            EuclidError::TooManyHits { hits, steps } => {
                write!(f, "cannot place {hits} hits in {steps} steps")
            }
        }
    }
}

impl std::error::Error for EuclidError {}

/// Bjorklund's distribution of `hits` ones among `steps` slots, as booleans.
///
/// Starts on a hit (when `hits > 0`) and matches Toussaint's published
/// patterns: E(3,8) `x..x..x.`, E(5,8) `x.xx.xx.`, E(5,12) `x..x.x..x.x.`,
/// E(7,12) `x.xx.x.xx.x.`, E(5,16) `x..x..x..x..x...`, E(7,16) `x..x.x.x..x.x.x.`.
pub fn bjorklund(hits: usize, steps: usize) -> Result<Vec<bool>, EuclidError> {
    if steps == 0 {
        return Err(EuclidError::ZeroSteps);
    }
    if hits > steps {
        return Err(EuclidError::TooManyHits { hits, steps });
    }
    if hits == 0 {
        return Ok(vec![false; steps]);
    }
    // Bjorklund: pair the shorter group onto the longer until the remainder is one group or none.
    let mut a: Vec<Vec<bool>> = vec![vec![true]; hits];
    let mut b: Vec<Vec<bool>> = vec![vec![false]; steps - hits];
    while a.len().min(b.len()) > 1 {
        let m = a.len().min(b.len());
        let remainder: Vec<Vec<bool>> = if a.len() > b.len() { a[m..].to_vec() } else { b[m..].to_vec() };
        let paired: Vec<Vec<bool>> = a[..m]
            .iter()
            .zip(&b[..m])
            .map(|(x, y)| x.iter().chain(y).copied().collect())
            .collect();
        a = paired;
        b = remainder;
    }
    Ok(a.into_iter().chain(b).flatten().collect())
}

/// The Euclidean rhythm E(`hits`, `steps`) on a grid of `step`-long slots.
///
/// Every slot is a [`Step::Hit`] or a [`Step::Rest`] of length `step`, so the
/// pattern lasts `steps * step`. Use [`Pattern::legato`] to let hits ring
/// until the next one (a bass line) and [`Pattern::rotate`] to move the
/// downbeat.
///
/// ```
/// use musecode_core::prelude::*;
/// let bembe = euclid(7, 12, e()).unwrap();
/// assert_eq!(bembe.grid(), "x.xx.x.xx.x.");
/// assert_eq!(bembe.hit_count(), 7);
/// assert_eq!(bembe.duration(), e() * 12);
/// ```
pub fn euclid(hits: usize, steps: usize, step: Beats) -> Result<Pattern, EuclidError> {
    let bits = bjorklund(hits, steps)?;
    Ok(Pattern::from_grid(&bits, step))
}

impl Pattern {
    /// A pattern from a grid of booleans, `true` for a hit, every slot `step` long.
    pub fn from_grid(grid: &[bool], step: Beats) -> Pattern {
        Pattern::new(grid.iter().map(|&hit| if hit { Step::Hit(step) } else { Step::Rest(step) }))
    }

    /// The pattern as a grid string, `x` for a hit and `.` for a rest, one
    /// character per step regardless of length: `tresillo().grid()` is `xxx`,
    /// `euclid(3, 8, e())` is `x..x..x.`.
    pub fn grid(&self) -> String {
        self.steps().iter().map(|s| if matches!(s, Step::Hit(_)) { 'x' } else { '.' }).collect()
    }

    /// Rotate the pattern left by `n` steps, so the step at index `n` becomes
    /// the downbeat. `rotate(0)` and `rotate(len)` are the identity.
    pub fn rotate(&self, n: usize) -> Pattern {
        let steps = self.steps();
        if steps.is_empty() {
            return self.clone();
        }
        let n = n % steps.len();
        Pattern::new(steps[n..].iter().chain(&steps[..n]).copied())
    }

    /// Swap hits and rests, keeping every duration: the complementary rhythm.
    pub fn complement(&self) -> Pattern {
        Pattern::new(self.steps().iter().map(|s| match *s {
            Step::Hit(d) => Step::Rest(d),
            Step::Rest(d) => Step::Hit(d),
        }))
    }

    /// Let every hit ring until the next hit: each hit absorbs the rests that
    /// follow it. Leading rests (before the first hit) are kept as rests.
    /// This is how a grid pattern becomes a sustained bass line;
    /// `euclid(3, 8, e()).legato()` equals [`tresillo`][crate::rhythm::tresillo].
    pub fn legato(&self) -> Pattern {
        let mut out: Vec<Step> = Vec::new();
        for &step in self.steps() {
            match (step, out.last_mut()) {
                (Step::Rest(d), Some(Step::Hit(held))) => *held += d,
                (s, _) => out.push(s),
            }
        }
        Pattern::new(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::G3;
    use crate::rhythm::{straight, tresillo};
    use crate::time::{dot, e, q, s, w};

    fn grid(hits: usize, steps: usize) -> String {
        euclid(hits, steps, e()).unwrap().grid()
    }

    #[test]
    fn matches_toussaints_published_patterns() {
        assert_eq!(grid(3, 8), "x..x..x.", "tresillo");
        assert_eq!(grid(5, 8), "x.xx.xx.", "cinquillo");
        assert_eq!(grid(5, 12), "x..x.x..x.x.", "Venda clapping pattern");
        assert_eq!(grid(7, 12), "x.xx.x.xx.x.", "bembé");
        assert_eq!(grid(5, 16), "x..x..x..x..x...", "bossa nova");
        assert_eq!(grid(7, 16), "x..x.x.x..x.x.x.", "Samba");
        assert_eq!(grid(2, 5), "x.x..");
        assert_eq!(grid(3, 7), "x.x.x..");
        assert_eq!(grid(4, 9), "x.x.x.x..");
    }

    #[test]
    fn degenerate_counts_and_errors() {
        assert_eq!(grid(0, 4), "....");
        assert_eq!(grid(4, 4), "xxxx");
        assert_eq!(grid(1, 1), "x");
        assert_eq!(euclid(3, 0, e()), Err(EuclidError::ZeroSteps));
        assert_eq!(euclid(9, 8, e()), Err(EuclidError::TooManyHits { hits: 9, steps: 8 }));
    }

    #[test]
    fn every_euclidean_rhythm_has_the_right_count_and_length() {
        for steps in 1..=24 {
            for hits in 0..=steps {
                let p = euclid(hits, steps, s()).unwrap();
                assert_eq!(p.hit_count(), hits, "E({hits},{steps})");
                assert_eq!(p.steps().len(), steps);
                assert_eq!(p.duration(), s() * steps as i32);
                if hits > 0 {
                    assert!(matches!(p.steps()[0], Step::Hit(_)), "E({hits},{steps}) starts on a hit");
                }
            }
        }
    }

    #[test]
    fn gaps_between_hits_differ_by_at_most_one() {
        // The defining property of a Euclidean rhythm: maximally even spacing.
        for steps in 1..=24 {
            for hits in 1..=steps {
                let bits = bjorklund(hits, steps).unwrap();
                let onsets: Vec<usize> = bits.iter().enumerate().filter(|(_, &b)| b).map(|(i, _)| i).collect();
                let gaps: Vec<usize> = onsets
                    .iter()
                    .zip(onsets.iter().skip(1).chain(std::iter::once(&(onsets[0] + steps))))
                    .map(|(a, b)| b - a)
                    .collect();
                let (lo, hi) = (*gaps.iter().min().unwrap(), *gaps.iter().max().unwrap());
                assert!(hi - lo <= 1, "E({hits},{steps}) gaps {gaps:?}");
            }
        }
    }

    #[test]
    fn legato_e38_is_the_tresillo_and_keeps_leading_rests() {
        assert_eq!(euclid(3, 8, e()).unwrap().legato(), tresillo());
        assert_eq!(euclid(3, 8, e()).unwrap().legato().on(G3), tresillo().on(G3));
        let shifted = euclid(3, 8, e()).unwrap().rotate(7); // ".x..x..x"
        assert_eq!(shifted.grid(), ".x..x..x");
        assert_eq!(shifted.legato(), Pattern::new([Step::Rest(e()), Step::Hit(dot(q())), Step::Hit(dot(q())), Step::Hit(e())]));
        assert_eq!(shifted.legato().duration(), w());
    }

    #[test]
    fn rotate_and_complement_are_involutions_where_they_should_be() {
        let p = euclid(5, 8, e()).unwrap();
        assert_eq!(p.rotate(0), p);
        assert_eq!(p.rotate(8), p);
        assert_eq!(p.rotate(3).rotate(5), p);
        assert_eq!(p.complement().complement(), p);
        assert_eq!(p.complement().grid(), ".x..x..x");
        assert_eq!(p.complement().hit_count(), 3);
        assert_eq!(Pattern::default().rotate(3), Pattern::default());
    }

    #[test]
    fn grid_and_from_grid_round_trip_on_equal_steps() {
        let p = euclid(7, 16, s()).unwrap();
        let bits: Vec<bool> = p.grid().chars().map(|c| c == 'x').collect();
        assert_eq!(Pattern::from_grid(&bits, s()), p);
        assert_eq!(straight(4, q()).grid(), "xxxx");
    }
}
