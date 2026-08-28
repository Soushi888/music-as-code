//! The resolver: from a [`Music`] tree to a flat, fully resolved event list.
//!
//! This is the one place that gives meaning to the polymorphic parts of the
//! IR. It walks the tree with an accumulated context (key, scale, transposition,
//! dynamics, articulation, voice, instrument, hints) and emits one [`Event`]
//! per sounding note with an absolute onset, a resolved [`ChromaticPitch`], a
//! velocity and a sounding duration. Every backend and the analysis layer read
//! the output of this pass; nothing downstream looks at a `Degree` or an
//! `Interval` again.
//!
//! Layer 5. May import `music`, `control`, `theory`, `pitch`, `time`, `attrs`
//! and `backends::hints`; must never import `phrase`, `combinators` or a
//! concrete backend.
//!
//! # Resolution rules
//!
//! - **Onsets.** `Seq` children start at the running sum of the preceding
//!   children's [`Music::duration`]; `Par` children all start at the `Par`'s
//!   onset; `Modify` does not move time.
//! - **Degrees** resolve against the active key's scale (or a `Control::Scale`
//!   override). Degree 1 with no octave shift is the tonic in octave 4, so
//!   degree 1 of F minor is F4 and degree 8 is F5. `alter` is a chromatic
//!   inflection of that scale step: `b3` in a major key is the minor third,
//!   `#7` in natural minor is the leading tone. For seven-note scales the
//!   letter comes from the scale step and the accidental from the semitone
//!   difference, so the flattened fifth of F minor is spelled Cb5, not B4.
//! - **Intervals** resolve against the previous note in the same `Seq` branch;
//!   each `Par` child inherits the `prev` from before the `Par` and does not
//!   see its siblings. With no previous note, the tonic anchor (tonic in
//!   octave 4) is used if a key is in scope; otherwise it is an error.
//! - **`Control::Transpose(n)`** is applied to the resolved chromatic pitch.
//!   Multiples of 12 move the octave and keep the spelling; anything else
//!   respells with sharps through [`ChromaticPitch::from_midi`].
//! - **`Control::DiatonicTranspose(n)`** moves `Degree` pitches by `n` scale
//!   steps, borrowing octaves so that degree 1 down one step is degree 7 an
//!   octave lower. Chromatic and interval pitches pass through unchanged.
//! - **Velocity** is the note's own `velocity` if set. Otherwise it is the
//!   dynamics level in scope through a fixed table (mf = 80), shifted by the
//!   articulation in scope (accent +15, marcato +30, ghost -25) and clamped to
//!   `1..=127`. `Crescendo` and `Decrescendo` leave the level unchanged.
//! - **Sounding duration** is the written duration scaled by the effective
//!   articulation: staccato 1/2, staccatissimo 1/4, everything else 1.
//! - **Ties.** `tie_to_next` merges a note with the next note in the same
//!   `Seq` branch when it resolves to the same pitch, summing both durations
//!   into one event. A following rest or a different pitch is an error. A
//!   `Modify` wrapper around the next note is transparent to the tie. A tie
//!   on the last note of a branch, or into a nested `Seq`/`Par`, is dropped
//!   silently; ties across boundaries are a later milestone.
//! - **Tempo and time signature** are recorded in [`Resolved::tempo_map`] and
//!   [`Resolved::time_sigs`] at the onset where the `Control` is met. A
//!   `Tempo::Ramp` is recorded as is; the MIDI backend reads its `from_bpm`.
//!
//! Errors, never panics: every condition a composer can reach from user code
//! is a [`ResolveError`] variant, including pitches pushed beyond the
//! representable octave range by extreme transpositions. The one exception
//! is inherited from [`Music::duration`]: `Rational32` overflows on pieces
//! millions of beats long, which M1 accepts as a known limit.

use crate::attrs::{Articulation, InstrumentId, VoiceId};
use crate::backends::BackendHint;
use crate::control::Control;
use crate::music::{Music, Note};
use crate::pitch::{ChromaticPitch, Degree, Interval, IntervalQuality, Letter, Pitch};
use crate::theory::{Key, Mode, Scale};
use crate::time::{Beats, Dynamics, Tempo, TimeSig};

/// One sounding note with every polymorphic part of the IR resolved.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Event {
    /// Absolute onset in beats from the start of the piece.
    pub onset: Beats,
    /// Sounding duration after articulation scaling.
    pub dur: Beats,
    /// The notated value before articulation, kept for notation backends.
    pub written_dur: Beats,
    /// Fully resolved pitch with its spelling preserved.
    pub pitch: ChromaticPitch,
    /// MIDI velocity, `1..=127`.
    pub velocity: u8,
    /// The note's own voice, else the voice in scope.
    pub voice: Option<VoiceId>,
    /// The instrument in scope, if any.
    pub instrument: Option<InstrumentId>,
    /// The effective articulation: the note's own, else the one in scope.
    pub articulation: Option<Articulation>,
    /// The note's hints followed by every enclosing `Control::Hint`, outermost first.
    pub hints: Vec<BackendHint>,
}

/// The output of [`resolve`]: events plus the conductor information a backend needs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Resolved {
    /// Every sounding note, sorted by onset (stable, so tree order breaks ties).
    pub events: Vec<Event>,
    /// `(onset, tempo)` for each `Control::Tempo` met, in onset order.
    pub tempo_map: Vec<(Beats, Tempo)>,
    /// `(onset, time signature)` for each `Control::TimeSignature` met, in onset order.
    pub time_sigs: Vec<(Beats, TimeSig)>,
    /// Total written duration, equal to [`Music::duration`] of the input.
    pub total: Beats,
}

