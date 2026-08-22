//! Structural analysis over resolved events.
//!
//! The minimum vocabulary for "analyse this fragment": range, pitch-class
//! content, melodic interval counts, what sounds together at each onset, and
//! triad labels for those vertical slices. Everything takes `&[Event]` from
//! the [`resolve`][crate::resolve] pass rather than `&Music`, so the same
//! functions work on anything a backend produces or consumes. The one
//! exception is [`summary`], a convenience that resolves a tree itself.
//!
//! Layer 5. Imports `resolve`, `pitch` and `time` (plus `music` for the
//! `summary` argument type); never a backend. Roman-numeral and functional
//! analysis are deliberately absent; they need key detection and
//! voice-leading rules that belong to a later milestone.

use std::collections::BTreeMap;
use std::fmt;

use crate::music::Music;
use crate::pitch::{ChromaticPitch, PitchClass};
use crate::resolve::{resolve, Event, ResolveError};
use crate::time::Beats;

/// Lowest and highest sounding pitch by MIDI number, or `None` for no events.
/// Spelling is that of the event that reaches each extreme first.
pub fn pitch_range(events: &[Event]) -> Option<(ChromaticPitch, ChromaticPitch)> {
    let mut it = events.iter().map(|e| e.pitch);
    let first = it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| {
        (if p.midi() < lo.midi() { p } else { lo }, if p.midi() > hi.midi() { p } else { hi })
    }))
}

/// The pitch classes present in a passage: a 12-bit set (bit 0 is C) and the
/// distinct spelled classes sorted by semitone above C, then by spelling.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PitchClassSet {
    /// Bit `n` is set when some event sounds pitch class `n` semitones above C.
    pub bits: u16,
    /// Every distinct spelled pitch class, sorted. `Cb` and `B` both set bit 11
    /// but appear here as two entries.
    pub classes: Vec<PitchClass>,
}

impl PitchClassSet {
    /// Number of distinct semitone classes, `0..=12`.
    pub fn len(&self) -> usize {
        self.bits.count_ones() as usize
    }

    /// `true` when no pitch class is present.
    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Whether the semitone class `n` (mod 12) is present.
    pub fn contains(&self, semitones: i32) -> bool {
        self.bits & (1 << semitones.rem_euclid(12)) != 0
    }
}

impl fmt::Display for PitchClassSet {
    /// Space-separated spelled classes: `C F G Ab Bb Cb`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.classes.iter().map(|c| c.to_string()).collect();
        f.write_str(&parts.join(" "))
    }
}

/// The pitch-class content of a passage.
pub fn pitch_class_set(events: &[Event]) -> PitchClassSet {
    let mut bits = 0u16;
    let mut classes: Vec<PitchClass> = Vec::new();
    for e in events {
        let class = e.pitch.class;
        bits |= 1 << class.semitones();
        if !classes.contains(&class) {
            classes.push(class);
        }
    }
    classes.sort_by_key(|c| (c.semitones(), c.letter.index(), c.accidental.offset()));
    PitchClassSet { bits, classes }
}

/// Counts of melodic intervals, in signed semitones, between consecutive
/// events of the same voice. Events that share an onset within a voice (a
/// chord written into one voice) are not counted as a melodic step.
pub fn interval_histogram(events: &[Event]) -> BTreeMap<i32, usize> {
    let mut by_voice: BTreeMap<Option<&str>, Vec<&Event>> = BTreeMap::new();
    for e in events {
        by_voice.entry(e.voice.as_ref().map(|v| v.0.as_str())).or_default().push(e);
    }
    let mut histogram = BTreeMap::new();
    for line in by_voice.values_mut() {
        line.sort_by_key(|e| e.onset);
        for pair in line.windows(2) {
            if pair[0].onset == pair[1].onset {
                continue;
            }
            *histogram.entry(pair[1].pitch.midi() - pair[0].pitch.midi()).or_insert(0) += 1;
        }
    }
    histogram
}

/// For each distinct onset, every pitch sounding at that instant (an event
/// sounds on `onset..onset + dur`, or exactly at `onset` if its duration is
/// zero), sorted low to high.
pub fn vertical_slices(events: &[Event]) -> Vec<(Beats, Vec<ChromaticPitch>)> {
    let mut onsets: Vec<Beats> = events.iter().map(|e| e.onset).collect();
    onsets.sort();
    onsets.dedup();
    onsets
        .into_iter()
        .map(|t| {
            let mut sounding: Vec<ChromaticPitch> = events
                .iter()
                .filter(|e| e.onset <= t && (t < e.onset + e.dur || (e.dur == Beats::from_integer(0) && e.onset == t)))
                .map(|e| e.pitch)
                .collect();
            sounding.sort_by_key(|p| p.midi());
            (t, sounding)
        })
        .collect()
}

