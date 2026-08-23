//! Rhythm as a first-class value: a [`Pattern`] is a sequence of hits and
//! rests with durations and no pitch. Pitch is applied afterwards, so one
//! rhythm serves any bass line, any riff, any voice.
//!
//! ```
//! use musecode_core::prelude::*;
//! // The nuevo-tango bass: tresillo (3+3+2 eighths) on the root of each bar.
//! let bar = tresillo().on(G3);
//! assert_eq!(bar.duration(), w());
//! // Same rhythm, different pitches cycling over the hits.
//! let walk = tresillo().with([G3, D4, G3]);
//! assert_eq!(walk.duration(), w());
//! ```
//!
//! Patterns compose like music does: [`Pattern::then`] concatenates,
//! [`Pattern::repeat`] loops, and the library ships the figures the genre is
//! built on: [`tresillo`], [`habanera`], [`cinquillo`], [`straight`].

use crate::music::{n, r, Music};
use crate::pitch::Pitch;
use crate::time::{dot, e, q, s, Beats};

/// One step of a [`Pattern`]: a struck note or a silence, each with a duration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Step {
    /// A note is struck and held for this long.
    Hit(Beats),
    /// Silence for this long.
    Rest(Beats),
}

impl Step {
    /// The duration of the step, hit or rest.
    pub fn duration(self) -> Beats {
        match self {
            Step::Hit(d) | Step::Rest(d) => d,
        }
    }
}

/// A rhythm with no pitch: an ordered list of [`Step`]s.
///
/// Apply pitch with [`on`][Self::on] (one pitch for every hit) or
/// [`with`][Self::with] (pitches cycled over the hits) to get [`Music`].
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Pattern {
    steps: Vec<Step>,
}

impl Pattern {
    /// A pattern from explicit steps.
    pub fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self { steps: steps.into_iter().collect() }
    }

    /// A pattern of hits only, one per duration.
    ///
    /// `Pattern::hits([dot(q()), dot(q()), q()])` is the [`tresillo`].
    pub fn hits(durations: impl IntoIterator<Item = Beats>) -> Self {
        Self::new(durations.into_iter().map(Step::Hit))
    }

    /// The steps, in order.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Number of struck notes (rests excluded).
    pub fn hit_count(&self) -> usize {
        self.steps.iter().filter(|s| matches!(s, Step::Hit(_))).count()
    }

    /// Total duration, hits and rests together.
    pub fn duration(&self) -> Beats {
        self.steps.iter().map(|s| s.duration()).sum()
    }

    /// This pattern followed by `other`.
    pub fn then(mut self, other: Pattern) -> Pattern {
        self.steps.extend(other.steps);
        self
    }

    /// This pattern repeated `n` times in a row. `repeat(0)` is the empty pattern.
    pub fn repeat(&self, n: usize) -> Pattern {
        Pattern { steps: self.steps.iter().copied().cycle().take(self.steps.len() * n).collect() }
    }

    /// Every hit gets `pitch`; rests stay rests. The result is a `Seq`.
    ///
    /// ```
    /// use musecode_core::prelude::*;
    /// let bass = tresillo().on(C3);
    /// assert_eq!(bass, seq![n(C3, dot(q())), n(C3, dot(q())), n(C3, q())]);
    /// ```
    pub fn on(&self, pitch: impl Into<Pitch>) -> Music {
        let pitch = pitch.into();
        Music::Seq(
            self.steps
                .iter()
                .map(|step| match *step {
                    Step::Hit(d) => n(pitch, d),
                    Step::Rest(d) => r(d),
                })
                .collect(),
        )
    }

    /// Hits take `pitches` in order, cycling when the pattern has more hits
    /// than pitches; rests stay rests. The result is a `Seq`.
    ///
    /// With no pitches at all, every hit becomes a rest of the same length,
    /// so the rhythm's duration is preserved.
    ///
    /// ```
    /// use musecode_core::prelude::*;
    /// // root, root, fifth on the tresillo
    /// let m = tresillo().with([G3, G3, D4]);
    /// assert_eq!(m, seq![n(G3, dot(q())), n(G3, dot(q())), n(D4, q())]);
    /// ```
    pub fn with<P: Into<Pitch>>(&self, pitches: impl IntoIterator<Item = P>) -> Music {
        let pitches: Vec<Pitch> = pitches.into_iter().map(Into::into).collect();
        let mut next = pitches.iter().cycle();
        Music::Seq(
            self.steps
                .iter()
                .map(|step| match *step {
                    Step::Hit(d) => match next.next() {
                        Some(p) => n(*p, d),
                        None => r(d),
                    },
                    Step::Rest(d) => r(d),
                })
                .collect(),
        )
    }
}