/// Everything that can go wrong while resolving a tree. One variant per
/// condition; none of them panics.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResolveError {
    /// A `Degree` with `number == 0`; degrees are 1-based.
    DegreeZero,
    /// A `Degree` met with no `Control::Key` in scope.
    DegreeWithoutKey,
    /// A `Degree` under a `Mode::Custom` key with no `Control::Scale` override.
    CustomModeWithoutScale {
        /// The custom mode that had no scale.
        mode: Mode,
    },
    /// A `Degree` under a `Control::Scale` whose `intervals` is empty.
    EmptyScale,
    /// A pitch fell outside the octave range a [`ChromaticPitch`] can hold,
    /// usually through an extreme `Control::Transpose` or `octave_shift`.
    PitchOutOfRange {
        /// The MIDI number that could not be represented.
        semitone: i64,
    },
    /// An `Interval` with no previous note in its branch and no key to anchor on.
    UnanchoredInterval,
    /// An `Interval` with `generic == 0`; a unison is `1`.
    ZeroInterval,
    /// A quality that does not exist for that interval size, such as a major fifth.
    InvalidIntervalQuality {
        /// The signed generic size.
        generic: i8,
        /// The quality that was asked for.
        quality: IntervalQuality,
    },
    /// `tie_to_next` followed, in the same branch, by a rest or a note of a different pitch.
    TieMismatch {
        /// Onset of the event that failed to continue the tie.
        at: Beats,
    },
    /// No accidental in `DoubleFlat..=DoubleSharp` spells this semitone with this letter.
    UnspellablePitch {
        /// The MIDI number that was being spelled.
        semitone: i32,
        /// The letter the scale step demanded.
        letter: Letter,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::DegreeZero => write!(f, "scale degree 0 does not exist; degrees start at 1"),
            ResolveError::DegreeWithoutKey => write!(f, "a scale degree was used with no key in scope"),
            ResolveError::CustomModeWithoutScale { mode } => {
                write!(f, "{mode:?} has no interval content; wrap the passage in Control::Scale")
            }
            ResolveError::EmptyScale => write!(f, "a Control::Scale with no intervals cannot resolve a degree"),
            ResolveError::PitchOutOfRange { semitone } => {
                write!(f, "MIDI {semitone} is outside the range a ChromaticPitch can represent")
            }
            ResolveError::UnanchoredInterval => {
                write!(f, "an interval was used with no previous note and no key to anchor on")
            }
            ResolveError::ZeroInterval => write!(f, "interval size 0 does not exist; a unison is 1"),
            ResolveError::InvalidIntervalQuality { generic, quality } => {
                write!(f, "a {quality:?} interval of generic size {generic} does not exist")
            }
            ResolveError::TieMismatch { at } => {
                write!(f, "tie at beat {at} is not followed by a note of the same pitch")
            }
            ResolveError::UnspellablePitch { semitone, letter } => {
                write!(f, "MIDI {semitone} cannot be spelled with the letter {letter:?}")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolve a tree to its flat event list.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// let triad = seq![n(d!(1), q()), n(d!(3), q()), n(d!(5), h())]
///     .modify(Control::Key(Key::major(pc!(C))));
/// let resolved = resolve(&triad).unwrap();
/// let pitches: Vec<_> = resolved.events.iter().map(|e| e.pitch).collect();
/// assert_eq!(pitches, vec![C4, E4, G4]);
/// assert_eq!(resolved.total, w());
/// ```
pub fn resolve(music: &Music) -> Result<Resolved, ResolveError> {
    let mut out = Resolved { total: music.duration(), ..Default::default() };
    let ctx = Context::default();
    walk(music, Beats::from_integer(0), &ctx, None, &mut out)?;
    out.events.sort_by_key(|e| e.onset);
    out.tempo_map.sort_by_key(|(onset, _)| *onset);
    out.time_sigs.sort_by_key(|(onset, _)| *onset);
    Ok(out)
}

/// Context carried down the tree. Cloned at each `Modify` and `Par` child.
#[derive(Clone, Debug)]
struct Context {
    key: Option<Key>,
    scale: Option<Scale>,
    transpose: i32,
    diatonic: i32,
    dynamics: Dynamics,
    articulation: Option<Articulation>,
    voice: Option<VoiceId>,
    instrument: Option<InstrumentId>,
    hints: Vec<BackendHint>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            key: None,
            scale: None,
            transpose: 0,
            diatonic: 0,
            dynamics: Dynamics::Mf,
            articulation: None,
            voice: None,
            instrument: None,
            hints: Vec::new(),
        }
    }
}

/// The previous resolved pitch in the current `Seq` branch.
type Prev = Option<ChromaticPitch>;

/// Walk one node. Returns the `prev` to thread into the next sibling.
fn walk(
    music: &Music,
    onset: Beats,
    ctx: &Context,
    prev: Prev,
    out: &mut Resolved,
) -> Result<Prev, ResolveError> {
    match music {
        Music::Note(note) => emit(note, onset, ctx, prev, None, out).map(|(p, _)| Some(p)),
        Music::Rest(_) => Ok(prev),
        Music::Seq(children) => walk_seq(children, onset, ctx, prev, out),
        Music::Par(children) => {
            for child in children {
                walk(child, onset, ctx, prev, out)?;
            }
            Ok(prev)
        }
        Music::Modify(_, _) => {
            let (inner, leaf) = unwrap_modify(music, onset, ctx, out);
            walk(leaf, onset, &inner, prev, out)
        }
    }
}

/// Peel a chain of `Modify` wrappers: record tempo and time-signature
/// changes at `onset`, fold every other control into the context, and return
/// the context and the innermost node.
fn unwrap_modify<'m>(mut music: &'m Music, onset: Beats, ctx: &Context, out: &mut Resolved) -> (Context, &'m Music) {
    let mut inner = ctx.clone();
    while let Music::Modify(control, body) = music {
        match control {
            Control::Tempo(t) => out.tempo_map.push((onset, t.clone())),
            Control::TimeSignature(ts) => out.time_sigs.push((onset, *ts)),
            Control::User(_, _) => {}
            other => inner = inner.apply(other),
        }
        music = body;
    }
    (inner, music)
}

