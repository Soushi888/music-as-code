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

use std::fmt;

use crate::attrs::{Articulation, NoteAttrs};
use crate::music::{r, Music, Note};
use crate::pitch::Pitch;
use crate::time::{dot, e, q, s, Beats};

/// How hard a hit is struck. The four levels map one to one onto the
/// articulations the resolver gives a velocity offset, so a rhythm's stress
/// and a note's articulation are the same fact written twice, not a
/// translation with a table to get wrong.
///
/// Written form, one character per hit: `o` ghost, `x` normal, `X` accent,
/// `^` marcato. The `^` echoes the `-^` shorthand a marcato note already
/// prints with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Stress {
    /// Under the line: audible but deliberately weak.
    Ghost,
    /// Struck normally. Carries no articulation at all.
    #[default]
    Normal,
    /// Stressed at the attack.
    Accent,
    /// Heavily stressed. The weight of a tango marcato.
    Marcato,
}

impl Stress {
    /// The articulation this stress writes onto a note. `Normal` writes none,
    /// so an unaccented pattern produces exactly the notes it produced before
    /// stress existed.
    pub fn articulation(self) -> Option<Articulation> {
        match self {
            Stress::Ghost => Some(Articulation::Ghost),
            Stress::Normal => None,
            Stress::Accent => Some(Articulation::Accent),
            Stress::Marcato => Some(Articulation::Marcato),
        }
    }

    /// The mark used in [`Pattern::accents`] and [`Pattern::accent_grid`].
    pub fn mark(self) -> char {
        match self {
            Stress::Ghost => 'o',
            Stress::Normal => 'x',
            Stress::Accent => 'X',
            Stress::Marcato => '^',
        }
    }

    /// The stress a mark means, or `None` for any other character.
    pub fn from_mark(c: char) -> Option<Self> {
        match c {
            'o' => Some(Stress::Ghost),
            'x' => Some(Stress::Normal),
            'X' => Some(Stress::Accent),
            '^' => Some(Stress::Marcato),
            _ => None,
        }
    }
}

impl fmt::Display for Stress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Stress::Ghost => "ghost",
            Stress::Normal => "normal",
            Stress::Accent => "accent",
            Stress::Marcato => "marcato",
        })
    }
}

/// Why an accent string does not fit the pattern it was given to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccentError {
    /// A character that is not one of `o x X ^`.
    BadMark {
        /// The offending character.
        mark: char,
        /// Its index in the string.
        at: usize,
    },
    /// More marks than the pattern has hits. Fewer is fine and cycles, but more
    /// means the caller is counting a hit that is not there, and silently
    /// dropping the tail would hide the miscount.
    TooManyMarks {
        /// How many marks were given.
        marks: usize,
        /// How many hits the pattern has.
        hits: usize,
    },
}

impl fmt::Display for AccentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccentError::BadMark { mark, at } => {
                write!(f, "'{mark}' at index {at} is not an accent mark (o x X ^)")
            }
            AccentError::TooManyMarks { marks, hits } => {
                write!(f, "{marks} accent marks for {hits} hits")
            }
        }
    }
}

impl std::error::Error for AccentError {}

/// One step of a [`Pattern`]: a struck note or a silence, each with a duration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Step {
    /// A note is struck and held for this long, at this stress.
    Hit {
        /// How long the note is held.
        dur: Beats,
        /// How hard it is struck.
        stress: Stress,
    },
    /// Silence for this long.
    Rest(Beats),
}

impl Step {
    /// A hit at normal stress.
    pub fn hit(dur: Beats) -> Self {
        Step::Hit { dur, stress: Stress::Normal }
    }

    /// The duration of the step, hit or rest.
    pub fn duration(self) -> Beats {
        match self {
            Step::Hit { dur, .. } | Step::Rest(dur) => dur,
        }
    }