/// Tresillo, the 3+3+2 figure: dotted quarter, dotted quarter, quarter.
///
/// One bar of 4/4. The rhythmic backbone of nuevo tango (Piazzolla's bass
/// lines), and of most Afro-Cuban and Caribbean music under other names.
pub fn tresillo() -> Pattern {
    Pattern::hits([dot(q()), dot(q()), q()])
}

/// Habanera: dotted eighth, sixteenth, eighth, eighth.
///
/// Half a bar of 4/4; `habanera().repeat(2)` fills a bar. The traditional
/// tango (and *milonga*) bass, kept here as the foil to [`tresillo`].
pub fn habanera() -> Pattern {
    Pattern::hits([dot(e()), s(), e(), e()])
}

/// Cinquillo: eighth, sixteenth, eighth, sixteenth, eighth.
///
/// Half a bar of 4/4, five hits in the space of four eighths; the figure that
/// decorates a tresillo in the danzón and in Piazzolla's faster passages.
pub fn cinquillo() -> Pattern {
    Pattern::hits([e(), s(), e(), s(), e()])
}

/// `count` equal hits of `dur` each: `straight(4, q())` is four quarters.
pub fn straight(count: usize, dur: Beats) -> Pattern {
    Pattern::hits(std::iter::repeat_n(dur, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::{C3, D4, G3};
    use crate::seq;
    use crate::time::{h, w};

    #[test]
    fn the_figures_have_the_lengths_the_genre_says() {
        assert_eq!(tresillo().duration(), w());
        assert_eq!(tresillo().hit_count(), 3);
        assert_eq!(habanera().duration(), h());
        assert_eq!(habanera().repeat(2).duration(), w());
        assert_eq!(cinquillo().duration(), h());
        assert_eq!(cinquillo().hit_count(), 5);
        assert_eq!(straight(4, q()).duration(), w());
    }

    #[test]
    fn on_gives_every_hit_the_same_pitch_and_keeps_rests() {
        let p = Pattern::new([Step::Hit(q()), Step::Rest(e()), Step::Hit(e())]);
        assert_eq!(p.on(C3), seq![n(C3, q()), r(e()), n(C3, e())]);
        assert_eq!(p.on(C3).duration(), p.duration());
    }

    #[test]
    fn with_cycles_pitches_over_hits_only() {
        let p = Pattern::new([Step::Hit(q()), Step::Rest(q()), Step::Hit(q()), Step::Hit(q())]);
        assert_eq!(p.with([G3, D4]), seq![n(G3, q()), r(q()), n(D4, q()), n(G3, q())]);
    }

    #[test]
    fn with_no_pitches_is_silence_of_the_same_length() {
        let m = tresillo().with(Vec::<Pitch>::new());
        assert_eq!(m, seq![r(dot(q())), r(dot(q())), r(q())]);
        assert_eq!(m.duration(), w());
    }

    #[test]
    fn then_and_repeat_compose_like_seq() {
        let two_bars = tresillo().then(tresillo());
        assert_eq!(two_bars, tresillo().repeat(2));
        assert_eq!(two_bars.duration(), w() + w());
        assert_eq!(tresillo().repeat(0), Pattern::default());
        assert_eq!(tresillo().repeat(2).on(C3), tresillo().on(C3) + tresillo().on(C3));
    }

    #[test]
    fn the_traditional_bass_is_habanera_then_two_quarters() {
        // The example's bass before slice 7, tree for tree; `habanera().repeat(2)` is a different figure.
        let old_bass = seq![n(G3, dot(e())), n(G3, s()), n(G3, e()), n(G3, e()), n(G3, q()), n(G3, q())];
        assert_eq!(habanera().then(straight(2, q())).on(G3), old_bass);
        assert_ne!(habanera().repeat(2).on(G3), old_bass);
    }

    #[test]
    fn a_pattern_resolves_and_renders_like_any_music() {
        use crate::backends::midi::{render_midi, MidiOptions};
        use crate::resolve::resolve;
        let bass = tresillo().repeat(2).on(G3);
        let events = resolve(&bass).unwrap().events;
        assert_eq!(events.len(), 6);
        let onsets: Vec<Beats> = events.iter().map(|e| e.onset).collect();
        assert_eq!(onsets, vec![Beats::from_integer(0), dot(q()), dot(q()) + dot(q()), w(), w() + dot(q()), w() + dot(q()) + dot(q())]);
        assert!(render_midi(&bass, &MidiOptions::default()).is_ok());
    }
}