/// A `Seq` threads `prev` through its children and owns tie merging.
fn walk_seq(
    children: &[Music],
    onset: Beats,
    ctx: &Context,
    mut prev: Prev,
    out: &mut Resolved,
) -> Result<Prev, ResolveError> {
    let mut t = onset;
    // Index into `out.events` of an event whose note was tied to the next one.
    let mut tie_from: Option<usize> = None;
    for child in children {
        // A Modify wrapper is transparent to ties and to `prev`: look through it.
        let (inner, leaf) = match child {
            Music::Modify(_, _) => unwrap_modify(child, t, ctx, out),
            other => (ctx.clone(), other),
        };
        match leaf {
            Music::Note(note) => {
                let (pitch, idx) = emit(note, t, &inner, prev, tie_from, out)?;
                prev = Some(pitch);
                tie_from = note.attrs.tie_to_next.then_some(idx);
            }
            Music::Rest(_) => {
                if tie_from.is_some() {
                    return Err(ResolveError::TieMismatch { at: t });
                }
            }
            other => {
                tie_from = None;
                prev = walk(other, t, &inner, prev, out)?;
            }
        }
        t += child.duration();
    }
    Ok(prev)
}

/// Resolve one note into an event, merging it into `tie_from` when tied.
/// Returns the resolved pitch *before* chromatic transposition (what the
/// next interval anchors on) and the index of the event that carries it.
fn emit(
    note: &Note,
    onset: Beats,
    ctx: &Context,
    prev: Prev,
    tie_from: Option<usize>,
    out: &mut Resolved,
) -> Result<(ChromaticPitch, usize), ResolveError> {
    let base = resolve_pitch(&note.pitch, ctx, prev)?;
    let pitch = transposed(base, ctx.transpose)?;
    let articulation = note.attrs.articulation.or(ctx.articulation);
    let sounding = note.dur * articulation_scale(articulation);

    if let Some(idx) = tie_from {
        let tied = &mut out.events[idx];
        if tied.pitch != pitch {
            return Err(ResolveError::TieMismatch { at: onset });
        }
        tied.dur += sounding;
        tied.written_dur += note.dur;
        return Ok((base, idx));
    }

    let velocity = match note.attrs.velocity {
        // An explicit velocity is the caller saying exactly what they mean; the
        // stress articulations do not second-guess it.
        Some(v) => v.clamp(1, 127),
        None => (i16::from(velocity_for(ctx.dynamics)) + velocity_offset(articulation)).clamp(1, 127) as u8,
    };
    let mut hints = note.attrs.hints.clone();
    hints.extend(ctx.hints.iter().cloned());
    out.events.push(Event {
        onset,
        dur: sounding,
        written_dur: note.dur,
        pitch,
        velocity,
        voice: note.attrs.voice_id.clone().or_else(|| ctx.voice.clone()),
        instrument: ctx.instrument.clone(),
        articulation,
        hints,
    });
    Ok((base, out.events.len() - 1))
}

impl Context {
    fn apply(&self, control: &Control) -> Context {
        let mut next = self.clone();
        match control {
            Control::Key(k) => {
                next.key = Some(*k);
                // A new key discards a scale override set for the old one.
                next.scale = None;
            }
            Control::Scale(s) => next.scale = Some(s.clone()),
            Control::Instrument(i) => next.instrument = Some(i.clone()),
            Control::Transpose(n) => next.transpose = next.transpose.saturating_add(*n),
            Control::DiatonicTranspose(n) => next.diatonic = next.diatonic.saturating_add(*n),
            Control::Dynamics(d) => match d {
                Dynamics::Crescendo | Dynamics::Decrescendo => {}
                level => next.dynamics = *level,
            },
            Control::Articulation(a) => next.articulation = Some(*a),
            Control::Voice(v) => next.voice = Some(v.clone()),
            Control::Hint(h) => next.hints.push(h.clone()),
            Control::Tempo(_) | Control::TimeSignature(_) | Control::User(_, _) => {}
        }
        next
    }

    /// The scale intervals in scope, or the error that explains why there are none.
    fn scale_intervals(&self) -> Result<(Key, Vec<i8>), ResolveError> {
        let key = self.key.ok_or(ResolveError::DegreeWithoutKey)?;
        if let Some(scale) = &self.scale {
            if scale.intervals.is_empty() {
                return Err(ResolveError::EmptyScale);
            }
            return Ok((key, scale.intervals.clone()));
        }
        match key.mode.intervals() {
            Some(iv) => Ok((key, iv.to_vec())),
            None => Err(ResolveError::CustomModeWithoutScale { mode: key.mode }),
        }
    }

    /// The tonic in octave 4, before any chromatic transposition.
    fn tonic_anchor(&self) -> Option<ChromaticPitch> {
        self.key.map(|key| ChromaticPitch { class: key.tonic, octave: 4 })
    }
}

/// Resolve a pitch to its chromatic form *before* `Control::Transpose`,
/// which [`emit`] applies uniformly afterwards. `prev` and the tonic anchor
/// are likewise untransposed, so an interval inside its own `transpose`
/// subtree is transposed exactly once.
fn resolve_pitch(pitch: &Pitch, ctx: &Context, prev: Prev) -> Result<ChromaticPitch, ResolveError> {
    match pitch {
        Pitch::Chromatic(cp) => Ok(*cp),
        Pitch::Degree(d) => resolve_degree(d, ctx),
        Pitch::Interval(iv) => {
            let anchor = prev.or_else(|| ctx.tonic_anchor()).ok_or(ResolveError::UnanchoredInterval)?;
            resolve_interval(iv, anchor)
        }
    }
}

fn resolve_degree(degree: &Degree, ctx: &Context) -> Result<ChromaticPitch, ResolveError> {
    if degree.number == 0 {
        return Err(ResolveError::DegreeZero);
    }
    let (key, intervals) = ctx.scale_intervals()?;
    let len = intervals.len() as i64;
    let steps = degree.number as i64 - 1 + ctx.diatonic as i64;
    let idx = steps.rem_euclid(len);
    let wrap = steps.div_euclid(len);
    let tonic_midi = ChromaticPitch { class: key.tonic, octave: 4 }.midi() as i64;
    let midi = tonic_midi
        + intervals[idx as usize] as i64
        + degree.alter as i64
        + 12 * (wrap + degree.octave_shift as i64);
    if len == 7 {
        spell(midi, key.tonic.letter.step(idx as i32))
    } else {
        from_midi_checked(midi)
    }
}

