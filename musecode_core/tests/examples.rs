//! One pinning test per example.
//!
//! Each example is pulled in as a module through its source path, so the test
//! calls the same `build()` the binary calls: there is one definition of each
//! piece, not a copy living in a test. The binaries themselves are not run
//! here; that is a separate concern and a later issue.
//!
//! What gets pinned, per example:
//!
//! 1. the notation, byte for byte, against `tests/snapshots/<name>.txt`
//! 2. the sha256 of the rendered `.mid` bytes
//! 3. a few `analysis` facts, so a failing hash arrives with a hint about what
//!    moved rather than only a pair of hex strings
//!
//! Rewrite the snapshots after a deliberate change with
//! `UPDATE_SNAPSHOTS=1 cargo test --test examples`, then read the diff before
//! committing it. The `.mid` hashes are updated by hand on purpose: a renderer
//! change should cost a moment's thought.

use std::fs;
use std::path::PathBuf;

use musecode_core::prelude::*;
use sha2::{Digest, Sha256};

#[allow(dead_code, unused_imports)]
#[path = "../examples/tango.rs"]
mod tango;

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots").join(format!("{name}.txt"))
}

/// Compare the notation against the committed snapshot, or rewrite it when
/// `UPDATE_SNAPSHOTS` is set.
fn check_notation(name: &str, music: &Music) {
    let actual = format!("{music}");
    let path = snapshot_path(name);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).expect("snapshot dir");
        fs::write(&path, &actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("example `{name}`: cannot read {}: {e}. Run with UPDATE_SNAPSHOTS=1 to create it.", path.display())
    });
    assert_eq!(actual, expected, "example `{name}`: notation changed");
}

fn midi_sha256(name: &str, music: &Music) -> String {
    let bytes = render_midi(music, &MidiOptions::default())
        .unwrap_or_else(|e| panic!("example `{name}`: render failed: {e}"));
    format!("{:x}", Sha256::digest(&bytes))
}

/// The shape of a pinned example: what its resolved events look like, so a
/// hash mismatch says which of these moved.
struct Facts {
    events: usize,
    lowest: &'static str,
    highest: &'static str,
    total_beats: (i32, i32),
}

fn check(name: &str, music: &Music, sha: &str, facts: Facts) {
    check_notation(name, music);

    let resolved = resolve(music).unwrap_or_else(|e| panic!("example `{name}`: {e}"));
    assert_eq!(resolved.events.len(), facts.events, "example `{name}`: event count");
    let (lo, hi) = pitch_range(&resolved.events).expect("a pinned example has notes");
    assert_eq!(lo.to_string(), facts.lowest, "example `{name}`: lowest pitch");
    assert_eq!(hi.to_string(), facts.highest, "example `{name}`: highest pitch");
    assert_eq!(
        resolved.total,
        b(facts.total_beats.0, facts.total_beats.1),
        "example `{name}`: total duration"
    );

    assert_eq!(midi_sha256(name, music), sha, "example `{name}`: rendered .mid changed");
}

#[test]
fn tango_is_pinned() {
    check(
        "tango",
        &tango::build(),
        "ad42811e3174021c7eb5c94e84e7ee2df08dc5712b3a41a7459097a7abf36624",
        Facts { events: 22, lowest: "C3", highest: "C5", total_beats: (16, 1) },
    );
}
