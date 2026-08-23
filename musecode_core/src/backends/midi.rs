//! MIDI file backend: a flat map from the resolved event list to a Standard
//! MIDI File (format 1).
//!
//! The backend never interprets music. It consumes [`Resolved`] (see
//! [`crate::resolve`]) and writes bytes:
//!
//! - track 0 is the conductor track: tempo and time-signature meta events from
//!   `tempo_map` and `time_sigs`, with `120 bpm` and `4/4` supplied at beat 0
//!   when the piece states neither;
//! - one track per distinct voice, in order of first appearance, the `None`
//!   voice being its own track; each track takes the next free channel
//!   (channel 9, the GM percussion channel, is skipped), and a
//!   [`MidiHint::Channel`] reroutes the note it is attached to (wrap a voice
//!   in `Control::Hint` to reroute all of it);
//! - every note wants a program: a [`MidiHint::ProgramChange`] on the note,
//!   else its [`InstrumentId`] through [`program_for`], else
//!   [`MidiOptions::default_program`]; a program change is written whenever
//!   that differs from what the note's channel last received, so the first
//!   note of every track opens with one. [`MidiHint::ControlChange`] is
//!   written just before the note it is attached to;
//! - ticks are `beats * ppq`, exact for every duration helper at the default
//!   480 PPQ (`triplet(ts()) * 480 == 40`); an arbitrary `b(n, d)` that is not
//!   integral is rounded to the nearest tick.
//!
//! Two deliberate simplifications for milestone 1: [`Tempo::Ramp`] contributes
//! its `from_bpm` only, and overlapping same-pitch notes on one channel are
//! written as they come.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::Path;

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use num_rational::Rational32;
use num_traits::CheckedMul;

use crate::attrs::{BackendId, InstrumentId, VoiceId};
use crate::backends::hints::{BackendHint, MidiHint};
use crate::music::Music;
use crate::resolve::{resolve, Event, ResolveError, Resolved};
use crate::time::{Beats, Tempo, TimeSig};

/// Identifier of this backend, the key half of a [`RenderCache`][crate::phrase::RenderCache] entry.
pub const BACKEND_ID: &str = "midi";

/// [`BACKEND_ID`] as a [`BackendId`].
pub fn backend_id() -> BackendId {
    BackendId(BACKEND_ID.to_string())
}

/// Rendering options. [`Default`] is 480 PPQ, 120 bpm, acoustic grand piano.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiOptions {
    /// Pulses (ticks) per quarter note. `1..=32767`; 480 divides every duration helper.
    pub ppq: u16,
    /// Tempo written at beat 0 when the piece has no `Control::Tempo` there.
    pub default_bpm: u32,
    /// General MIDI program (`0..=127`) for tracks whose first event names no instrument.
    pub default_program: u8,
}

impl Default for MidiOptions {
    fn default() -> Self {
        Self { ppq: 480, default_bpm: 120, default_program: 0 }
    }
}

/// Everything that can go wrong between a [`Music`] tree and MIDI bytes.
#[derive(Debug)]
pub enum MidiError {
    /// The tree could not be resolved; see [`ResolveError`].
    Resolve(ResolveError),
    /// Writing the file failed.
    Io(io::Error),
    /// [`MidiOptions::ppq`] is zero or above 32767.
    InvalidPpq(u16),
    /// A `Control::Tempo` of 0 bpm at this beat.
    ZeroTempo {
        /// Onset of the offending tempo change.
        at: Beats,
    },
    /// A time signature whose denominator is not a power of two; MIDI cannot encode it.
    BadTimeSignature {
        /// Onset of the offending time signature.
        at: Beats,
        /// Its numerator.
        numerator: u8,
        /// Its denominator.
        denominator: u8,
    },
    /// A resolved pitch outside MIDI's `0..=127`.
    PitchOutOfMidiRange {
        /// Onset of the note.
        at: Beats,
        /// Its MIDI number.
        midi: i32,
    },
    /// A tick value that does not fit the file format (negative, or beyond 2^28).
    TickOverflow {
        /// The beat that could not be converted.
        at: Beats,
    },
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiError::Resolve(e) => write!(f, "cannot resolve music: {e}"),
            MidiError::Io(e) => write!(f, "cannot write MIDI: {e}"),
            MidiError::InvalidPpq(p) => write!(f, "ppq must be 1..=32767, got {p}"),
            MidiError::ZeroTempo { at } => write!(f, "tempo of 0 bpm at beat {at}"),
            MidiError::BadTimeSignature { at, numerator, denominator } => write!(
                f,
                "time signature {numerator}/{denominator} at beat {at}: MIDI needs a power-of-two denominator"
            ),
            MidiError::PitchOutOfMidiRange { at, midi } => {
                write!(f, "pitch MIDI {midi} at beat {at} is outside 0..=127")
            }
            MidiError::TickOverflow { at } => write!(f, "beat {at} does not fit in MIDI ticks"),
        }
    }
}

