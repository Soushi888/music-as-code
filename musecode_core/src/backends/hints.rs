//! Backend hints: metadata that lives alongside the IR, not inside it.
//!
//! A backend that doesn't understand a hint ignores it. A backend that needs
//! information not present in hints synthesizes a reasonable default.
//! This keeps the core [`Music`][crate::music::Music] IR target-agnostic.

use serde::{Deserialize, Serialize};

/// A hint attached to a [`NoteAttrs`][crate::attrs::NoteAttrs] that carries
/// backend-specific rendering instructions.
///
/// Hints are additive: multiple hints of different backend types can coexist
/// on the same note. A backend only reads the variant it understands.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BackendHint {
    /// A hint for the LilyPond notation backend.
    Lilypond(LilypondHint),
    /// A hint for the MIDI file backend.
    Midi(MidiHint),
    /// A hint for sample-library audio backends.
    Audio(AudioHint),
}

/// LilyPond-specific rendering instructions.
///
/// These affect notation layout and output without changing the musical meaning
/// of the note. Raw `\markup` escapes provide an escape hatch for anything not
/// covered by the typed variants.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LilypondHint {
    /// Force the note's stem to point upward.
    StemUp,
    /// Force the note's stem to point downward.
    StemDown,
    /// Begin a manual beam group at this note.
    BeamStart,
    /// End a manual beam group at this note.
    BeamEnd,
    /// Attach a raw LilyPond `\markup` string to this note.
    Markup(String),
    /// Suppress the time-signature glyph at this note's position.
    OmitTimeSignature,
    /// Render this rest as invisible (useful for alignment in multi-voice scores).
    HiddenRest,
}

/// MIDI file backend hints.
///
/// Applied before the note-on event for the note they are attached to.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum MidiHint {
    /// Send a program-change message before this note. Value `0..=127`.
    ProgramChange(u8),
    /// Route this note to the specified MIDI channel. Value `0..=15`.
    Channel(u8),
    /// Send a control-change (CC) message before this note.
    ControlChange {
        /// MIDI CC number (0–127).
        controller: u8,
        /// CC value (0–127).
        value: u8,
    },
}

/// Sample-library audio backend hints.
///
/// These map to standard conventions used by most orchestral sample libraries
/// (e.g. key-switches, articulation names, mic routing).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AudioHint {
    /// Trigger an articulation keyswitch note before this note. Value is a MIDI
    /// note number (0–127) used by the sample library to switch patches.
    KeySwitch(u8),
    /// Named articulation string for libraries that use text-based patch selection
    /// (e.g. `"spiccato"`, `"col_legno"`, `"sul_pont"`).
    Articulation(String),
    /// Preferred microphone position for this note's sample playback.
    MicPosition(MicPos),
    /// Advance the round-robin counter by this many steps, for libraries that
    /// cycle through multiple sample variants per note.
    RoundRobin(u8),
}

/// Microphone position for sample-library audio routing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum MicPos {
    /// Close-microphone capture (dry, detailed).
    Close,
    /// Mid-field microphone (balanced).
    Mid,
    /// Far/ambient microphone (reverberant room sound).
    Far,
    /// Blend of multiple positions as configured in the sample library.
    Mix,
}
