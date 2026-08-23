//! `impl Display for Music`: the textual notation specified in `docs/NOTATION.md`.
//!
//! The notation is the display half of the future text syntax (#14): what
//! this module prints is exactly what the parser will have to accept. The
//! layout rules are simple and deterministic:
//!
//! - A note is `pitch:dur` followed by its attribute suffix (`C4:q`, `b3:e.`,
//!   `G3:q~`, `+M3:q->`). A rest is `r:dur`.
//! - A `Par` whose children are all notes of one duration with identical
//!   attributes is a chord: `[C4 E4 G4]:h`.
//! - Any other `Par` is `{ piece | piece }`.
//! - A `Modify` is `ctrl { piece }`; nested modifiers chain: `key(F minor) { tempo(96) { ... } }`.
//! - A `Seq` nested inside a `Seq` is parenthesised: `( C4:q D4:q )`.
//! - A `Seq` whose children are all leaves (notes, rests, chords) prints on
//!   one line. A `Seq` with compound children prints one child per line, with
//!   runs of consecutive leaves sharing a line, so the README sketch reads one
//!   bar per line.
//!
//! Whitespace, including line breaks, carries no meaning; the parser treats
//! every run of whitespace as one separator.

use std::fmt;

use crate::music::{Music, Note};
use crate::time::duration_name;

impl fmt::Display for Music {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        write_block(self, 0, &mut out);
        f.write_str(out.trim_end_matches('\n'))
    }
}

fn note_text(note: &Note) -> String {
    format!("{}:{}{}", note.pitch, duration_name(note.dur), note.attrs)
}

/// If `par`'s children are all notes sharing a duration and attributes,
/// return the chord text.
fn chord_text(children: &[Music]) -> Option<String> {
    let first = match children.first()? {
        Music::Note(n) => n,
        _ => return None,
    };
    let mut pitches = Vec::with_capacity(children.len());
    for child in children {
        match child {
            Music::Note(n) if n.dur == first.dur && n.attrs == first.attrs => pitches.push(n.pitch.to_string()),
            _ => return None,
        }
    }
    Some(format!("[{}]:{}{}", pitches.join(" "), duration_name(first.dur), first.attrs))
}

fn is_leaf(m: &Music) -> bool {
    match m {
        Music::Note(_) | Music::Rest(_) => true,
        Music::Par(children) => chord_text(children).is_some(),
        _ => false,
    }
}

/// One-line rendering of a node, or `None` when it must be laid out as a block.
fn inline(m: &Music) -> Option<String> {
    match m {
        Music::Note(n) => Some(note_text(n)),
        Music::Rest(d) => Some(format!("r:{}", duration_name(*d))),
        Music::Seq(children) => {
            if children.iter().all(is_leaf) {
                Some(children.iter().filter_map(inline).collect::<Vec<_>>().join(" "))
            } else {
                None
            }
        }
        Music::Par(children) => {
            if let Some(chord) = chord_text(children) {
                return Some(chord);
            }
            let parts: Option<Vec<String>> = children.iter().map(inline).collect();
            parts.map(|p| format!("{{ {} }}", p.join(" | ")))
        }
        Music::Modify(ctrl, body) => inline(body).map(|b| format!("{ctrl} {{ {b} }}")),
    }
}

/// Inline text of a node as an *element* of a `Seq`: a nested `Seq` is parenthesised.
fn inline_element(m: &Music) -> Option<String> {
    match m {
        Music::Seq(_) => inline(m).map(|s| format!("( {s} )")),
        _ => inline(m),
    }
}

fn push_line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push(' ');
    }
    out.push_str(text);
    out.push('\n');
}