    /// The stress of a hit; `None` for a rest.
    pub fn stress(self) -> Option<Stress> {
        match self {
            Step::Hit { stress, .. } => Some(stress),
            Step::Rest(_) => None,
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
        Self::new(durations.into_iter().map(Step::hit))
    }

    /// The steps, in order.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Number of struck notes (rests excluded).
    pub fn hit_count(&self) -> usize {
        self.steps.iter().filter(|s| matches!(s, Step::Hit { .. })).count()
    }

    /// Mark which hits lean, one character per hit from `o x X ^`. Rests are
    /// skipped and consume no character, so the string reads as the accents
    /// alone.
    ///
    /// Fewer marks than hits cycles, which is how a one-bar accent figure is
    /// written once and applied to a repeated pattern. More marks than hits is
    /// [`AccentError::TooManyMarks`]: the caller is counting a hit that is not
    /// there, and dropping the tail silently would hide the miscount.
    ///
    /// ```
    /// use musecode_core::prelude::*;
    /// // The tango marcato: weight on the downbeat, the anticipation accented.
    /// let bass = tresillo().accents("^xX").unwrap();
    /// assert_eq!(bass.accent_grid(), "^xX");
    /// ```
    pub fn accents(&self, marks: &str) -> Result<Pattern, AccentError> {
        let parsed: Vec<Stress> = marks
            .chars()
            .enumerate()
            .map(|(at, mark)| Stress::from_mark(mark).ok_or(AccentError::BadMark { mark, at }))
            .collect::<Result<_, _>>()?;
        if parsed.is_empty() {
            return Ok(self.clone());
        }
        let hits = self.hit_count();
        if parsed.len() > hits {
            return Err(AccentError::TooManyMarks { marks: parsed.len(), hits });
        }
        let mut next = parsed.iter().copied().cycle();
        Ok(Pattern {
            steps: self
                .steps
                .iter()
                .map(|step| match *step {
                    Step::Hit { dur, .. } => Step::Hit { dur, stress: next.next().unwrap_or_default() },
                    rest => rest,
                })
                .collect(),
        })
    }

    /// The pattern's stresses as a string, one character per step: `o x X ^`
    /// for hits and `.` for rests. The companion to
    /// [`grid`][Pattern::grid], which shows only hit against rest.
    pub fn accent_grid(&self) -> String {
        self.steps
            .iter()
            .map(|s| match s {
                Step::Hit { stress, .. } => stress.mark(),
                Step::Rest(_) => '.',
            })
            .collect()
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
                    Step::Hit { dur, stress } => struck(pitch, dur, stress),
                    Step::Rest(dur) => r(dur),
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
                    Step::Hit { dur, stress } => match next.next() {
                        Some(p) => struck(*p, dur, stress),
                        None => r(dur),
                    },
                    Step::Rest(dur) => r(dur),
                })
                .collect(),
        )
    }
}