impl std::error::Error for MidiError {}

impl From<ResolveError> for MidiError {
    fn from(e: ResolveError) -> Self {
        MidiError::Resolve(e)
    }
}

impl From<io::Error> for MidiError {
    fn from(e: io::Error) -> Self {
        MidiError::Io(e)
    }
}

/// Resolve `music` and render it to Standard MIDI File bytes.
///
/// # Examples
/// ```
/// use musecode_core::prelude::*;
/// let bytes = render_midi(&n(C4, q()), &MidiOptions::default()).unwrap();
/// assert_eq!(&bytes[0..4], b"MThd");
/// ```
pub fn render_midi(music: &Music, opts: &MidiOptions) -> Result<Vec<u8>, MidiError> {
    let resolved = resolve(music)?;
    render_resolved(&resolved, opts)
}

/// Resolve `music`, render it, and write the bytes to `path`.
pub fn write_midi(
    music: &Music,
    path: impl AsRef<Path>,
    opts: &MidiOptions,
) -> Result<(), MidiError> {
    let bytes = render_midi(music, opts)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Map an instrument name to a General MIDI program number.
///
/// Accepts a bare number (`"21"`), a `gm:` prefix (`"gm:21"`), or a common
/// instrument name in any case with spaces, hyphens or underscores
/// (`"acoustic grand"`, `"bandoneon"`, `"Electric_Bass"`). Returns `None` for
/// anything it does not know, so the caller can fall back to
/// [`MidiOptions::default_program`].
pub fn program_for(instrument: &InstrumentId) -> Option<u8> {
    let raw = instrument.0.trim().to_ascii_lowercase();
    let name = raw.trim_start_matches("gm:").trim().replace(['-', '_'], " ");
    if let Ok(n) = name.parse::<u8>() {
        return (n <= 127).then_some(n);
    }
    let program = match name.as_str() {
        "piano" | "acoustic grand" | "acoustic grand piano" | "grand piano" => 0,
        "bright piano" | "bright acoustic piano" => 1,
        "electric piano" | "rhodes" => 4,
        "harpsichord" => 6,
        "celesta" => 8,
        "glockenspiel" => 9,
        "vibraphone" | "vibes" => 11,
        "marimba" => 12,
        "organ" | "drawbar organ" => 16,
        "church organ" => 19,
        "accordion" | "bandoneon" => 21,
        "harmonica" => 22,
        "guitar" | "nylon guitar" | "acoustic guitar" => 24,
        "steel guitar" => 25,
        "electric guitar" | "clean guitar" => 27,
        "bass" | "acoustic bass" | "upright bass" => 32,
        "electric bass" => 33,
        "violin" => 40,
        "viola" => 41,
        "cello" | "violoncello" => 42,
        "contrabass" | "double bass" => 43,
        "harp" => 46,
        "strings" | "string ensemble" => 48,
        "choir" | "choir aahs" => 52,
        "trumpet" => 56,
        "trombone" => 57,
        "tuba" => 58,
        "horn" | "french horn" => 60,
        "sax" | "alto sax" | "alto saxophone" => 65,
        "tenor sax" | "tenor saxophone" => 66,
        "oboe" => 68,
        "bassoon" => 70,
        "clarinet" => 71,
        "piccolo" => 72,
        "flute" => 73,
        "recorder" => 74,
        _ => return None,
    };
    Some(program)
}

/// Channels handed to voice tracks in order; 9 (GM percussion) is skipped.
const CHANNELS: [u8; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15];

/// Slowest tempo MIDI can encode: `u24::MAX` microseconds per quarter.
const SLOWEST_US_PER_QUARTER: u32 = 0x00FF_FFFF;

/// Sort key within one tick: control messages, then note-offs, then note-ons,
/// so a repeated pitch is released before it is struck again.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Order {
    Control = 0,
    Off = 1,
    On = 2,
}

