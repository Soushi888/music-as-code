//! The README's Piazzolla sketch: a ii-V-I in F minor with a tresillo bass
//! (nuevo tango, not the habanera of the traditional kind), shell-voiced
//! comping and a melody in scale-degree space.
//!
//! `cargo run --example tango` prints the notation and the structural summary,
//! then writes `target/tango.mid`. `just play tango` does that and plays the
//! file through `fluidsynth`; `just listen tango` is an alias.

use musecode_core::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = Key::minor(pc!(F));

    // Nuevo tango bass: the tresillo (3+3+2) on the root of each bar. The
    // rhythm comes from `rhythm::tresillo`; only the pitch is decided here.
    let tango = |root: ChromaticPitch| tresillo().on(root);

    // Shell-voiced comping on beats 2 and 4
    let comp_rest = r(q()) + r(dot(q()));

    // Melody in scale-degree space: descends 5 b5 4 3 (C Cb Bb Ab in F minor)
    let melody = seq![
        n(d!(5), q()), n(d!(b 5), e()), n(d!(4), e()), n(d!(3), h()),
    ];

    // Assemble bars: bass | comp | melody
    let bar1 = tango(G3) | comp_rest.clone() | melody.clone();
    let bar2 = tango(C3) | comp_rest.clone() | melody.diatonic_transpose(-1);
    let bar3 = tango(F3) | comp_rest        | seq![n(d!(1), w())];

    let piece = seq![bar1, bar2, bar3.clone(), bar3]
        .modify(Control::Key(key))
        .modify(Control::TimeSignature(TimeSig::common()))
        .modify(Control::Tempo(Tempo::bpm(96)));

    println!("{piece}\n");
    print!("{}", summary(&piece)?);

    let path = "target/tango.mid";
    write_midi(&piece, path, &MidiOptions::default())?;
    println!("\nwrote {path}");
    Ok(())
}
