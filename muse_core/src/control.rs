//! Context modifiers: the [`Control`] enum applied via [`crate::music::Music::Modify`].
//!
//! A `Control` wraps a subtree and changes how that subtree is interpreted:
//! key, scale, tempo, transposition, dynamics, and backend-specific hints.
//! Controls nest; inner values shadow outer ones.

use serde::{Deserialize, Serialize};

use crate::attrs::{Articulation, InstrumentId, VoiceId};
use crate::backends::BackendHint;
use crate::theory::{Key, Scale};
use crate::time::{Dynamics, Tempo, TimeSig};

/// A context modifier applied to a [`Music`][crate::music::Music] subtree via
/// [`Music::Modify`][crate::music::Music::Modify].
///
/// Controls scope to their subtree: applying `Control::Key(k)` to a section
/// changes degree-pitch resolution for that section only. Wrapping the same
/// section in a second `Modify` with a different key overrides the outer one
/// for that inner region.
///
/// # Examples
/// ```
/// use muse_core::prelude::*;
/// let phrase = seq![n(d!(1), q()), n(d!(3), q()), n(d!(5), h())]
///     .modify(Control::Key(Key::minor(pc!(F))))
///     .modify(Control::Tempo(Tempo::bpm(96)))
///     .modify(Control::TimeSignature(TimeSig::common()));
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Control {
    /// Set the playback tempo for this subtree.
    Tempo(Tempo),
    /// Set the time signature for this subtree (affects notation layout).
    TimeSignature(TimeSig),
    /// Establish a key context used to resolve [`Degree`][crate::pitch::Degree] pitches
    /// and diatonic transpositions within this subtree.
    Key(Key),
    /// Override the scale derived from the active [`Key`], providing explicit
    /// interval content for degree resolution.
    Scale(Scale),
    /// Set the active instrument for this subtree. How this is interpreted
    /// depends on the backend (MIDI program, sample library patch, etc.).
    Instrument(InstrumentId),
    /// Transpose all pitches in this subtree by `n` chromatic semitones.
    /// Positive values transpose up, negative values transpose down.
    Transpose(i32),
    /// Transpose all scale-degree pitches in this subtree by `n` diatonic steps
    /// within the active [`Key`] context. A diatonic step moves to the next
    /// scale degree rather than moving by a fixed number of semitones.
    DiatonicTranspose(i32),
    /// Apply a dynamic level to this subtree (affects velocity in MIDI,
    /// dynamic markings in notation).
    Dynamics(Dynamics),
    /// Apply a default articulation to all notes in this subtree that do not
    /// have a per-note articulation set.
    Articulation(Articulation),
    /// Assign all notes in this subtree to the named voice/staff.
    Voice(VoiceId),
    /// Attach a backend-specific hint to every note in this subtree.
    Hint(BackendHint),
    /// Arbitrary key-value metadata, passed through to backends that
    /// understand it. Use this as an escape hatch for backend-specific
    /// features not covered by other variants.
    User(String, String),
}
