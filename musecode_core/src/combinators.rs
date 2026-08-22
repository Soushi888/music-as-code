//! Musical combinators: pure transformations on [`Music`] trees.
//!
//! All functions here are purely functional: they take a `Music` value and
//! return a transformed one. No mutation, no side effects. Compose freely:
//!
//! ```ignore
//! melody.transpose(5).augment(h()).pipe(canon(vec![(q(), 7)]))
//! ```

use crate::control::Control;
use crate::music::{r, Music, Note};
use crate::pitch::ChromaticPitch;
use crate::time::Beats;

impl Music {
    /// Transpose all pitches in this subtree by `semis` chromatic semitones.
    ///
    /// Wraps `self` in `Modify(Control::Transpose(semis), _)`. Resolution is
    /// deferred: backends apply the transposition when rendering. Positive values
    /// transpose up, negative values transpose down.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::prelude::*;
    /// let up_fifth = n(C4, q()).transpose(7);   // sounds as G4
    /// let down_oct = n(A4, q()).transpose(-12); // sounds as A3
    /// ```
    pub fn transpose(self, semis: i32) -> Music {
        self.modify(Control::Transpose(semis))
    }

    /// Transpose all scale-degree pitches by `steps` diatonic steps in the
    /// active [`Key`][crate::theory::Key] context.
    ///
    /// Unlike [`transpose`][Self::transpose], diatonic transposition moves to
    /// the next scale degree rather than a fixed number of semitones. A major
    /// second up from the 3rd degree is a 4th, not always a whole tone.
    ///
    /// Wraps `self` in `Modify(Control::DiatonicTranspose(steps), _)`.
    pub fn diatonic_transpose(self, steps: i32) -> Music {
        self.modify(Control::DiatonicTranspose(steps))
    }

    /// Multiply all note and rest durations by `factor`.
    ///
    /// `augment(h())` = `augment(b(2, 1))` doubles all durations (classical augmentation).
    /// `augment(b(3, 2))` creates dotted equivalents of all values.
    ///
    /// This is an eager transformation: durations are rewritten in the tree immediately,
    /// unlike [`transpose`][Self::transpose] which defers to the backend.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::prelude::*;
    /// let augmented = seq![n(C4, q()), n(E4, e())].augment(h());
    /// // durations become: h(), q()
    /// ```
    pub fn augment(self, factor: Beats) -> Music {
        self.map_notes(move |mut note| { note.dur = note.dur * factor; note })
            .map_rests(move |dur| dur * factor)
    }

    /// Divide all note and rest durations by `factor` (inverse of [`augment`][Self::augment]).
    ///
    /// `diminish(h())` = `diminish(b(2, 1))` halves all durations (classical diminution).
    /// Implemented as `augment(factor.recip())`.
    pub fn diminish(self, factor: Beats) -> Music {
        self.augment(factor.recip())
    }

    /// Apply `f` to every [`Note`] leaf in the tree, preserving structure.
    ///
    /// This is the primary building block for custom per-note transformations.
    /// `Seq`, `Par`, and `Modify` nodes are traversed recursively; `Rest` nodes
    /// are passed through unchanged.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::prelude::*;
    /// let melody = seq![n(C4, q()), n(E4, q()), n(G4, h())];
    /// // Accent every note at velocity 100
    /// let accented = melody.map_notes(|mut note| {
    ///     note.attrs.velocity = Some(100);
    ///     note
    /// });
    /// ```
    pub fn map_notes(self, f: impl Fn(Note) -> Note + Clone) -> Music {
        match self {
            Music::Note(note) => Music::Note(f(note)),
            Music::Rest(d) => Music::Rest(d),
            Music::Seq(v) => Music::Seq(v.into_iter().map(|m| m.map_notes(f.clone())).collect()),
            Music::Par(v) => Music::Par(v.into_iter().map(|m| m.map_notes(f.clone())).collect()),
            Music::Modify(ctrl, body) => Music::Modify(ctrl, Box::new(body.map_notes(f))),
        }
    }

    /// Apply `f` to every [`Rest`][Music::Rest] duration in the tree, preserving structure.
    ///
    /// `Note` leaves are passed through unchanged. Use this alongside
    /// [`map_notes`][Self::map_notes] to transform all durations uniformly
    /// (as [`augment`][Self::augment] does internally).
    pub fn map_rests(self, f: impl Fn(Beats) -> Beats + Clone) -> Music {
        match self {
            Music::Note(note) => Music::Note(note),
            Music::Rest(d) => Music::Rest(f(d)),
            Music::Seq(v) => Music::Seq(v.into_iter().map(|m| m.map_rests(f.clone())).collect()),
            Music::Par(v) => Music::Par(v.into_iter().map(|m| m.map_rests(f.clone())).collect()),
            Music::Modify(ctrl, body) => Music::Modify(ctrl, Box::new(body.map_rests(f))),
        }
    }