/// The quality of a three-note chord.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TriadQuality {
    /// Root, major third, perfect fifth.
    Major,
    /// Root, minor third, perfect fifth.
    Minor,
    /// Root, minor third, diminished fifth.
    Diminished,
    /// Root, major third, augmented fifth.
    Augmented,
    /// Root, major second, perfect fifth.
    Sus2,
    /// Root, perfect fourth, perfect fifth.
    Sus4,
}

impl fmt::Display for TriadQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TriadQuality::Major => "major",
            TriadQuality::Minor => "minor",
            TriadQuality::Diminished => "diminished",
            TriadQuality::Augmented => "augmented",
            TriadQuality::Sus2 => "sus2",
            TriadQuality::Sus4 => "sus4",
        })
    }
}

/// A labelled triad: the root as spelled in the slice, and its quality.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Triad {
    /// The root, taking the spelling of the pitch that played it.
    pub root: PitchClass,
    /// The quality.
    pub quality: TriadQuality,
}

impl fmt::Display for Triad {
    /// `F minor`, `Bb sus4`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.root, self.quality)
    }
}

/// Label a vertical slice as a triad when its pitch classes form one, in
/// any inversion and with any doubling. Candidate roots are tried from the
/// lowest sounding pitch upward, so a `C D G` slice is `C sus2` and
/// `G C D` is `G sus4`. Returns `None` for anything that is not exactly
/// three pitch classes in one of the six shapes.
pub fn label_triad(pitches: &[ChromaticPitch]) -> Option<Triad> {
    let mut sorted: Vec<ChromaticPitch> = pitches.to_vec();
    sorted.sort_by_key(|p| p.midi());
    let mut classes: Vec<i32> = sorted.iter().map(|p| p.class.semitones()).collect();
    classes.sort();
    classes.dedup();
    if classes.len() != 3 {
        return None;
    }
    const SHAPES: [([i32; 3], TriadQuality); 6] = [
        ([0, 4, 7], TriadQuality::Major),
        ([0, 3, 7], TriadQuality::Minor),
        ([0, 3, 6], TriadQuality::Diminished),
        ([0, 4, 8], TriadQuality::Augmented),
        ([0, 2, 7], TriadQuality::Sus2),
        ([0, 5, 7], TriadQuality::Sus4),
    ];
    for candidate in &sorted {
        let root = candidate.class.semitones();
        let mut relative: Vec<i32> = classes.iter().map(|c| (c - root).rem_euclid(12)).collect();
        relative.sort();
        for (shape, quality) in SHAPES {
            if relative == shape {
                return Some(Triad { root: candidate.class, quality });
            }
        }
    }
    None
}