/// One struck note: the stress becomes the note's articulation, and a normal
/// hit carries none, so an unaccented pattern is byte-for-byte what it was
/// before stress existed. Velocity is deliberately left unset: the resolver
/// turns the articulation into loudness relative to the dynamics in scope,
/// which is the only place that knows them.
fn struck(pitch: Pitch, dur: Beats, stress: Stress) -> Music {
    Music::Note(Note {
        pitch,
        dur,
        attrs: NoteAttrs { articulation: stress.articulation(), ..Default::default() },
    })
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
    use crate::attrs::Articulation;
    use crate::music::n;
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
        let p = Pattern::new([Step::hit(q()), Step::Rest(e()), Step::hit(e())]);
        assert_eq!(p.on(C3), seq![n(C3, q()), r(e()), n(C3, e())]);
        assert_eq!(p.on(C3).duration(), p.duration());
    }

    #[test]
    fn with_cycles_pitches_over_hits_only() {
        let p = Pattern::new([Step::hit(q()), Step::Rest(q()), Step::hit(q()), Step::hit(q())]);
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
    #[test]
    fn accents_cycle_over_hits_and_skip_rests() {
        let p = Pattern::new([Step::hit(q()), Step::Rest(q()), Step::hit(q()), Step::hit(q())]);
        // Three hits, two marks: ^ X ^. A rest must not eat a mark.
        let a = p.accents("^X").unwrap();
        assert_eq!(a.accent_grid(), "^.X^");
        assert_eq!(a.grid(), "x.xx", "the hit/rest grid is untouched");
        assert_eq!(a.duration(), p.duration());
        // One mark accents everything; an empty string changes nothing.
        assert_eq!(tresillo().accents("^").unwrap().accent_grid(), "^^^");
        assert_eq!(tresillo().accents("").unwrap(), tresillo());
        // Exactly as many marks as hits is the ordinary case.
        assert_eq!(tresillo().accents("^xX").unwrap().accent_grid(), "^xX");
        // A cycling figure over a repeated pattern: two marks, six hits.
        assert_eq!(tresillo().repeat(2).accents("^x").unwrap().accent_grid(), "^x^x^x");
    }

    #[test]
    fn a_bad_mark_is_an_error_naming_the_character() {
        assert_eq!(tresillo().accents("^.x"), Err(AccentError::BadMark { mark: '.', at: 1 }));
        assert_eq!(tresillo().accents("z"), Err(AccentError::BadMark { mark: 'z', at: 0 }));
        // The first offender wins, and the pattern is left alone.
        assert!(tresillo().accents("^^!!").is_err());
    }

    /// More marks than hits is a miscount, not a truncation: the caller is
    /// accenting a hit that is not there.
    #[test]
    fn more_marks_than_hits_is_an_error() {
        assert_eq!(
            tresillo().accents("^xXo"),
            Err(AccentError::TooManyMarks { marks: 4, hits: 3 })
        );
        // Rests do not count as hits, so this is four marks for two hits.
        let p = Pattern::new([Step::hit(q()), Step::Rest(q()), Step::hit(q())]);
        assert_eq!(p.accents("^x^x"), Err(AccentError::TooManyMarks { marks: 4, hits: 2 }));
        // A bad mark is reported before the count, since it is the earlier problem.
        assert!(matches!(tresillo().accents("^x!o"), Err(AccentError::BadMark { .. })));
        // The empty pattern has no hits, so any mark at all is too many.
        assert_eq!(
            Pattern::default().accents("x"),
            Err(AccentError::TooManyMarks { marks: 1, hits: 0 })
        );
        assert_eq!(Pattern::default().accents(""), Ok(Pattern::default()));
        // And the error reads as a sentence.
        assert_eq!(
            tresillo().accents("^xXo").unwrap_err().to_string(),
            "4 accent marks for 3 hits"
        );
    }

    #[test]
    fn stress_maps_one_to_one_onto_the_articulations_that_move_velocity() {
        assert_eq!(Stress::Normal.articulation(), None);
        assert_eq!(Stress::Ghost.articulation(), Some(Articulation::Ghost));
        assert_eq!(Stress::Accent.articulation(), Some(Articulation::Accent));
        assert_eq!(Stress::Marcato.articulation(), Some(Articulation::Marcato));
        for st in [Stress::Ghost, Stress::Normal, Stress::Accent, Stress::Marcato] {
            assert_eq!(Stress::from_mark(st.mark()), Some(st));
        }
        assert_eq!(Stress::from_mark('.'), None);
        assert_eq!(Stress::default(), Stress::Normal);
    }

    /// The whole point: `on` writes articulation, never velocity, so the
    /// dynamics of the piece still decide how loud the accent is.
    #[test]
    fn on_writes_articulation_and_leaves_velocity_to_the_dynamics() {
        use crate::control::Control;
        use crate::resolve::resolve;
        use crate::time::Dynamics;

        let bass = tresillo().accents("^xX").unwrap().on(C3);
        // Nothing in the tree carries a velocity.
        let Music::Seq(notes) = &bass else { panic!("on returns a Seq") };
        for note in notes {
            let Music::Note(note) = note else { panic!("all hits") };
            assert_eq!(note.attrs.velocity, None, "a pattern never writes velocity");
        }
        // Under mf (80) the three hits are marcato, plain and accent.
        let ev = resolve(&bass).unwrap().events;
        assert_eq!(ev.iter().map(|e| e.velocity).collect::<Vec<_>>(), vec![110, 80, 95]);
        // The same pattern under pp (33) scales with the dynamics, which is
        // exactly what writing an absolute velocity would have broken.
        let quiet = resolve(&bass.clone().modify(Control::Dynamics(Dynamics::Pp))).unwrap().events;
        assert_eq!(quiet.iter().map(|e| e.velocity).collect::<Vec<_>>(), vec![63, 33, 48]);
    }

    #[test]
    fn an_unaccented_pattern_is_what_it_always_was() {
        assert_eq!(tresillo().accent_grid(), "xxx");
        assert_eq!(tresillo().on(C3), seq![n(C3, dot(q())), n(C3, dot(q())), n(C3, q())]);
        // Normal stress writes no articulation at all.
        let Music::Seq(notes) = tresillo().on(C3) else { unreachable!() };
        for note in notes {
            let Music::Note(note) = note else { unreachable!() };
            assert_eq!(note.attrs, crate::attrs::NoteAttrs::default());
        }
    }

    #[test]
    fn accented_hits_print_their_articulation() {
        let m = tresillo().accents("^xo").unwrap().on(C3);
        assert_eq!(format!("{m}"), "C3:q.-^ C3:q. C3:q-ghost");
    }

}