struct Timed {
    tick: u32,
    order: Order,
    kind: TrackEventKind<'static>,
}

fn ticks(beats: Beats, ppq: u16) -> Option<u32> {
    let scaled = beats.checked_mul(&Rational32::from_integer(i32::from(ppq)))?;
    let rounded = scaled.round().to_integer();
    u32::try_from(rounded).ok()
}

fn tick_at(beats: Beats, ppq: u16) -> Result<u32, MidiError> {
    ticks(beats, ppq).ok_or(MidiError::TickOverflow { at: beats })
}

fn bpm_of(tempo: &Tempo) -> u32 {
    match tempo {
        Tempo::Fixed(bpm) => *bpm,
        Tempo::Ramp { from_bpm, .. } => *from_bpm,
    }
}

/// First `MidiHint` of the kind selected by `pick`, in hint order (the note's
/// own hints come before enclosing `Control::Hint`s, so the most specific wins).
fn midi_hint<T>(hints: &[BackendHint], pick: impl Fn(&MidiHint) -> Option<T>) -> Option<T> {
    hints.iter().find_map(|h| match h {
        BackendHint::Midi(m) => pick(m),
        _ => None,
    })
}

fn conductor_track(resolved: &Resolved, opts: &MidiOptions) -> Result<Vec<Timed>, MidiError> {
    let mut out = Vec::new();
    let zero = Rational32::from_integer(0);

    let has_tempo_at_zero = resolved.tempo_map.iter().any(|(at, _)| *at == zero);
    let mut tempos: Vec<(Beats, u32)> = Vec::new();
    if !has_tempo_at_zero {
        tempos.push((zero, opts.default_bpm));
    }
    tempos.extend(resolved.tempo_map.iter().map(|(at, t)| (*at, bpm_of(t))));
    for (at, bpm) in tempos {
        if bpm == 0 {
            return Err(MidiError::ZeroTempo { at });
        }
        let us = (60_000_000u32 / bpm).min(SLOWEST_US_PER_QUARTER);
        out.push(Timed {
            tick: tick_at(at, opts.ppq)?,
            order: Order::Control,
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(us))),
        });
    }

    let has_sig_at_zero = resolved.time_sigs.iter().any(|(at, _)| *at == zero);
    let mut sigs: Vec<(Beats, TimeSig)> = Vec::new();
    if !has_sig_at_zero {
        sigs.push((zero, TimeSig::common()));
    }
    sigs.extend(resolved.time_sigs.iter().copied());
    for (at, sig) in sigs {
        if sig.denominator == 0 || !sig.denominator.is_power_of_two() {
            return Err(MidiError::BadTimeSignature {
                at,
                numerator: sig.numerator,
                denominator: sig.denominator,
            });
        }
        let log2 = sig.denominator.trailing_zeros() as u8;
        out.push(Timed {
            tick: tick_at(at, opts.ppq)?,
            order: Order::Control,
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(sig.numerator, log2, 24, 8)),
        });
    }
    Ok(out)
}

/// Events of one voice, in resolved order.
struct VoiceTrack<'e> {
    name: Vec<u8>,
    events: Vec<&'e Event>,
}

fn group_by_voice(events: &[Event]) -> Vec<VoiceTrack<'_>> {
    let mut index: HashMap<Option<&VoiceId>, usize> = HashMap::new();
    let mut tracks: Vec<VoiceTrack<'_>> = Vec::new();
    for ev in events {
        let key = ev.voice.as_ref();
        let i = *index.entry(key).or_insert_with(|| {
            tracks.push(VoiceTrack {
                name: key.map(|v| v.0.clone().into_bytes()).unwrap_or_else(|| b"voice".to_vec()),
                events: Vec::new(),
            });
            tracks.len() - 1
        });
        tracks[i].events.push(ev);
    }
    tracks
}