/// The MIDI numbers whose octave fits in an `i8` for every spelling.
const MIDI_RANGE: std::ops::RangeInclusive<i64> = -1500..=1500;

fn from_midi_checked(midi: i64) -> Result<ChromaticPitch, ResolveError> {
    if !MIDI_RANGE.contains(&midi) {
        return Err(ResolveError::PitchOutOfRange { semitone: midi });
    }
    Ok(ChromaticPitch::from_midi(midi as i32))
}

fn spell(midi: i64, letter: Letter) -> Result<ChromaticPitch, ResolveError> {
    if !MIDI_RANGE.contains(&midi) {
        return Err(ResolveError::PitchOutOfRange { semitone: midi });
    }
    ChromaticPitch::with_letter(midi as i32, letter)
        .ok_or(ResolveError::UnspellablePitch { semitone: midi as i32, letter })
}

fn resolve_interval(iv: &Interval, anchor: ChromaticPitch) -> Result<ChromaticPitch, ResolveError> {
    if iv.generic == 0 {
        return Err(ResolveError::ZeroInterval);
    }
    let size = iv.generic.unsigned_abs() as i32;
    let simple = (size - 1).rem_euclid(7);
    let octaves = (size - 1).div_euclid(7);
    let perfect_class = matches!(simple, 0 | 3 | 4);
    let base = [0, 2, 4, 5, 7, 9, 11][simple as usize];
    let invalid = || ResolveError::InvalidIntervalQuality { generic: iv.generic, quality: iv.quality };
    let adjust = match (perfect_class, iv.quality) {
        (true, IntervalQuality::Perfect) => 0,
        (true, IntervalQuality::Augmented) => 1,
        (true, IntervalQuality::Diminished) => -1,
        (true, IntervalQuality::DoublyAugmented) => 2,
        (true, IntervalQuality::DoublyDiminished) => -2,
        (true, IntervalQuality::Major | IntervalQuality::Minor) => return Err(invalid()),
        (false, IntervalQuality::Major) => 0,
        (false, IntervalQuality::Minor) => -1,
        (false, IntervalQuality::Augmented) => 1,
        (false, IntervalQuality::Diminished) => -2,
        (false, IntervalQuality::DoublyAugmented) => 2,
        (false, IntervalQuality::DoublyDiminished) => -3,
        (false, IntervalQuality::Perfect) => return Err(invalid()),
    };
    let semitones = base + adjust + 12 * octaves;
    let sign = iv.generic.signum() as i32;
    let midi = anchor.midi() as i64 + (sign * semitones) as i64;
    spell(midi, anchor.class.letter.step(sign * (size - 1)))
}

/// Apply a chromatic transposition. Multiples of twelve keep the spelling;
/// anything else respells with sharps (ADR-008).
fn transposed(p: ChromaticPitch, semitones: i32) -> Result<ChromaticPitch, ResolveError> {
    if semitones == 0 {
        return Ok(p);
    }
    let midi = p.midi() as i64 + semitones as i64;
    if semitones % 12 == 0 {
        let octave = p.octave as i64 + (semitones / 12) as i64;
        let octave = i8::try_from(octave).map_err(|_| ResolveError::PitchOutOfRange { semitone: midi })?;
        Ok(ChromaticPitch { class: p.class, octave })
    } else {
        from_midi_checked(midi)
    }
}

/// Velocity offset for the three stress articulations, applied on top of the
/// dynamics-derived velocity. Accent and marcato lean into the note, ghost puts
/// it under the line; every other articulation leaves velocity alone.
///
/// The result is clamped to `1..=127` at the call site, and the quiet end is
/// the one that matters: a velocity-0 note-on is a note-off on most
/// synthesizers, so an unclamped ghost under `ppp` would delete the note
/// instead of softening it.
fn velocity_offset(a: Option<Articulation>) -> i16 {
    match a {
        Some(Articulation::Accent) => 15,
        Some(Articulation::Marcato) => 30,
        Some(Articulation::Ghost) => -25,
        _ => 0,
    }
}

fn articulation_scale(a: Option<Articulation>) -> Beats {
    match a {
        Some(Articulation::Staccato) => Beats::new(1, 2),
        Some(Articulation::Staccatissimo) => Beats::new(1, 4),
        _ => Beats::from_integer(1),
    }
}

