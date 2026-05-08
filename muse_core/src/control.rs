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

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Control {
    Tempo(Tempo),
    TimeSignature(TimeSig),
    Key(Key),
    Scale(Scale),
    Instrument(InstrumentId),
    Transpose(i32),
    DiatonicTranspose(i32),
    Dynamics(Dynamics),
    Articulation(Articulation),
    Voice(VoiceId),
    Hint(BackendHint),
    User(String, String),
}