fn voice_track(
    track: &VoiceTrack<'_>,
    track_index: usize,
    opts: &MidiOptions,
) -> Result<Vec<Timed>, MidiError> {
    let mut out = Vec::new();
    let track_channel = CHANNELS[track_index % CHANNELS.len()];

    // MIDI program state lives per channel, so remember what each channel last received.
    let mut programs: HashMap<u8, u8> = HashMap::new();

    for ev in &track.events {
        let on_tick = tick_at(ev.onset, opts.ppq)?;
        let channel_no = midi_hint(&ev.hints, |h| match h {
            MidiHint::Channel(c) => Some(*c & 0x0F),
            _ => None,
        })
        .unwrap_or(track_channel);
        let channel = u4::new(channel_no);

        // Program: an explicit hint wins, then the instrument in scope, then the option default.
        let desired = midi_hint(&ev.hints, |h| match h {
            MidiHint::ProgramChange(p) => Some(*p & 0x7F),
            _ => None,
        })
        .unwrap_or_else(|| {
            ev.instrument.as_ref().and_then(program_for).unwrap_or(opts.default_program & 0x7F)
        });
        if programs.get(&channel_no) != Some(&desired) {
            out.push(Timed {
                tick: on_tick,
                order: Order::Control,
                kind: TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::ProgramChange { program: u7::new(desired) },
                },
            });
            programs.insert(channel_no, desired);
        }

        for h in &ev.hints {
            if let BackendHint::Midi(MidiHint::ControlChange { controller, value }) = h {
                out.push(Timed {
                    tick: on_tick,
                    order: Order::Control,
                    kind: TrackEventKind::Midi {
                        channel,
                        message: MidiMessage::Controller {
                            controller: u7::new(*controller & 0x7F),
                            value: u7::new(*value & 0x7F),
                        },
                    },
                });
            }
        }

        let midi = ev.pitch.midi();
        let key = u8::try_from(midi)
            .ok()
            .filter(|k| *k <= 127)
            .ok_or(MidiError::PitchOutOfMidiRange { at: ev.onset, midi })?;
        let key = u7::new(key);
        let vel = u7::new(ev.velocity.clamp(1, 127));

        let off_beat = ev.onset + ev.dur;
        let mut off_tick = tick_at(off_beat, opts.ppq)?;
        if off_tick <= on_tick {
            // A zero-length note would be released before it is struck; give it one tick.
            off_tick = on_tick + 1;
        }

        out.push(Timed {
            tick: on_tick,
            order: Order::On,
            kind: TrackEventKind::Midi { channel, message: MidiMessage::NoteOn { key, vel } },
        });
        out.push(Timed {
            tick: off_tick,
            order: Order::Off,
            kind: TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOff { key, vel: u7::new(64) },
            },
        });
    }
    Ok(out)
}

