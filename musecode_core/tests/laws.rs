//! Property tests for the algebraic laws the IR claims in the README.
//!
//! Trees are generated with chromatic pitches only, spelled through
//! `ChromaticPitch::from_midi`, so structural equality is meaningful for the
//! laws that need it. Degree resolution laws live with the resolver.

use musecode_core::prelude::*;
use proptest::prelude::*;

fn arb_dur() -> impl Strategy<Value = Beats> {
    prop_oneof![
        Just(w()),
        Just(h()),
        Just(q()),
        Just(e()),
        Just(s()),
        Just(ts()),
        Just(dot(q())),
        Just(dot(e())),
        Just(triplet(q())),
        Just(triplet(e())),
        (1i32..=7, 1i32..=8).prop_map(|(num, den)| b(num, den)),
    ]
}

fn arb_pitch() -> impl Strategy<Value = ChromaticPitch> {
    (36i32..=96).prop_map(ChromaticPitch::from_midi)
}

fn arb_control() -> impl Strategy<Value = Control> {
    prop_oneof![
        (-12i32..=12).prop_map(Control::Transpose),
        Just(Control::Key(Key::major(pc!(C)))),
        Just(Control::Key(Key::minor(pc!(F)))),
        Just(Control::Dynamics(Dynamics::F)),
        Just(Control::Articulation(Articulation::Staccato)),
    ]
}

fn arb_leaf() -> impl Strategy<Value = Music> {
    prop_oneof![
        3 => (arb_pitch(), arb_dur()).prop_map(|(p, d)| n(p, d)),
        1 => arb_dur().prop_map(r),
    ]
}

/// Trees of depth at most 4 and at most 64 nodes.
fn arb_music() -> impl Strategy<Value = Music> {
    arb_leaf().prop_recursive(4, 64, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Music::Seq),
            prop::collection::vec(inner.clone(), 0..4).prop_map(Music::Par),
            (arb_control(), inner).prop_map(|(c, body)| body.modify(c)),
        ]
    })
}

fn arb_factor() -> impl Strategy<Value = Beats> {
    prop_oneof![Just(b(1, 2)), Just(b(2, 1)), Just(b(3, 2)), Just(b(2, 3)), Just(b(3, 1))]
}

proptest! {
    #[test]
    fn seq_is_associative(a in arb_music(), b in arb_music(), c in arb_music()) {
        prop_assert_eq!((a.clone() + b.clone()) + c.clone(), a + (b + c));
    }

    #[test]
    fn par_is_associative(a in arb_music(), b in arb_music(), c in arb_music()) {
        prop_assert_eq!((a.clone() | b.clone()) | c.clone(), a | (b | c));
    }

    #[test]
    fn empty_seq_is_the_identity_for_plus(m in arb_music()) {
        prop_assert_eq!(Music::Seq(vec![]) + m.clone(), m.clone());
        prop_assert_eq!(m.clone() + Music::Seq(vec![]), m);
    }

    #[test]
    fn empty_par_is_the_identity_for_bar(m in arb_music()) {
        prop_assert_eq!(Music::Par(vec![]) | m.clone(), m.clone());
        prop_assert_eq!(m.clone() | Music::Par(vec![]), m);
    }

    #[test]
    fn duration_of_seq_is_the_sum(a in arb_music(), b in arb_music()) {
        prop_assert_eq!((a.clone() + b.clone()).duration(), a.duration() + b.duration());
    }

    #[test]
    fn duration_of_par_is_the_max(a in arb_music(), b in arb_music()) {
        prop_assert_eq!((a.clone() | b.clone()).duration(), a.duration().max(b.duration()));
    }

    #[test]
    fn retrograde_is_an_involution(m in arb_music()) {
        prop_assert_eq!(m.clone().retrograde().retrograde(), m);
    }

    #[test]
    fn augment_then_diminish_is_the_identity(m in arb_music(), f in arb_factor()) {
        prop_assert_eq!(m.clone().augment(f).diminish(f), m);
    }

    #[test]
    fn invert_is_an_involution(m in arb_music(), axis in 24i32..=108) {
        prop_assert_eq!(m.clone().invert(axis).invert(axis), m);
    }

    #[test]
    fn transpose_zero_resolves_identically(m in arb_music()) {
        prop_assert_eq!(resolve(&m.clone().transpose(0)), resolve(&m));
    }

    #[test]
    fn resolved_total_equals_duration(m in arb_music()) {
        prop_assert_eq!(resolve(&m).unwrap().total, m.duration());
    }
}
