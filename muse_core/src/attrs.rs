//! Shared attribute types: ID newtypes, [`Articulation`], and [`NoteAttrs`].
//!
//! These are leaf types imported by both `music` and `control` to avoid
//! circular module dependencies.

use serde::{Deserialize, Serialize};

use crate::backends::BackendHint;

// === ID newtypes ===

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct VoiceId(pub String);

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct InstrumentId(pub String);

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct BackendId(pub String);

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub struct ScaleId(pub String);

// === Articulation ===

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Articulation {
    Staccato,
    Staccatissimo,
    Tenuto,
    Accent,
    Marcato,
    Legato,
    Slur,
    Fermata,
    Pizzicato,
    Arco,
    Trill,
    Mordent,
    Turn,
    HarmonicNatural,
    HarmonicArtificial,
}

// === Note attributes ===

#[derive(Clone, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub struct NoteAttrs {
    pub velocity: Option<u8>,
    pub articulation: Option<Articulation>,
    pub tie_to_next: bool,
    pub voice_id: Option<VoiceId>,
    pub hints: Vec<BackendHint>,
}
