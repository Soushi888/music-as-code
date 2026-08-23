//! `musecode_core` — the kernel of the `muse` musical DSL.
//!
//! A small, orthogonal intermediate representation for music: five core ADT
//! constructors, polymorphic pitch, rational time, and content-addressed
//! fragments. Everything ergonomic is built on top of these primitives.
//!
//! Start with [`prelude`] to bring the full API into scope.

pub mod analysis;
pub mod attrs;
pub mod backends;
pub mod combinators;
pub mod control;
pub mod display;
pub mod euclid;
pub mod music;
pub mod phrase;
pub mod pitch;
pub mod prelude;
pub mod resolve;
pub mod rhythm;
pub mod theory;
pub mod time;