    /// Reverse the temporal order of a piece: last event plays first.
    ///
    /// Recursively reverses the children of every [`Seq`][Music::Seq] node.
    /// [`Par`][Music::Par] children are not reordered (parallel events stay
    /// parallel after retrograde). [`Note`][Music::Note] and [`Rest`][Music::Rest]
    /// leaves are returned unchanged.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::prelude::*;
    /// let forward  = seq![n(C4, q()), n(E4, q()), n(G4, h())];
    /// let backward = forward.retrograde(); // G4 h, E4 q, C4 q
    /// ```
    pub fn retrograde(self) -> Music {
        match self {
            Music::Seq(v) => Music::Seq(v.into_iter().map(|m| m.retrograde()).rev().collect()),
            Music::Par(v) => Music::Par(v.into_iter().map(|m| m.retrograde()).collect()),
            Music::Modify(ctrl, body) => Music::Modify(ctrl, Box::new(body.retrograde())),
            leaf => leaf,
        }
    }

    /// Pitch-invert all chromatic pitches around an axis given as a MIDI note number.
    ///
    /// A note at distance `d` semitones above `axis_midi` is reflected to `d` semitones
    /// below. Enharmonic spelling uses sharps for the reflected pitches.
    ///
    /// Only [`Pitch::Chromatic`][crate::pitch::Pitch::Chromatic] notes are affected;
    /// [`Pitch::Degree`][crate::pitch::Pitch::Degree] and
    /// [`Pitch::Interval`][crate::pitch::Pitch::Interval] notes are passed through.
    ///
    /// # Examples
    /// ```
    /// use musecode_core::prelude::*;
    /// // Invert around C4 (MIDI 60): G4 (67) becomes F3 (53)
    /// let inverted = n(G4, q()).invert(C4.midi());
    /// ```
    pub fn invert(self, axis_midi: i32) -> Music {
        self.map_notes(move |mut note| {
            if let crate::pitch::Pitch::Chromatic(ref mut cp) = note.pitch {
                let dist = cp.midi() - axis_midi;
                *cp = ChromaticPitch::from_midi(axis_midi - dist);
            }
            note
        })
    }

    /// Pass `self` through a `FnOnce(Music) -> Music` inline in a method chain.
    ///
    /// Enables named combinator functions to participate in dot-chaining without
    /// requiring them to be methods on `Music`:
    ///
    /// ```ignore
    /// melody
    ///     .transpose(5)
    ///     .augment(h())
    ///     .pipe(canon(vec![(q(), 7)]))
    ///     .pipe(humanize(42, 0.05))
    /// ```
    pub fn pipe(self, f: impl FnOnce(Music) -> Music) -> Music {
        f(self)
    }
}

/// Apply swing feel to a piece: consecutive eighth-note pairs become
/// a long-short triplet rhythm (first note takes 2/3, second takes 1/3).
///
/// # Status
/// Not yet implemented — will panic with `todo!()`.
pub fn swing(m: Music) -> Music {
    let _ = m;
    todo!("swing transform")
}

/// Return a closure that applies deterministic timing and velocity jitter.
///
/// `seed` controls the random sequence; the same seed always produces the same
/// jitter pattern. `amount` is a relative magnitude in the range `0.0..=1.0`.
///
/// # Status
/// Not yet implemented — the returned closure will panic with `todo!()`.
pub fn humanize(seed: u64, amount: f32) -> impl Fn(Music) -> Music {
    move |_m| {
        let _ = (seed, amount);
        todo!("humanize transform")
    }
}

/// Return a closure that builds a multi-voice canon from a melody.
///
/// Each entry in `voices` is `(time_offset, semitones)`: the canon voice enters
/// after `time_offset` beats and is transposed by `semitones` chromatic semitones.
///
/// The resulting closure wraps all voices in a `Par` node. Pass it to
/// [`Music::pipe`] to apply inline:
///
/// ```ignore
/// use musecode_core::prelude::*;
/// let two_voice = melody.pipe(canon(vec![
///     (b(0, 1), 0),   // original voice
///     (q(),     7),   // canon entry: one quarter later, a fifth above
/// ]));
/// ```
pub fn canon(voices: Vec<(Beats, i32)>) -> impl Fn(Music) -> Music {
    move |melody| {
        voices
            .iter()
            .map(|(offset, semis)| r(*offset) + melody.clone().transpose(*semis))
            .fold(Music::Par(vec![]), |acc, v| acc | v)
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    /// `invert` is an involution on sharps-spelled chromatic trees. Spelling
    /// round-trips only for sharps because `from_midi` is sharps-only; the
    /// MIDI numbers round-trip for any spelling.
    #[test]
    fn invert_is_an_involution() {
        let tree = seq![n(C4, q()), n(DS4, e()), n(G4, h()), n(AS4, q()), n(B3, s())];
        for axis in [0, 30, 60, 61, 72, 127] {
            assert_eq!(tree.clone().invert(axis).invert(axis), tree, "axis {axis}");
        }
    }

    #[test]
    fn invert_reflects_below_midi_zero_into_the_right_octave() {
        let inverted = n(CS4, q()).invert(30);
        match inverted {
            Music::Note(note) => match note.pitch {
                Pitch::Chromatic(cp) => {
                    assert_eq!(cp.midi(), -1);
                    assert_eq!(cp.octave, -2);
                }
                other => panic!("expected chromatic pitch, got {other:?}"),
            },
            other => panic!("expected a note, got {other:?}"),
        }
    }
}
