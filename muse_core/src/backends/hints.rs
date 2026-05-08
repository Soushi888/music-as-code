//! Backend hints: metadata that lives alongside the IR, not inside it.
//!
//! A backend that doesn't understand a hint ignores it. A backend that needs
//! information not present in hints synthesizes a reasonable default.
//! This keeps the core [`Music`][crate::music::Music] IR target-agnostic.

use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BackendHint {
    Lilypond(LilypondHint),
    Midi(MidiHint),
    Audio(AudioHint),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LilypondHint {
    StemUp,
    StemDown,
    BeamStart,
    BeamEnd,
    Markup(String),
    OmitTimeSignature,
    HiddenRest,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum MidiHint {
    ProgramChange(u8),
    Channel(u8),
    ControlChange { controller: u8, value: u8 },
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum AudioHint {
    KeySwitch(u8),
    Articulation(String),
    MicPosition(MicPos),
    RoundRobin(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum MicPos {
    Close,
    Mid,
    Far,
    Mix,
}