fn write_block(m: &Music, indent: usize, out: &mut String) {
    if let Some(line) = inline(m) {
        push_line(out, indent, &line);
        return;
    }
    match m {
        Music::Seq(children) => {
            let mut run: Vec<String> = Vec::new();
            for child in children {
                if is_leaf(child) {
                    run.extend(inline(child));
                    continue;
                }
                if !run.is_empty() {
                    push_line(out, indent, &run.join(" "));
                    run.clear();
                }
                match inline_element(child) {
                    Some(line) => push_line(out, indent, &line),
                    None => match child {
                        Music::Seq(_) => {
                            push_line(out, indent, "(");
                            write_block(child, indent + 2, out);
                            push_line(out, indent, ")");
                        }
                        other => write_block(other, indent, out),
                    },
                }
            }
            if !run.is_empty() {
                push_line(out, indent, &run.join(" "));
            }
        }
        Music::Par(children) => {
            push_line(out, indent, "{");
            for (i, child) in children.iter().enumerate() {
                if i > 0 {
                    push_line(out, indent, "|");
                }
                write_block(child, indent + 2, out);
            }
            push_line(out, indent, "}");
        }
        Music::Modify(_, _) => {
            let mut heads = Vec::new();
            let mut body = m;
            while let Music::Modify(ctrl, inner) = body {
                heads.push(format!("{ctrl} {{"));
                body = inner;
            }
            push_line(out, indent, &heads.join(" "));
            write_block(body, indent + 2, out);
            push_line(out, indent, " }".repeat(heads.len()).trim_start());
        }
        Music::Note(_) | Music::Rest(_) => unreachable!("leaves always render inline"),
    }
}

#[cfg(test)]
mod tests {
    use crate::attrs::NoteAttrs;
    use crate::prelude::*;

    fn show(m: &Music) -> String {
        m.to_string()
    }

    #[test]
    fn notes_rests_and_attributes() {
        assert_eq!(show(&n(C4, q())), "C4:q");
        assert_eq!(show(&n(EB4, dot(e())), ), "Eb4:e.");
        assert_eq!(show(&n(d!(b 3), h())), "b3:h");
        assert_eq!(show(&n(Degree { number: 5, alter: 1, octave_shift: -2 }, s())), "#5,,:s");
        assert_eq!(show(&n(Interval { generic: -5, quality: IntervalQuality::Perfect }, q())), "-P5:q");
        assert_eq!(show(&r(triplet(q()))), "r:q3");
        assert_eq!(show(&n(ChromaticPitch::from_midi(-1), q())), "B-2:q");
        let tied_staccato = Music::Note(Note {
            pitch: G3.into(),
            dur: dot(q()),
            attrs: NoteAttrs {
                tie_to_next: true,
                articulation: Some(Articulation::Staccato),
                velocity: Some(100),
                voice_id: Some(VoiceId("bass".into())),
                hints: vec![BackendHint::Midi(MidiHint::Channel(2))],
            },
        });
        assert_eq!(show(&tied_staccato), "G3:q.-.~@v100@voice(bass)@hint(Midi(Channel(2)))");
        let fermata = Music::Note(Note {
            pitch: C4.into(),
            dur: w(),
            attrs: NoteAttrs { articulation: Some(Articulation::Fermata), ..Default::default() },
        });
        assert_eq!(show(&fermata), "C4:w-fermata");
    }

    #[test]
    fn chord_and_par() {
        assert_eq!(show(&chord([C4, E4, G4], h())), "[C4 E4 G4]:h");
        let mixed = par![n(C4, h()), n(E4, q())];
        assert_eq!(show(&mixed), "{ C4:h | E4:q }");
        let voices = par![seq![n(C4, q()), n(D4, q())], seq![n(E4, h())]];
        assert_eq!(show(&voices), "{ C4:q D4:q | E4:h }");
    }

    #[test]
    fn nested_modify_chains_on_one_line_when_the_body_is_inline() {
        let m = seq![n(d!(1), q()), n(d!(3), q())]
            .modify(Control::Key(Key::minor(pc!(F))))
            .modify(Control::Tempo(Tempo::bpm(96)));
        assert_eq!(show(&m), "tempo(96) { key(F minor) { 1:q 3:q } }");
        let t = n(C4, q()).transpose(7).diatonic_transpose(-1);
        assert_eq!(show(&t), "transpose_diatonic(-1) { transpose(7) { C4:q } }");
    }

