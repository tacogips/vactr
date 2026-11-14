//! Named bass preset coverage and offline render checks.

use super::bass_render::{check, write_wav};
use super::E2e;

const PRESETS: [(&str, &str, &str); 19] = [
    ("analog-pluck", "analog", "[:c2 :c2 :g1 :bb1]"),
    ("analog-driven", "analog", "[:c2 :c2 :g1 :bb1]"),
    ("analog-square", "analog", "[:c2 :c2 :g1 :bb1]"),
    (
        "acid-squelch",
        "acid",
        "[:c2 :c2 :eb2 :c3 :c2 :g1 :c2 :bb1] > accent [1 0 0 1 0 0 1 0] > slide-from [0 0 0 -9 12 0 -7 0] > cut 1",
    ),
    (
        "acid-rolling",
        "acid",
        "[:c2 :c2 :eb2 :c3 :c2 :g1 :c2 :bb1] > accent [1 0 0 1 0 0 1 0] > slide-from [0 0 0 -9 12 0 -7 0] > cut 1",
    ),
    (
        "acid-deep",
        "acid",
        "[:c2 :c2 :eb2 :c3 :c2 :g1 :c2 :bb1] > accent [1 0 0 1 0 0 1 0] > slide-from [0 0 0 -9 12 0 -7 0] > cut 1",
    ),
    ("fm-pluck", "fm", "[:c2 :c2 :c3 :c2]"),
    ("fm-digital-grit", "fm", "[:c2 :c2 :c3 :c2]"),
    ("fm-metal", "fm", "[:c2 :c2 :c3 :c2]"),
    ("wobble-quarter", "wobble", "[:c1 :g0]"),
    ("wobble-eighth", "wobble", "[:c1 :g0]"),
    ("wobble-triplet", "wobble", "[:c1 :g0]"),
    ("wobble-free", "wobble", "[:c1 :g0]"),
    ("sub-deep", "sub", "[:c1 nil :c1 :g0]"),
    ("sub-driven", "sub", "[:c1 nil :c1 :g0]"),
    ("sub-long", "sub", "[:c1 nil :c1 :g0]"),
    ("reese-hoover", "reese", "[:c1 :c1 :eb1 :c1]"),
    ("reese-dark", "reese", "[:c1 :c1 :eb1 :c1]"),
    ("reese-rumble", "reese", "[:c1 :c1 :eb1 :c1]"),
];

const SOURCE: &str = include_str!("../../../../../examples/bass-presets.vact");

#[test]
fn bass_preset_fn_pipes_to_sink() {
    let mut e = E2e::new();
    e.eval("fn p notes:\n\ts :sub-bass > note notes\np [:c2] > d1");
    let _ = e.run_stereo_for(0.1);
    assert!(e.faults.is_empty(), "preset function: {:?}", e.faults);
    assert!(e.committed > 0, "preset function committed an event");
}

#[test]
fn bass_presets_cover_every_family() {
    let mut counts = [0_usize; 6];
    let families = ["analog", "acid", "fm", "wobble", "sub", "reese"];
    let names: Vec<_> = SOURCE
        .lines()
        .filter_map(|line| line.strip_prefix("fn "))
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert!(names.len() >= 18, "found {} presets", names.len());
    for name in names {
        let (family, _) = name
            .split_once('-')
            .unwrap_or_else(|| panic!("preset {name} needs a family prefix"));
        let index = families
            .iter()
            .position(|candidate| *candidate == family)
            .unwrap_or_else(|| panic!("unknown preset family in {name}"));
        counts[index] += 1;
    }
    for (family, count) in families.iter().zip(counts) {
        assert!(count >= 3, "{family} has only {count} presets");
    }
}

#[test]
fn bass_presets_render_and_meet_level_targets() {
    let mut failures = Vec::new();
    let mut metrics = Vec::new();
    for (name, family, phrase) in PRESETS {
        let mut e = E2e::new();
        e.eval(SOURCE);
        e.eval(&format!("{name} {phrase} > d1"));
        let (left, right) = e.run_stereo_for(2.0);
        let result = check(&left, &right);
        write_wav(&format!("presets/{name}"), &left, &right);
        metrics.push(format!(
            "{name}: rms={:.6}, peak={:.6}, low={:.6}",
            result.rms, result.peak, result.low
        ));

        let min_low = match family {
            "sub" => 0.8,
            "analog" | "reese" | "wobble" | "fm" => 0.5,
            "acid" => 0.0,
            _ => unreachable!("PRESETS family must be known"),
        };
        let valid = e.faults.is_empty()
            && e.committed > 0
            && result.finite
            && result.rms > 1.0e-3
            && result.peak <= 1.0
            && result.low >= min_low;
        if !valid {
            failures.push(format!(
                "{name}: faults={:?}, committed={}, finite={}, rms={:.6}, peak={:.6}, low={:.6}, required_low={min_low:.2}",
                e.faults, e.committed, result.finite, result.rms, result.peak, result.low
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "preset render failures:\n{}",
        failures.join("\n")
    );
    println!("PRESET_METRICS\n{}", metrics.join("\n"));
}
