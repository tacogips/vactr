//! Offline render coverage for the FM1 algorithm and voice examples.

use super::bass_render::check;
use super::E2e;
use crate::dsp::ugen::fm::patch::Fm6Patch;

const EXAMPLES: [(&str, u32, &str); 5] = [
    (
        "fm6-algorithms",
        120,
        include_str!("../../../../../examples/fm6-algorithms.vact"),
    ),
    (
        "kalimba",
        108,
        include_str!("../../../../../examples/kalimba.vact"),
    ),
    (
        "tonewheel-organ",
        112,
        include_str!("../../../../../examples/tonewheel-organ.vact"),
    ),
    (
        "hurdy-gurdy",
        96,
        include_str!("../../../../../examples/hurdy-gurdy.vact"),
    ),
    (
        "experimental-oscillators",
        100,
        include_str!("../../../../../examples/experimental-oscillators.vact"),
    ),
];

#[test]
fn fm1_examples_render_audible_and_bounded() {
    for (name, bpm, source) in EXAMPLES {
        let mut e = E2e::new();
        e.eval(source);
        let seconds = 2.0 * 240.0 / f64::from(bpm);
        let (left, right) = e.run_stereo_for(seconds);
        let metrics = check(&left, &right);

        assert!(e.faults.is_empty(), "{name}: faults={:?}", e.faults);
        assert!(e.committed > 0, "{name}: no event was committed");
        assert!(metrics.finite, "{name}: rendered samples are not finite");
        assert!(metrics.rms > 1.0e-3, "{name}: rms={}", metrics.rms);
        assert!(metrics.peak <= 1.0, "{name}: peak={}", metrics.peak);
    }
}

#[test]
fn fm1_examples_mention_their_target_names() {
    let expected: [(&str, &[&str]); 5] = [
        ("fm6-algorithms", &[":fm", "algorithm", "fm6-core"]),
        ("kalimba", &[":kalimba"]),
        ("tonewheel-organ", &[":tonewheel-organ", "rotary"]),
        ("hurdy-gurdy", &[":hurdy-gurdy"]),
        (
            "experimental-oscillators",
            &[":vosim", ":gendyn", ":scanned"],
        ),
    ];

    for ((name, _, source), (expected_name, needles)) in EXAMPLES.iter().zip(expected) {
        assert_eq!(*name, expected_name);
        for needle in needles {
            assert!(source.contains(needle), "{name}: missing {needle}");
        }
    }
}

#[test]
fn fm1_examples_patch_is_in_range() {
    let source = EXAMPLES[0].2;
    let patch_body = source
        .split_once("let epiano-patch [")
        .unwrap_or_else(|| panic!("fm6-algorithms: missing epiano-patch declaration"))
        .1
        .split_once(']')
        .unwrap_or_else(|| panic!("fm6-algorithms: unterminated epiano-patch list"))
        .0;
    let values: Vec<f32> = patch_body
        .lines()
        .flat_map(|line| {
            line.split('#')
                .next()
                .unwrap_or_default()
                .split_whitespace()
        })
        .map(|value| {
            value
                .parse::<f32>()
                .unwrap_or_else(|error| panic!("invalid epiano-patch value {value:?}: {error}"))
        })
        .collect();

    assert_eq!(values.len(), 155, "epiano-patch must contain 155 values");
    assert!(
        Fm6Patch::from_flat(&values).is_ok(),
        "epiano-patch values must be in range"
    );
}