/// Sort by `(tick, order)`, delta-encode, and close the track at `end_tick`.
fn finish_track(mut timed: Vec<Timed>, end_tick: u32) -> Result<Vec<TrackEvent<'static>>, MidiError> {
    timed.sort_by_key(|t| (t.tick, t.order));
    let mut out = Vec::with_capacity(timed.len() + 1);
    let mut cursor = 0u32;
    for t in timed {
        let delta = t.tick - cursor;
        let delta = u28::try_from(delta).ok_or(MidiError::TickOverflow {
            at: Rational32::from_integer(i32::try_from(t.tick).unwrap_or(i32::MAX)),
        })?;
        out.push(TrackEvent { delta, kind: t.kind });
        cursor = t.tick;
    }
    let end = end_tick.max(cursor);
    let delta = u28::try_from(end - cursor).ok_or(MidiError::TickOverflow {
        at: Rational32::from_integer(i32::try_from(end).unwrap_or(i32::MAX)),
    })?;
    out.push(TrackEvent { delta, kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
    Ok(out)
}

/// Render an already resolved piece to Standard MIDI File bytes.
///
/// Use this when you hold a [`Resolved`] from [`resolve`] already (for
/// analysis, say) and do not want to walk the tree twice.
pub fn render_resolved(resolved: &Resolved, opts: &MidiOptions) -> Result<Vec<u8>, MidiError> {
    if opts.ppq == 0 || opts.ppq > 0x7FFF {
        return Err(MidiError::InvalidPpq(opts.ppq));
    }
    let end_tick = tick_at(resolved.total, opts.ppq)?;

    let voices = group_by_voice(&resolved.events);
    let mut tracks: Vec<Vec<TrackEvent<'static>>> = Vec::with_capacity(voices.len() + 1);
    tracks.push(finish_track(conductor_track(resolved, opts)?, end_tick)?);
    for (i, voice) in voices.iter().enumerate() {
        tracks.push(finish_track(voice_track(voice, i, opts)?, end_tick)?);
    }

    // Track names borrow from `voices`; build the Smf inside this scope.
    let mut smf = Smf::new(Header::new(Format::Parallel, Timing::Metrical(u15::new(opts.ppq))));
    let conductor_name: &[u8] = b"conductor";
    for (i, track) in tracks.into_iter().enumerate() {
        let name: &[u8] = if i == 0 { conductor_name } else { &voices[i - 1].name };
        let mut named = Vec::with_capacity(track.len() + 1);
        named.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(name)),
        });
        named.extend(track);
        smf.tracks.push(named);
    }

    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::NoteAttrs;
    use crate::control::Control;
    use crate::music::{chord, n, r, Note};
    use crate::pitch::{Accidental, ChromaticPitch, Letter, Pitch, C3, C4, C5, E4, F3, G3, G4};
    use crate::theory::Key;
    use crate::time::{dot, e, h, q, s, triplet, ts, w, Dynamics, TimeSig};
    use crate::{d, pc, seq};
    use midly::Smf;

    fn parse(bytes: &[u8]) -> Smf<'_> {
        Smf::parse(bytes).expect("midly parses what we wrote")
    }

    /// `(absolute_tick, key, is_on)` for every note message of a track.
    fn notes(track: &[TrackEvent<'_>]) -> Vec<(u32, u8, bool)> {
        let mut t = 0u32;
        let mut out = Vec::new();
        for ev in track {
            t += ev.delta.as_int();
            match ev.kind {
                TrackEventKind::Midi { message: MidiMessage::NoteOn { key, vel }, .. } => {
                    out.push((t, key.as_int(), vel.as_int() > 0))
                }
                TrackEventKind::Midi { message: MidiMessage::NoteOff { key, .. }, .. } => {
                    out.push((t, key.as_int(), false))
                }
                _ => {}
            }
        }
        out
    }

    fn default() -> MidiOptions {
        MidiOptions::default()
    }

    #[test]
    fn every_duration_helper_is_an_integer_tick_count_at_480() {
        for (beats, expect) in [
            (w(), 1920),
            (h(), 960),
            (q(), 480),
            (e(), 240),
            (s(), 120),
            (ts(), 60),
            (dot(q()), 720),
            (dot(ts()), 90),
            (triplet(q()), 320),
            (triplet(ts()), 40),
        ] {
            assert_eq!(ticks(beats, 480), Some(expect), "{beats}");
        }
        // 1/7 of a beat is not integral; nearest tick.
        assert_eq!(ticks(Rational32::new(1, 7), 480), Some(69));
    }

    #[test]
    fn single_quarter_is_one_note_pair_480_ticks_apart() {
        let bytes = render_midi(&n(C4, q()), &default()).unwrap();
        let smf = parse(&bytes);
        assert_eq!(smf.header.format, Format::Parallel);
        assert_eq!(smf.header.timing, Timing::Metrical(u15::new(480)));
        assert_eq!(smf.tracks.len(), 2, "conductor + one voice");
        assert_eq!(notes(&smf.tracks[1]), vec![(0, 60, true), (480, 60, false)]);
    }

    #[test]
    fn conductor_carries_default_tempo_and_meter_when_the_piece_states_none() {
        let bytes = render_midi(&n(C4, q()), &default()).unwrap();
        let smf = parse(&bytes);
        let metas: Vec<_> = smf.tracks[0]
            .iter()
            .filter_map(|ev| match &ev.kind {
                TrackEventKind::Meta(m) => Some(m.to_static()),
                _ => None,
            })
            .collect();
        assert!(metas.contains(&MetaMessage::Tempo(u24::new(500_000))), "120 bpm");
        assert!(metas.contains(&MetaMessage::TimeSignature(4, 2, 24, 8)), "4/4");
        assert_eq!(metas.last(), Some(&MetaMessage::EndOfTrack));
    }

    #[test]
    fn tempo_and_time_signature_controls_reach_the_conductor() {
        let piece = n(C4, q())
            .modify(Control::TimeSignature(TimeSig::new(6, 8)))
            .modify(Control::Tempo(Tempo::bpm(96)));
        let bytes = render_midi(&piece, &default()).unwrap();
        let smf = parse(&bytes);
        let metas: Vec<_> = smf.tracks[0]
            .iter()
            .filter_map(|ev| match &ev.kind {
                TrackEventKind::Meta(m) => Some(m.to_static()),
                _ => None,
            })
            .collect();
        assert!(metas.contains(&MetaMessage::Tempo(u24::new(625_000))), "96 bpm");
        assert!(metas.contains(&MetaMessage::TimeSignature(6, 3, 24, 8)), "6/8");
        assert!(!metas.contains(&MetaMessage::Tempo(u24::new(500_000))), "no default when stated");
    }

    #[test]
    fn non_power_of_two_meter_is_an_error_not_a_panic() {
        let piece = n(C4, q()).modify(Control::TimeSignature(TimeSig::new(7, 5)));
        match render_midi(&piece, &default()) {
            Err(MidiError::BadTimeSignature { numerator: 7, denominator: 5, .. }) => {}
            other => panic!("expected BadTimeSignature, got {other:?}"),
        }
        let piece = n(C4, q()).modify(Control::Tempo(Tempo::bpm(0)));
        assert!(matches!(render_midi(&piece, &default()), Err(MidiError::ZeroTempo { .. })));
    }

    #[test]
    fn a_par_chord_strikes_all_notes_on_the_same_tick() {
        let bytes = render_midi(&chord([C4, E4, G4], h()), &default()).unwrap();
        let smf = parse(&bytes);
        assert_eq!(
            notes(&smf.tracks[1]),
            vec![(0, 60, true), (0, 64, true), (0, 67, true), (960, 60, false), (960, 64, false), (960, 67, false)]
        );
    }

    #[test]
    fn two_voices_make_two_tracks_on_two_channels() {
        let melody = seq![n(E4, q()), n(G4, q())].modify(Control::Voice(VoiceId("melody".into())));
        let bass = seq![n(C3, h())].modify(Control::Voice(VoiceId("bass".into())));
        let bytes = render_midi(&(melody | bass), &default()).unwrap();
        let smf = parse(&bytes);
        assert_eq!(smf.tracks.len(), 3);
        let names: Vec<_> = smf
            .tracks
            .iter()
            .filter_map(|t| match t.first().map(|ev| &ev.kind) {
                Some(TrackEventKind::Meta(MetaMessage::TrackName(b))) => Some(String::from_utf8_lossy(b).to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["conductor", "melody", "bass"]);
        let channel_of = |track: &[TrackEvent<'_>]| {
            track.iter().find_map(|ev| match ev.kind {
                TrackEventKind::Midi { channel, message: MidiMessage::NoteOn { .. } } => Some(channel.as_int()),
                _ => None,
            })
        };
        assert_eq!(channel_of(&smf.tracks[1]), Some(0));
        assert_eq!(channel_of(&smf.tracks[2]), Some(1));
    }

    #[test]
    fn sixteen_voices_skip_the_percussion_channel_and_wrap() {
        let channels: Vec<u8> = (0..16).map(|i| CHANNELS[i % CHANNELS.len()]).collect();
        assert!(!channels[..15].contains(&9));
        assert_eq!(channels[15], 0);
    }

    #[test]
    fn instrument_and_midi_hints_become_program_changes_channels_and_cc() {
        let attrs = NoteAttrs {
            hints: vec![
                BackendHint::Midi(MidiHint::Channel(9)),
                BackendHint::Midi(MidiHint::ProgramChange(40)),
                BackendHint::Midi(MidiHint::ControlChange { controller: 7, value: 100 }),
            ],
            ..NoteAttrs::default()
        };
        let hinted = Music::Note(Note { pitch: Pitch::Chromatic(C4), dur: q(), attrs });
        let piece = seq![hinted, n(E4, q())]
            .modify(Control::Instrument(InstrumentId("bandoneon".into())));
        let bytes = render_midi(&piece, &default()).unwrap();
        let smf = parse(&bytes);
        let msgs: Vec<(u8, MidiMessage)> = smf.tracks[1]
            .iter()
            .filter_map(|ev| match ev.kind {
                TrackEventKind::Midi { channel, message } => Some((channel.as_int(), message)),
                _ => None,
            })
            .collect();
        // Hinted note: channel 9, program 40, CC7=100, then its note.
        assert_eq!(msgs[0], (9, MidiMessage::ProgramChange { program: u7::new(40) }));
        assert_eq!(msgs[1], (9, MidiMessage::Controller { controller: u7::new(7), value: u7::new(100) }));
        assert!(matches!(msgs[2], (9, MidiMessage::NoteOn { .. })));
        // Second note: no hint, so the track channel (0) and the instrument's program (accordion 21).
        assert!(msgs.iter().any(|m| *m == (0, MidiMessage::ProgramChange { program: u7::new(21) })));
        assert!(msgs.iter().any(|m| matches!(m, (0, MidiMessage::NoteOn { key, .. }) if key.as_int() == 64)));
    }

    #[test]
    fn program_table_accepts_numbers_prefixes_and_names() {
        assert_eq!(program_for(&InstrumentId("21".into())), Some(21));
        assert_eq!(program_for(&InstrumentId("gm:43".into())), Some(43));
        assert_eq!(program_for(&InstrumentId("Electric_Bass".into())), Some(33));
        assert_eq!(program_for(&InstrumentId("Double-Bass".into())), Some(43));
        assert_eq!(program_for(&InstrumentId("theremin".into())), None);
        assert_eq!(program_for(&InstrumentId("200".into())), None);
    }

    #[test]
    fn velocity_follows_dynamics_and_notes_out_of_midi_range_are_errors() {
        let bytes = render_midi(&n(C4, q()).modify(Control::Dynamics(Dynamics::Ff)), &default()).unwrap();
        let smf = parse(&bytes);
        let vel = smf.tracks[1].iter().find_map(|ev| match ev.kind {
            TrackEventKind::Midi { message: MidiMessage::NoteOn { vel, .. }, .. } => Some(vel.as_int()),
            _ => None,
        });
        assert_eq!(vel, Some(112));
        let too_high = ChromaticPitch::new(Letter::C, Accidental::Natural, 11); // MIDI 144
        assert!(matches!(
            render_midi(&n(too_high, q()), &default()),
            Err(MidiError::PitchOutOfMidiRange { midi: 144, .. })
        ));
    }

    #[test]
    fn rests_advance_time_and_the_track_ends_at_the_total_duration() {
        let piece = seq![n(C4, q()), r(h()), n(C4, q())];
        let bytes = render_midi(&piece, &default()).unwrap();
        let smf = parse(&bytes);
        assert_eq!(notes(&smf.tracks[1]), vec![(0, 60, true), (480, 60, false), (1440, 60, true), (1920, 60, false)]);
        let end: u32 = smf.tracks[0].iter().map(|ev| ev.delta.as_int()).sum();
        assert_eq!(end, 1920);
    }

    #[test]
    fn invalid_ppq_is_an_error() {
        let opts = MidiOptions { ppq: 0, ..default() };
        assert!(matches!(render_midi(&n(C4, q()), &opts), Err(MidiError::InvalidPpq(0))));
    }

    #[test]
    fn the_readme_sketch_writes_sixteen_beats_and_parses_back() {
        let key = Key::minor(pc!(F));
        let tango = |root: ChromaticPitch| {
            seq![n(root, dot(e())), n(root, s()), n(root, e()), n(root, e()), n(root, q()), n(root, q())]
        };
        let comp_rest = r(q()) + r(dot(q()));
        let melody = seq![n(d!(5), q()), n(d!(b 5), e()), n(d!(4), e()), n(d!(3), h())];
        let bar1 = tango(G3) | comp_rest.clone() | melody.clone();
        let bar2 = tango(C3) | comp_rest.clone() | melody.diatonic_transpose(-1);
        let bar3 = tango(F3) | comp_rest | seq![n(d!(1), w())];
        let piece = seq![bar1, bar2, bar3.clone(), bar3]
            .modify(Control::Key(key))
            .modify(Control::TimeSignature(TimeSig::common()))
            .modify(Control::Tempo(Tempo::bpm(96)));

        let path = std::env::temp_dir().join("musecode-readme-tango.mid");
        write_midi(&piece, &path, &default()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let smf = parse(&bytes);
        let end: u32 = smf.tracks[0].iter().map(|ev| ev.delta.as_int()).sum();
        assert_eq!(end, 16 * 480, "four bars of 4/4 at 480 PPQ");
        let on: Vec<_> = notes(&smf.tracks[1]).into_iter().filter(|(_, _, on)| *on).collect();
        assert_eq!(on.len(), 4 * 6 + 4 + 4 + 1 + 1, "24 bass notes, two 4-note melodies, two whole notes");
        assert!(on.iter().any(|&(t, k, _)| t == 0 && k == 72), "melody starts on C5");
        assert!(on.iter().any(|&(t, k, _)| t == 480 && k == 71), "then Cb5 on beat 2");
        assert_eq!(bytes, render_midi(&piece, &default()).unwrap(), "write_midi and render_midi agree");
        let _ = C5;
    }
}