/// The fixed dynamics-to-velocity table. Hairpins map to mf; they never
/// reach here because [`Context::apply`] leaves the level unchanged for them.
fn velocity_for(d: Dynamics) -> u8 {
    match d {
        Dynamics::Ppp => 16,
        Dynamics::Pp => 33,
        Dynamics::P => 49,
        Dynamics::Mp => 64,
        Dynamics::Mf => 80,
        Dynamics::F => 96,
        Dynamics::Ff => 112,
        Dynamics::Fff => 127,
        Dynamics::Sfz => 120,
        Dynamics::Fp => 96,
        Dynamics::Crescendo | Dynamics::Decrescendo => 80,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::NoteAttrs;
    use crate::pitch::Accidental;
    use crate::prelude::*;

    fn pitches(m: &Music) -> Vec<ChromaticPitch> {
        resolve(m).unwrap().events.iter().map(|e| e.pitch).collect()
    }

    fn midis(m: &Music) -> Vec<i32> {
        pitches(m).iter().map(|p| p.midi()).collect()
    }

    fn cp(letter: Letter, accidental: Accidental, octave: i8) -> ChromaticPitch {
        ChromaticPitch::new(letter, accidental, octave)
    }

    fn iv(generic: i8, quality: IntervalQuality) -> Interval {
        Interval { generic, quality }
    }

    /// The README melody: degrees 5 b5 4 3 in F minor.
    fn readme_melody() -> Music {
        seq![n(d!(5), q()), n(d!(b 5), e()), n(d!(4), e()), n(d!(3), h())]
    }

    fn in_f_minor(m: Music) -> Music {
        m.modify(Control::Key(Key::minor(pc!(F))))
    }

    // --- degrees ---

    #[test]
    fn c_major_triad_in_degrees() {
        let m = seq![n(d!(1), q()), n(d!(3), q()), n(d!(5), h())].modify(Control::Key(Key::major(pc!(C))));
        assert_eq!(pitches(&m), vec![C4, E4, G4]);
    }

    #[test]
    fn readme_melody_in_f_minor_is_spelled_from_the_scale() {
        // 5 = C5; b5 = Cb5 (MIDI 71, not B4); 4 = Bb4; 3 = Ab4, the third of the
        // minor scale as it stands.
        let got = pitches(&in_f_minor(readme_melody()));
        assert_eq!(got, vec![C5, cp(Letter::C, Accidental::Flat, 5), BB4, AB4]);
        assert_eq!(midis(&in_f_minor(readme_melody())), vec![72, 71, 70, 68]);
    }

    #[test]
    fn alter_inflects_the_scale_step_so_b3_in_minor_is_a_double_flat() {
        let m = in_f_minor(n(d!(b 3), h()));
        assert_eq!(pitches(&m), vec![cp(Letter::A, Accidental::DoubleFlat, 4)]);
        assert_eq!(midis(&m), vec![67]);
    }

    #[test]
    fn degree_eight_is_the_tonic_an_octave_up_and_extensions_wrap() {
        let m = in_f_minor(seq![n(d!(1), q()), n(d!(8), q()), n(d!(9), q()), n(d!(13), q())]);
        assert_eq!(pitches(&m), vec![F4, F5, G5, cp(Letter::D, Accidental::Flat, 6)]);
    }

    #[test]
    fn octave_shift_moves_whole_octaves() {
        let low = Degree { number: 1, alter: 0, octave_shift: -1 };
        let high = Degree { number: 5, alter: 1, octave_shift: 1 };
        let m = in_f_minor(seq![n(low, q()), n(high, q())]);
        assert_eq!(pitches(&m), vec![F3, cp(Letter::C, Accidental::Sharp, 6)]);
    }

    #[test]
    fn nested_key_shadows_only_its_subtree() {
        let inner = n(d!(3), q()).modify(Control::Key(Key::major(pc!(C))));
        let m = in_f_minor(seq![n(d!(3), q()), inner, n(d!(3), q())]);
        assert_eq!(pitches(&m), vec![AB4, E4, AB4]);
    }

    #[test]
    fn scale_override_replaces_the_mode_and_a_new_key_clears_it() {
        let lydian_f = Scale::custom(vec![0, 2, 4, 6, 7, 9, 11]);
        let m = in_f_minor(
            seq![n(d!(4), q()), n(d!(4), q()).modify(Control::Key(Key::minor(pc!(F))))]
                .modify(Control::Scale(lydian_f)),
        );
        assert_eq!(pitches(&m), vec![B4, BB4]);
    }

    #[test]
    fn custom_mode_resolves_through_a_scale_override() {
        let m = n(d!(3), q())
            .modify(Control::Scale(Scale::custom(vec![0, 2, 4, 7, 9])))
            .modify(Control::Key(Key::new(pc!(C), Mode::Custom(1))));
        assert_eq!(pitches(&m), vec![E4]);
    }

    #[test]
    fn non_heptatonic_scales_spell_with_sharps() {
        // C major pentatonic, degree 6 wraps to C5; degree b3 is MIDI 63 = D#4.
        let pent = Scale::custom(vec![0, 2, 4, 7, 9]);
        let m = seq![n(d!(6), q()), n(d!(b 3), q())]
            .modify(Control::Scale(pent))
            .modify(Control::Key(Key::major(pc!(C))));
        assert_eq!(pitches(&m), vec![C5, DS4]);
    }

    // --- transposition ---

    #[test]
    fn chromatic_transpose_moves_resolved_output() {
        let m = in_f_minor(readme_melody()).transpose(7);
        assert_eq!(midis(&m), vec![79, 78, 77, 75]);
        // Sharps-only respelling (ADR-008).
        assert_eq!(pitches(&m)[1], cp(Letter::F, Accidental::Sharp, 5));
    }

    #[test]
    fn transpose_by_octaves_keeps_the_spelling() {
        let m = seq![n(EB4, q()), n(d!(b 5), q())].modify(Control::Key(Key::minor(pc!(F)))).transpose(-12);
        assert_eq!(pitches(&m), vec![cp(Letter::E, Accidental::Flat, 3), cp(Letter::C, Accidental::Flat, 4)]);
    }

    #[test]
    fn nested_transposes_sum() {
        let m = n(C4, q()).transpose(2).transpose(3);
        assert_eq!(midis(&m), vec![65]);
    }

    #[test]
    fn diatonic_transpose_moves_every_degree_one_step_in_f_minor() {
        let shifted = in_f_minor(readme_melody().diatonic_transpose(-1));
        let explicit = in_f_minor(seq![n(d!(4), q()), n(d!(b 4), e()), n(d!(3), e()), n(d!(2), h())]);
        assert_eq!(resolve(&shifted).unwrap(), resolve(&explicit).unwrap());
        assert_eq!(midis(&shifted), vec![70, 69, 68, 67]);
    }

    #[test]
    fn diatonic_transpose_borrows_octaves_and_ignores_chromatic_pitches() {
        let m = in_f_minor(seq![n(d!(1), q()), n(C4, q())].diatonic_transpose(-1));
        assert_eq!(pitches(&m), vec![cp(Letter::E, Accidental::Flat, 4), C4]);
    }

    // --- intervals ---

    #[test]
    fn intervals_resolve_against_the_previous_note_with_spelling() {
        let m = seq![
            n(EB4, q()),
            n(iv(3, IntervalQuality::Major), q()),   // G4
            n(iv(-5, IntervalQuality::Perfect), q()), // C4
            n(iv(2, IntervalQuality::Minor), q()),    // Db4
        ];
        assert_eq!(pitches(&m), vec![EB4, G4, C4, DB4]);
    }

    #[test]
    fn diminished_fifth_above_b_is_f_natural() {
        let m = seq![n(B3, q()), n(iv(5, IntervalQuality::Diminished), q())];
        assert_eq!(pitches(&m), vec![B3, F4]);
    }

    #[test]
    fn compound_intervals_add_octaves() {
        let m = seq![n(C4, q()), n(iv(10, IntervalQuality::Major), q()), n(iv(-8, IntervalQuality::Perfect), q())];
        assert_eq!(pitches(&m), vec![C4, E5, E4]);
    }

    #[test]
    fn transpose_applies_once_to_intervals_whatever_the_modify_wraps() {
        // The interval note alone is transposed: E4 + 7 = B4.
        let inner = seq![n(C4, q()), n(iv(3, IntervalQuality::Major), q()).transpose(7)];
        assert_eq!(pitches(&inner), vec![C4, B4]);
        // The anchor alone is transposed: the following interval is outside the subtree, so E4.
        let anchor_only = seq![n(C4, q()).transpose(7), n(iv(3, IntervalQuality::Major), q())];
        assert_eq!(pitches(&anchor_only), vec![G4, E4]);
        // Both inside one subtree: each transposed once.
        let both = seq![n(C4, q()), n(iv(3, IntervalQuality::Major), q())].transpose(7);
        assert_eq!(pitches(&both), vec![G4, B4]);
    }

    #[test]
    fn first_interval_anchors_on_the_tonic_when_a_key_is_in_scope() {
        let m = in_f_minor(n(iv(5, IntervalQuality::Perfect), q()));
        assert_eq!(pitches(&m), vec![C5]);
        // The anchor follows the chromatic transposition in scope.
        let up = in_f_minor(n(iv(5, IntervalQuality::Perfect), q())).transpose(1);
        assert_eq!(midis(&up), vec![73]);
    }

    #[test]
    fn par_children_share_the_prev_from_before_the_par() {
        let m = seq![
            n(C4, q()),
            par![n(iv(3, IntervalQuality::Major), q()), n(iv(5, IntervalQuality::Perfect), q())],
            n(iv(2, IntervalQuality::Major), q()),
        ];
        // After the Par, prev is still C4, so the final M2 is D4.
        assert_eq!(pitches(&m), vec![C4, E4, G4, D4]);
    }

    // --- time ---

    #[test]
    fn par_shares_an_onset_and_seq_runs_forward() {
        let m = seq![n(C4, q()), par![n(E4, h()), seq![n(G4, e()), n(A4, e())]], n(B4, q())];
        let r = resolve(&m).unwrap();
        let onsets: Vec<Beats> = r.events.iter().map(|e| e.onset).collect();
        assert_eq!(onsets, vec![b(0, 1), b(1, 1), b(1, 1), b(3, 2), b(3, 1)]);
        assert_eq!(r.total, w());
    }

    #[test]
    fn events_are_sorted_by_onset_with_tree_order_for_ties() {
        let m = par![seq![r(q()), n(C4, q())], seq![n(E4, e()), n(G4, e()), n(A4, q())]];
        let got: Vec<(Beats, ChromaticPitch)> = resolve(&m).unwrap().events.iter().map(|e| (e.onset, e.pitch)).collect();
        assert_eq!(got, vec![(b(0, 1), E4), (b(1, 2), G4), (b(1, 1), C4), (b(1, 1), A4)]);
    }

    #[test]
    fn tempo_and_time_signature_are_recorded_at_their_onset() {
        let m = seq![
            n(C4, w()),
            n(D4, w()).modify(Control::Tempo(Tempo::bpm(140))).modify(Control::TimeSignature(TimeSig::waltz())),
        ]
        .modify(Control::Tempo(Tempo::bpm(96)))
        .modify(Control::TimeSignature(TimeSig::common()));
        let r = resolve(&m).unwrap();
        assert_eq!(r.tempo_map, vec![(b(0, 1), Tempo::bpm(96)), (w(), Tempo::bpm(140))]);
        assert_eq!(r.time_sigs, vec![(b(0, 1), TimeSig::common()), (w(), TimeSig::waltz())]);
    }

    // --- ties ---

    #[test]
    fn two_tied_quarters_make_one_half_event() {
        let tied = Music::Note(Note {
            pitch: C4.into(),
            dur: q(),
            attrs: NoteAttrs { tie_to_next: true, ..Default::default() },
        });
        let m = seq![tied, n(C4, q()), n(D4, q())];
        let r = resolve(&m).unwrap();
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[0].dur, h());
        assert_eq!(r.events[0].written_dur, h());
        assert_eq!(r.events[1].onset, h());
    }

    #[test]
    fn a_chain_of_ties_merges_into_one_event() {
        let tied = |p: ChromaticPitch| Music::Note(Note { pitch: p.into(), dur: q(), attrs: NoteAttrs { tie_to_next: true, ..Default::default() } });
        let m = seq![tied(G4), tied(G4), n(G4, q())];
        let r = resolve(&m).unwrap();
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.events[0].dur, dot(h()));
    }

    #[test]
    fn a_modify_wrapper_is_transparent_to_ties() {
        let tied = Music::Note(Note { pitch: C4.into(), dur: q(), attrs: NoteAttrs { tie_to_next: true, ..Default::default() } });
        let m = seq![tied.clone(), n(C4, q()).modify(Control::Dynamics(Dynamics::F))];
        let r = resolve(&m).unwrap();
        assert_eq!(r.events.len(), 1);
        assert_eq!(r.events[0].dur, h());
        let mismatch = seq![tied, n(D4, q()).modify(Control::Articulation(Articulation::Tenuto))];
        assert_eq!(resolve(&mismatch), Err(ResolveError::TieMismatch { at: q() }));
    }

    #[test]
    fn tempo_ramp_is_recorded_whole() {
        let ramp = Tempo::Ramp { from_bpm: 60, to_bpm: 120, over_beats: 4 };
        let r = resolve(&n(C4, q()).modify(Control::Tempo(ramp.clone()))).unwrap();
        assert_eq!(r.tempo_map, vec![(b(0, 1), ramp)]);
    }

    #[test]
    fn tie_on_the_last_note_of_a_branch_is_dropped() {
        let tied = Music::Note(Note { pitch: C4.into(), dur: q(), attrs: NoteAttrs { tie_to_next: true, ..Default::default() } });
        let r = resolve(&seq![n(D4, q()), tied]).unwrap();
        assert_eq!(r.events.len(), 2);
        assert_eq!(r.events[1].dur, q());
    }

    // --- dynamics, articulation, voice, hints ---

    #[test]
    fn velocity_comes_from_the_note_else_the_dynamics_in_scope() {
        let loud = Music::Note(Note { pitch: C4.into(), dur: q(), attrs: NoteAttrs { velocity: Some(127), ..Default::default() } });
        let m = seq![n(C4, q()), loud, n(C4, q()).modify(Control::Dynamics(Dynamics::Pp))]
            .modify(Control::Dynamics(Dynamics::Crescendo))
            .modify(Control::Dynamics(Dynamics::F));
        let v: Vec<u8> = resolve(&m).unwrap().events.iter().map(|e| e.velocity).collect();
        assert_eq!(v, vec![96, 127, 33]);
        let bare = resolve(&n(C4, q())).unwrap();
        assert_eq!(bare.events[0].velocity, 80);
    }

    #[test]
    fn staccato_halves_the_sounding_duration_and_keeps_the_written_one() {
        let short = Music::Note(Note { pitch: C4.into(), dur: q(), attrs: NoteAttrs { articulation: Some(Articulation::Staccatissimo), ..Default::default() } });
        let m = seq![n(C4, q()), short, n(C4, q()).modify(Control::Articulation(Articulation::Tenuto))]
            .modify(Control::Articulation(Articulation::Staccato));
        let r = resolve(&m).unwrap();
        let durs: Vec<Beats> = r.events.iter().map(|e| e.dur).collect();
        assert_eq!(durs, vec![e(), s(), q()]);
        assert!(r.events.iter().all(|e| e.written_dur == q()));
        assert_eq!(r.events[0].articulation, Some(Articulation::Staccato));
        assert_eq!(r.events[2].articulation, Some(Articulation::Tenuto));
        assert_eq!(r.total, dot(h()));
    }

    #[test]
    fn voice_instrument_and_hints_follow_scope_rules() {
        let own_voice = Music::Note(Note {
            pitch: C4.into(),
            dur: q(),
            attrs: NoteAttrs {
                voice_id: Some(VoiceId("solo".into())),
                hints: vec![BackendHint::Midi(MidiHint::Channel(3))],
                ..Default::default()
            },
        });
        let m = seq![n(C4, q()), own_voice]
            .modify(Control::Hint(BackendHint::Midi(MidiHint::ProgramChange(21))))
            .modify(Control::Voice(VoiceId("left".into())))
            .modify(Control::Instrument(InstrumentId("bandoneon".into())))
            .modify(Control::Hint(BackendHint::Lilypond(LilypondHint::StemUp)));
        let r = resolve(&m).unwrap();
        assert_eq!(r.events[0].voice, Some(VoiceId("left".into())));
        assert_eq!(r.events[1].voice, Some(VoiceId("solo".into())));
        assert!(r.events.iter().all(|e| e.instrument == Some(InstrumentId("bandoneon".into()))));
        assert_eq!(
            r.events[1].hints,
            vec![
                BackendHint::Midi(MidiHint::Channel(3)),
                BackendHint::Lilypond(LilypondHint::StemUp),
                BackendHint::Midi(MidiHint::ProgramChange(21)),
            ]
        );
    }

    // --- errors: one test per variant ---

    #[test]
    fn error_degree_zero() {
        let m = in_f_minor(n(Degree::new(0), q()));
        assert_eq!(resolve(&m), Err(ResolveError::DegreeZero));
    }

    #[test]
    fn error_degree_without_key() {
        assert_eq!(resolve(&n(d!(1), q())), Err(ResolveError::DegreeWithoutKey));
    }

    #[test]
    fn error_custom_mode_without_scale() {
        let m = n(d!(1), q()).modify(Control::Key(Key::new(pc!(C), Mode::Custom(4))));
        assert_eq!(resolve(&m), Err(ResolveError::CustomModeWithoutScale { mode: Mode::Custom(4) }));
    }

    #[test]
    fn error_empty_scale() {
        let m = n(d!(1), q()).modify(Control::Scale(Scale::custom(vec![]))).modify(Control::Key(Key::major(pc!(C))));
        assert_eq!(resolve(&m), Err(ResolveError::EmptyScale));
    }

    #[test]
    fn error_pitch_out_of_range_instead_of_overflow() {
        assert!(matches!(resolve(&n(C4, q()).transpose(1500)), Err(ResolveError::PitchOutOfRange { .. })));
        assert!(matches!(resolve(&n(C4, q()).transpose(1501)), Err(ResolveError::PitchOutOfRange { .. })));
        assert!(matches!(resolve(&n(C4, q()).transpose(i32::MAX)), Err(ResolveError::PitchOutOfRange { .. })));
        assert!(matches!(resolve(&n(C4, q()).transpose(i32::MAX).transpose(1)), Err(ResolveError::PitchOutOfRange { .. })));
        let high = Degree { number: 1, alter: 0, octave_shift: 127 };
        let m = n(high, q()).modify(Control::Key(Key::major(pc!(C))));
        assert!(matches!(resolve(&m), Err(ResolveError::PitchOutOfRange { .. })));
        // The most extreme interval size is still representable: an 18-octave major second down.
        let far = seq![n(C4, q()), n(iv(i8::MIN, IntervalQuality::Major), q())];
        assert_eq!(midis(&far), vec![60, -158]);
        // Large but representable transpositions still work.
        assert_eq!(midis(&n(C4, q()).transpose(120)), vec![180]);
    }

    #[test]
    fn error_unanchored_interval() {
        let m = n(iv(3, IntervalQuality::Major), q());
        assert_eq!(resolve(&m), Err(ResolveError::UnanchoredInterval));
        let in_par = par![n(iv(3, IntervalQuality::Major), q()), n(C4, q())];
        assert_eq!(resolve(&in_par), Err(ResolveError::UnanchoredInterval));
    }

    #[test]
    fn error_zero_interval() {
        let m = seq![n(C4, q()), n(iv(0, IntervalQuality::Perfect), q())];
        assert_eq!(resolve(&m), Err(ResolveError::ZeroInterval));
    }

    #[test]
    fn error_invalid_interval_quality() {
        let m = seq![n(C4, q()), n(iv(5, IntervalQuality::Major), q())];
        assert_eq!(
            resolve(&m),
            Err(ResolveError::InvalidIntervalQuality { generic: 5, quality: IntervalQuality::Major })
        );
        let m = seq![n(C4, q()), n(iv(-3, IntervalQuality::Perfect), q())];
        assert_eq!(
            resolve(&m),
            Err(ResolveError::InvalidIntervalQuality { generic: -3, quality: IntervalQuality::Perfect })
        );
    }

    #[test]
    fn error_tie_mismatch() {
        let tied = Music::Note(Note { pitch: C4.into(), dur: q(), attrs: NoteAttrs { tie_to_next: true, ..Default::default() } });
        assert_eq!(resolve(&seq![tied.clone(), n(D4, q())]), Err(ResolveError::TieMismatch { at: q() }));
        assert_eq!(resolve(&seq![n(E4, h()), tied, r(q())]), Err(ResolveError::TieMismatch { at: b(3, 1) }));
    }

    #[test]
    fn error_unspellable_pitch() {
        let m = n(Degree { number: 1, alter: 3, octave_shift: 0 }, q()).modify(Control::Key(Key::major(pc!(C))));
        assert_eq!(resolve(&m), Err(ResolveError::UnspellablePitch { semitone: 63, letter: Letter::C }));
    }

    #[test]
    fn errors_display_as_sentences() {
        let text = ResolveError::TieMismatch { at: b(3, 2) }.to_string();
        assert!(text.contains("3/2"), "{text}");
    }
    /// One case per dynamics level: an accent adds 15 on top of whatever the
    /// dynamics say, and the articulation survives into the event for the
    /// notation backends.
    #[test]
    fn accent_shifts_the_dynamics_derived_velocity() {
        let table = [
            (Dynamics::Ppp, 16),
            (Dynamics::Pp, 33),
            (Dynamics::P, 49),
            (Dynamics::Mp, 64),
            (Dynamics::Mf, 80),
            (Dynamics::F, 96),
            (Dynamics::Ff, 112),
            (Dynamics::Sfz, 120),
        ];
        for (level, plain) in table {
            let bare = n(C4, q()).modify(Control::Dynamics(level));
            assert_eq!(resolve(&bare).unwrap().events[0].velocity, plain, "{level:?} plain");
            for (art, offset) in [
                (Articulation::Accent, 15i16),
                (Articulation::Marcato, 30),
                (Articulation::Ghost, -25),
            ] {
                let m = n(C4, q())
                    .modify(Control::Articulation(art))
                    .modify(Control::Dynamics(level));
                let ev = &resolve(&m).unwrap().events[0];
                let want = (i16::from(plain) + offset).clamp(1, 127) as u8;
                assert_eq!(ev.velocity, want, "{level:?} with {art:?}");
                assert_eq!(ev.articulation, Some(art), "articulation must survive to the event");
            }
        }
    }

    /// The two ends. Loud clamps at 127 rather than wrapping; quiet clamps at 1
    /// rather than reaching 0, because a velocity-0 note-on is a note-off on
    /// most synthesizers and would delete the note instead of softening it.
    #[test]
    fn the_offsets_clamp_at_both_ends() {
        let loud = n(C4, q())
            .modify(Control::Articulation(Articulation::Marcato))
            .modify(Control::Dynamics(Dynamics::Fff));
        assert_eq!(resolve(&loud).unwrap().events[0].velocity, 127, "fff + marcato saturates");

        let quiet = n(C4, q())
            .modify(Control::Articulation(Articulation::Ghost))
            .modify(Control::Dynamics(Dynamics::Ppp));
        let v = resolve(&quiet).unwrap().events[0].velocity;
        assert_eq!(v, 1, "ppp + ghost floors at 1");
        assert_ne!(v, 0, "velocity 0 is a note-off, never a soft note");
    }

    /// Everything that is not a stress articulation leaves velocity alone,
    /// including tenuto, which emphasises length rather than attack.
    #[test]
    fn only_the_stress_articulations_move_velocity() {
        let plain = resolve(&n(C4, q())).unwrap().events[0].velocity;
        for art in [
            Articulation::Tenuto,
            Articulation::Staccato,
            Articulation::Legato,
            Articulation::Fermata,
            Articulation::Pizzicato,
        ] {
            let m = n(C4, q()).modify(Control::Articulation(art));
            assert_eq!(resolve(&m).unwrap().events[0].velocity, plain, "{art:?}");
        }
    }

    /// An explicit velocity is the caller being exact, so the offsets stay out
    /// of it; and the note's own articulation still shadows the one in scope.
    #[test]
    fn an_explicit_velocity_wins_over_the_offset() {
        let note = Music::Note(Note {
            pitch: C4.into(),
            dur: q(),
            attrs: NoteAttrs {
                velocity: Some(40),
                articulation: Some(Articulation::Marcato),
                ..Default::default()
            },
        });
        let m = note.modify(Control::Dynamics(Dynamics::Fff));
        assert_eq!(resolve(&m).unwrap().events[0].velocity, 40);

        // The note's own articulation shadows the enclosing one, velocity included.
        let shadowed = Music::Note(Note {
            pitch: C4.into(),
            dur: q(),
            attrs: NoteAttrs { articulation: Some(Articulation::Ghost), ..Default::default() },
        })
        .modify(Control::Articulation(Articulation::Marcato));
        assert_eq!(resolve(&shadowed).unwrap().events[0].velocity, 80 - 25);
    }

    /// Ghost prints by name, since it has no shorthand.
    #[test]
    fn ghost_prints_as_a_named_articulation() {
        assert_eq!(Articulation::Ghost.to_string(), "ghost");
        let note = Music::Note(Note {
            pitch: C4.into(),
            dur: q(),
            attrs: NoteAttrs { articulation: Some(Articulation::Ghost), ..Default::default() },
        });
        assert_eq!(format!("{note}"), "C4:q-ghost");
    }

}
