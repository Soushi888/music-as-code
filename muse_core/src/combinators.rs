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
use crate::time::Beats;

impl Music {
    /// Chromatic transposition by `semis` semitones.
    pub fn transpose(self, semis: i32) -> Music {
        self.modify(Control::Transpose(semis))
    }

    /// Diatonic transposition: move N scale steps in the current key context.
    pub fn diatonic_transpose(self, steps: i32) -> Music {
        self.modify(Control::DiatonicTranspose(steps))
    }

    /// Time-stretch all durations by `factor`.
    pub fn augment(self, factor: Beats) -> Music {
        self.map_notes(move |mut note| {
            note.dur = note.dur * factor;
            note
        })
        .map_rests(move |dur| dur * factor)
    }

    /// Time-compress all durations by `factor`.
    pub fn diminish(self, factor: Beats) -> Music {
        self.augment(factor.recip())
    }

    /// Apply a function to every leaf `Note`.
    pub fn map_notes(self, f: impl Fn(Note) -> Note + Clone) -> Music {
        match self {
            Music::Note(note) => Music::Note(f(note)),
            Music::Rest(d) => Music::Rest(d),
            Music::Seq(v) => Music::Seq(v.into_iter().map(|m| m.map_notes(f.clone())).collect()),
            Music::Par(v) => Music::Par(v.into_iter().map(|m| m.map_notes(f.clone())).collect()),
            Music::Modify(ctrl, body) => Music::Modify(ctrl, Box::new(body.map_notes(f))),
        }
    }

    /// Apply a function to every leaf `Rest` duration.
    pub fn map_rests(self, f: impl Fn(Beats) -> Beats + Clone) -> Music {
        match self {
            Music::Note(note) => Music::Note(note),
            Music::Rest(d) => Music::Rest(f(d)),
            Music::Seq(v) => Music::Seq(v.into_iter().map(|m| m.map_rests(f.clone())).collect()),
            Music::Par(v) => Music::Par(v.into_iter().map(|m| m.map_rests(f.clone())).collect()),
            Music::Modify(ctrl, body) => Music::Modify(ctrl, Box::new(body.map_rests(f))),
        }
    }

    /// Retrograde: reverse sequential ordering within Seq nodes.
    pub fn retrograde(self) -> Music {
        match self {
            Music::Seq(v) => Music::Seq(v.into_iter().map(|m| m.retrograde()).rev().collect()),
            Music::Par(v) => Music::Par(v.into_iter().map(|m| m.retrograde()).collect()),
            Music::Modify(ctrl, body) => Music::Modify(ctrl, Box::new(body.retrograde())),
            leaf => leaf,
        }
    }

    /// Pitch inversion around `axis_midi`. Requires resolved chromatic pitches.
    pub fn invert(self, axis_midi: i32) -> Music {
        self.map_notes(move |mut note| {
            if let crate::pitch::Pitch::Chromatic(ref mut cp) = note.pitch {
                let dist = cp.midi() - axis_midi;
                let new_midi = axis_midi - dist;
                // Reconstruct chromatic pitch from MIDI number
                let oct = (new_midi / 12) - 1;
                let pc_idx = new_midi.rem_euclid(12);
                let (letter, acc) = midi_class_to_natural(pc_idx);
                cp.class = crate::pitch::PitchClass {
                    letter,
                    accidental: acc,
                };
                cp.octave = oct as i8;
            }
            note
        })
    }

    /// Apply a function pipeline step (for `.pipe(fn)` chaining).
    pub fn pipe(self, f: impl FnOnce(Music) -> Music) -> Music {
        f(self)
    }
}

fn midi_class_to_natural(semitone: i32) -> (crate::pitch::Letter, crate::pitch::Accidental) {
    use crate::pitch::{Accidental, Letter};
    match semitone {
        0 => (Letter::C, Accidental::Natural),
        1 => (Letter::C, Accidental::Sharp),
        2 => (Letter::D, Accidental::Natural),
        3 => (Letter::D, Accidental::Sharp),
        4 => (Letter::E, Accidental::Natural),
        5 => (Letter::F, Accidental::Natural),
        6 => (Letter::F, Accidental::Sharp),
        7 => (Letter::G, Accidental::Natural),
        8 => (Letter::G, Accidental::Sharp),
        9 => (Letter::A, Accidental::Natural),
        10 => (Letter::A, Accidental::Sharp),
        11 => (Letter::B, Accidental::Natural),
        _ => unreachable!("semitone must be 0..=11"),
    }
}

/// Apply swing feel: pairs of eighth notes become triplet long-short.
pub fn swing(m: Music) -> Music {
    // Swing is a rendering/interpretation pass; stub for now
    let _ = m;
    todo!("swing transform")
}

/// Humanize: deterministic jitter on durations and velocities.
pub fn humanize(seed: u64, amount: f32) -> impl Fn(Music) -> Music {
    move |_m| {
        let _ = (seed, amount);
        todo!("humanize transform")
    }
}

/// Build a canon from a melody: `voices` = list of (time_offset, transposition).
pub fn canon(voices: Vec<(Beats, i32)>) -> impl Fn(Music) -> Music {
    move |melody| {
        voices
            .iter()
            .map(|(offset, semis)| {
                let delay = r(*offset);
                delay + melody.clone().transpose(*semis)
            })
            .fold(Music::Par(vec![]), |acc, v| acc | v)
    }
}
