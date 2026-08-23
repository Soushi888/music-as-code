//! Rendering backends and the hints they read.
//!
//! [`hints`] is a leaf module: the metadata that lives alongside the IR.
//! [`midi`] is the first concrete backend, a flat map from
//! [`Resolved`][crate::resolve::Resolved] events to a Standard MIDI File.

pub mod hints;
pub mod midi;

pub use hints::{AudioHint, BackendHint, LilypondHint, MicPos, MidiHint};
pub use midi::{render_midi, render_resolved, write_midi, MidiError, MidiOptions};
