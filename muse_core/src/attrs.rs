//! Shared attribute types: ID newtypes, [`Articulation`], and [`NoteAttrs`].
//!
//! These are leaf types imported by both `music` and `control` to avoid
//! circular module dependencies.

use serde::{Deserialize, Serialize};

use crate::backends::BackendHint;

// === ID newtypes ===

/// Identifies a voice or staff within a multi-voice piece.
///
/// Used in [`NoteAttrs::voice_id`] and [`Control::Voice`][crate::control::Control::Voice]
/// to assign notes to named parts (e.g. `VoiceId("violin_1".into())`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct VoiceId(pub String);

/// Identifies an instrument patch for a section of music.
///
/// Passed to [`Control::Instrument`][crate::control::Control::Instrument] to
/// change the active instrument for a subtree.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct InstrumentId(pub String);

/// Identifies a rendering backend, used as a key in [`RenderCache`][crate::phrase::RenderCache].
///
/// Example: `BackendId("lilypond".into())`, `BackendId("fluidsynth".into())`.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct BackendId(pub String);

/// Identifies a named scale definition stored outside the IR.
///
/// Used in `Mode::Custom` to reference a user-defined scale by name.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct ScaleId(pub String);

// === Articulation ===

/// A performance articulation applied to a single note.
///
/// Stored in [`NoteAttrs::articulation`]. Backends interpret each variant
/// according to their own conventions (e.g. `Staccato` shortens the note
/// duration in MIDI, adds a dot glyph in LilyPond, triggers a staccato
/// keyswitch in audio libraries).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Articulation {
    /// Short, detached — note played at roughly half its written duration.
    Staccato,
    /// Very short, extremely detached — shorter than staccato.
    Staccatissimo,
    /// Held for full duration with slight emphasis.
    Tenuto,
    /// Sharply stressed at the note's attack.
    Accent,
    /// Very strongly stressed, like a heavy accent.
    Marcato,
    /// Smooth connection to adjacent notes (slur within a phrase).
    Legato,
    /// Tie or slur connecting this note to the next.
    Slur,
    /// Hold the note beyond its written duration at the performer's discretion.
    Fermata,
    /// Plucked string technique (strings/guitar).
    Pizzicato,
    /// Return to bowed technique after pizzicato (strings).
    Arco,
    /// Rapid alternation between the written pitch and the pitch above.
    Trill,
    /// Single-note ornament: quick lower auxiliary note before the main pitch.
    Mordent,
    /// Four-note ornament: upper, main, lower, main.
    Turn,
    /// Natural harmonic (lightly touch the string at a node).
    HarmonicNatural,
    /// Artificial harmonic (stop + touch on strings).
    HarmonicArtificial,
}

// === Note attributes ===

/// Optional per-note performance attributes and backend metadata.
///
/// All fields are optional; `Default::default()` produces an attrs struct with
/// no velocity, no articulation, not tied, no voice assignment, and no hints.
/// Set only the fields you need:
///
/// ```
/// use muse_core::attrs::{NoteAttrs, Articulation};
/// let attrs = NoteAttrs {
///     velocity: Some(80),
///     articulation: Some(Articulation::Staccato),
///     ..Default::default()
/// };
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub struct NoteAttrs {
    /// MIDI velocity, `0..=127`. `None` means use the context default (typically 64).
    pub velocity: Option<u8>,
    /// Performance articulation applied to this note.
    pub articulation: Option<Articulation>,
    /// If `true`, this note is tied to the immediately following note of the same pitch.
    /// The two notes sound as a single sustained event.
    pub tie_to_next: bool,
    /// Voice/staff assignment. `None` means the note inherits the enclosing
    /// `Control::Voice` context, or belongs to a default voice.
    pub voice_id: Option<VoiceId>,
    /// Backend-specific hints attached to this note. A backend reads only
    /// the variants it understands and ignores the rest.
    pub hints: Vec<BackendHint>,
}