/// A one-string report: total duration, event count, range, pitch-class
/// set, melodic interval histogram, and a triad label for every vertical
/// slice that has one.
pub fn summary(music: &Music) -> Result<String, ResolveError> {
    let resolved = resolve(music)?;
    let events = &resolved.events;
    let mut out = String::new();
    out.push_str(&format!("duration: {} beats\n", resolved.total));
    out.push_str(&format!("events: {}\n", events.len()));
    match pitch_range(events) {
        Some((lo, hi)) => out.push_str(&format!("range: {lo} .. {hi}\n")),
        None => out.push_str("range: (no notes)\n"),
    }
    let pcs = pitch_class_set(events);
    out.push_str(&format!("pitch classes ({}): {pcs}\n", pcs.len()));
    let histogram = interval_histogram(events);
    let parts: Vec<String> = histogram.iter().map(|(k, v)| format!("{k:+}:{v}")).collect();
    out.push_str(&format!("melodic intervals (semitones:count): {}\n", if parts.is_empty() { "(none)".to_string() } else { parts.join(" ") }));
    let slices = vertical_slices(events);
    let labelled: Vec<String> = slices
        .iter()
        .filter_map(|(t, pitches)| label_triad(pitches).map(|tr| format!("{t}: {tr}")))
        .collect();
    out.push_str(&format!("slices: {} ({} with a triad)\n", slices.len(), labelled.len()));
    for line in labelled {
        out.push_str("  ");
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::{Accidental, Letter};
    use crate::prelude::*;

    fn events(m: &Music) -> Vec<Event> {
        resolve(m).unwrap().events
    }

    #[test]
    fn range_is_by_midi_and_keeps_spelling() {
        let m = seq![n(E4, q()), n(ChromaticPitch::new(Letter::C, Accidental::Flat, 5), q()), n(F3, q()), n(B4, q())];
        let (lo, hi) = pitch_range(&events(&m)).unwrap();
        assert_eq!(lo, F3);
        assert_eq!(hi, ChromaticPitch::new(Letter::C, Accidental::Flat, 5));
        assert_eq!(pitch_range(&[]), None);
    }

    #[test]
    fn pitch_class_set_keeps_distinct_spellings_of_one_semitone() {
        let cb4 = ChromaticPitch::new(Letter::C, Accidental::Flat, 4);
        let m = seq![n(B3, q()), n(cb4, q()), n(G4, q()), n(C5, q()), n(G3, q())];
        let set = pitch_class_set(&events(&m));
        assert_eq!(set.len(), 3);
        assert!(set.contains(11) && set.contains(7) && set.contains(0));
        assert!(!set.contains(2));
        assert_eq!(set.to_string(), "C G Cb B");
        assert_eq!(set.bits, 0b1000_1000_0001);
    }

    #[test]
    fn histogram_counts_steps_per_voice_and_skips_chords() {
        let left = seq![n(C3, q()), n(G3, q()), n(C3, q())].modify(Control::Voice(VoiceId("left".into())));
        let right = seq![n(E4, q()), chord([F4, A4], q()), n(G4, q())];
        let h = interval_histogram(&events(&(left | right)));
        // left: +7, -7. right: E4->F4 +1, E4->A4 is not counted (chord sibling), then F4->G4 +2 or A4->G4 -2
        // depending on the stable sort; the chord pair itself (same onset) is skipped.
        assert_eq!(h.get(&7), Some(&1));
        assert_eq!(h.get(&-7), Some(&1));
        assert_eq!(h.get(&0), None);
        assert_eq!(h.values().sum::<usize>(), 4);
    }

    #[test]
    fn vertical_slices_report_what_sounds_at_each_onset() {
        let m = par![n(C3, w()), seq![n(E4, h()), n(G4, h())], seq![r(q()), n(C5, q())]];
        let slices = vertical_slices(&events(&m));
        assert_eq!(slices.len(), 3);
        assert_eq!(slices[0], (b(0, 1), vec![C3, E4]));
        assert_eq!(slices[1], (q(), vec![C3, E4, C5]));
        assert_eq!(slices[2], (h(), vec![C3, G4]));
    }

    #[test]
    fn triads_in_every_quality_inversion_and_doubling() {
        let t = |p: &[ChromaticPitch]| label_triad(p).map(|t| t.to_string());
        assert_eq!(t(&[C4, E4, G4]), Some("C major".into()));
        assert_eq!(t(&[E4, G4, C5]), Some("C major".into()));
        assert_eq!(t(&[G3, C4, E4, G4, C5]), Some("C major".into()));
        assert_eq!(t(&[A3, C4, E4]), Some("A minor".into()));
        assert_eq!(t(&[B3, D4, F4]), Some("B diminished".into()));
        assert_eq!(t(&[C4, E4, GS4]), Some("C augmented".into()));
        assert_eq!(t(&[C4, D4, G4]), Some("C sus2".into()));
        assert_eq!(t(&[G3, C4, D4]), Some("G sus4".into()));
        assert_eq!(t(&[F3, AB4, C5]), Some("F minor".into()));
        assert_eq!(t(&[C4, E4]), None);
        assert_eq!(t(&[C4, D4, E4]), None);
        assert_eq!(t(&[C4, E4, G4, B4]), None);
        assert_eq!(t(&[]), None);
    }

    /// The README Piazzolla sketch, unchanged.
    fn readme_sketch() -> Music {
        let key = Key::minor(pc!(F));
        let tango = |root: ChromaticPitch| {
            seq![n(root, dot(e())), n(root, s()), n(root, e()), n(root, e()), n(root, q()), n(root, q())]
        };
        let comp_rest = r(q()) + r(dot(q()));
        let melody = seq![n(d!(5), q()), n(d!(b 5), e()), n(d!(4), e()), n(d!(3), h())];
        let bar1 = tango(G3) | comp_rest.clone() | melody.clone();
        let bar2 = tango(C3) | comp_rest.clone() | melody.diatonic_transpose(-1);
        let bar3 = tango(F3) | comp_rest | seq![n(d!(1), w())];
        seq![bar1, bar2, bar3.clone(), bar3]
            .modify(Control::Key(key))
            .modify(Control::TimeSignature(TimeSig::common()))
            .modify(Control::Tempo(Tempo::bpm(96)))
    }

    #[test]
    fn summary_of_the_readme_sketch() {
        let text = summary(&readme_sketch()).unwrap();
        assert!(text.starts_with("duration: 16 beats\n"), "{text}");
        assert!(text.contains("events: 34\n"), "{text}");
        assert!(text.contains("range: C3 .. C5\n"), "{text}");
        assert!(text.contains("pitch classes (7): C F G Ab Bbb Bb Cb\n"), "{text}");
        assert!(text.contains("melodic intervals"), "{text}");
        assert!(text.contains("slices: "), "{text}");
        let set = pitch_class_set(&events(&readme_sketch()));
        for semis in [5, 7, 8, 10, 0] {
            assert!(set.contains(semis), "F minor class {semis} missing");
        }
    }

    #[test]
    fn summary_surfaces_resolve_errors() {
        assert_eq!(summary(&n(d!(1), q())), Err(ResolveError::DegreeWithoutKey));
    }
}
