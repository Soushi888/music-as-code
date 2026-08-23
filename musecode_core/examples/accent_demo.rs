//! The tango bass with and without accents, for the ear.
use musecode_core::prelude::*;

fn piece(marks: Option<&str>) -> Music {
    let bass = |root: ChromaticPitch| match marks {
        Some(m) => tresillo().accents(m).unwrap().on(root),
        None => tresillo().on(root),
    };
    let comp_rest = r(q()) + r(dot(q()));
    let melody = seq![n(d!(5), q()), n(d!(b 5), e()), n(d!(4), e()), n(d!(3), h())];
    let bars: Vec<Music> = [G3, C3, F3, F3]
        .iter()
        .enumerate()
        .map(|(i, &root)| {
            let top = match i {
                0 => melody.clone(),
                1 => melody.clone().diatonic_transpose(-1),
                _ => seq![n(d!(1), w())],
            };
            bass(root) | comp_rest.clone() | top
        })
        .collect();
    Music::Seq(bars)
        .modify(Control::Key(Key::minor(pc!(F))))
        .modify(Control::TimeSignature(TimeSig::common()))
        .modify(Control::Tempo(Tempo::bpm(96)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (name, marks) in [("flat", None), ("marcato", Some("^xX"))] {
        let m = piece(marks);
        // Report the bass alone, so the melody's events do not interleave.
        let solo = match marks {
            Some(mk) => tresillo().accents(mk).unwrap().on(G3),
            None => tresillo().on(G3),
        };
        let v: Vec<u8> = resolve(&solo)?.events.iter().map(|e| e.velocity).collect();
        println!("{name}: bass velocities {v:?}  grid {}", match marks {
            Some(mk) => tresillo().accents(mk).unwrap().accent_grid(),
            None => tresillo().accent_grid(),
        });
        write_midi(&m, &format!("target/accent-{name}.mid"), &MidiOptions::default())?;
    }
    Ok(())
}