    #[test]
    fn every_control_has_a_head() {
        let heads = [
            (Control::Tempo(Tempo::bpm(120)), "tempo(120)"),
            (Control::TimeSignature(TimeSig::new(6, 8)), "time(6/8)"),
            (Control::Key(Key::new(pc!(C), Mode::Lydian)), "key(C lydian)"),
            (Control::Scale(Scale::custom(vec![0, 2, 4, 7, 9])), "scale(0 2 4 7 9)"),
            (Control::Scale(Scale::major()), "scale(\"major\" 0 2 4 5 7 9 11)"),
            (Control::Instrument(InstrumentId("bandoneon".into())), "instrument(bandoneon)"),
            (Control::Transpose(-12), "transpose(-12)"),
            (Control::DiatonicTranspose(2), "transpose_diatonic(2)"),
            (Control::Dynamics(Dynamics::Ff), "dyn(ff)"),
            (Control::Dynamics(Dynamics::Crescendo), "dyn(<)"),
            (Control::Articulation(Articulation::Tenuto), "art(tenuto)"),
            (Control::Voice(VoiceId("left".into())), "voice(left)"),
            (Control::Hint(BackendHint::Lilypond(LilypondHint::StemUp)), "hint(Lilypond(StemUp))"),
            (Control::User("k".into(), "v".into()), "user(\"k\" \"v\")"),
        ];
        for (ctrl, head) in heads {
            assert_eq!(show(&n(C4, q()).modify(ctrl)), format!("{head} {{ C4:q }}"));
        }
    }

    #[test]
    fn nested_seq_is_parenthesised_and_compound_seq_breaks_lines() {
        let m = seq![n(C4, q()), seq![n(D4, q()), n(E4, q())], par![n(F4, h()), n(A4, q())], n(G4, q())];
        assert_eq!(show(&m), "C4:q\n( D4:q E4:q )\n{ F4:h | A4:q }\nG4:q");
    }

    #[test]
    fn block_par_and_block_modify() {
        let deep = par![
            seq![n(C4, q()), par![n(D4, h()), n(E4, q())]],
            n(G3, w()),
        ];
        assert_eq!(show(&deep), "{\n  C4:q\n  { D4:h | E4:q }\n|\n  G3:w\n}");
        let wrapped = deep.modify(Control::Key(Key::major(pc!(C)))).modify(Control::Tempo(Tempo::bpm(60)));
        assert_eq!(
            show(&wrapped),
            "tempo(60) { key(C major) {\n  {\n    C4:q\n    { D4:h | E4:q }\n  |\n    G3:w\n  }\n} }"
        );
    }

    #[test]
    fn empty_collections_print_as_brackets() {
        assert_eq!(show(&Music::Seq(vec![])), "");
        assert_eq!(show(&Music::Par(vec![])), "{  }");
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
    fn the_readme_sketch_prints_one_bar_per_line() {
        let text = show(&readme_sketch());
        let expected = "\
tempo(96) { time(4/4) { key(F minor) {
  { G3:e. G3:s G3:e G3:e G3:q G3:q | r:q r:q. | 5:q b5:e 4:e 3:h }
  { C3:e. C3:s C3:e C3:e C3:q C3:q | r:q r:q. | transpose_diatonic(-1) { 5:q b5:e 4:e 3:h } }
  { F3:e. F3:s F3:e F3:e F3:q F3:q | r:q r:q. | 1:w }
  { F3:e. F3:s F3:e F3:e F3:q F3:q | r:q r:q. | 1:w }
} } }";
        assert_eq!(text, expected);
        assert!(text.lines().count() < 15);
    }
}
