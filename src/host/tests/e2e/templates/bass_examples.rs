//! Offline render coverage for the four BASS-41 techno examples.

use super::bass_render::{check, write_wav};
use super::E2e;

const EXAMPLES: [(&str, u32, &str); 4] = [
    (
        "acid-techno",
        132,
        include_str!("../../../../../examples/acid-techno.vact"),
    ),
    (
        "rumble-techno",
        128,
        include_str!("../../../../../examples/rumble-techno.vact"),
    ),
    (
        "offbeat-bass-techno",
        126,
        include_str!("../../../../../examples/offbeat-bass-techno.vact"),
    ),
    (
        "wobble-techno",
        136,
        include_str!("../../../../../examples/wobble-techno.vact"),
    ),
];

const BASS_TEMPLATES: [&str; 6] = [
    ":analog-bass",
    ":acid-bass",
    ":fm-bass",
    ":wobble-bass",
    ":sub-bass",
    ":reese-bass",
];

#[test]
fn bass_examples_render_and_meet_level_targets() {
    let mut failures = Vec::new();
    let mut measurements = Vec::new();

    for (stem, bpm, source) in EXAMPLES {
        let mut e = E2e::new();
        e.eval(source);
        let seconds = 2.0 * 240.0 / f64::from(bpm);
        let (left, right) = e.run_stereo_for(seconds);
        let metrics = check(&left, &right);
        let wav_path = write_wav(&format!("examples/{stem}"), &left, &right);

        let mut problems = Vec::new();
        if !e.faults.is_empty() {
            problems.push(format!("faults={:?}", e.faults));
        }
        if e.committed == 0 {
            problems.push("committed=0".to_owned());
        }
        if !metrics.finite {
            problems.push("finite=false".to_owned());
        }
        if metrics.rms <= 1.0e-3 {
            problems.push("rms <= 1e-3".to_owned());
        }
        if metrics.peak > 1.0 {
            problems.push("peak > 1.0".to_owned());
        }
        if metrics.low < 0.3 {
            problems.push("low-band share < 0.3".to_owned());
        }

        measurements.push(format!(
            "{stem} ({bpm} BPM): committed={}; finite={}; rms={:.6}; peak={:.6}; low={:.6}; wav={}",
            e.committed,
            metrics.finite,
            metrics.rms,
            metrics.peak,
            metrics.low,
            wav_path.display(),
        ));
        if !problems.is_empty() {
            failures.push(format!("{stem}: {}", problems.join(", ")));
        }
    }

    eprintln!("{}", measurements.join("\n"));
    assert!(
        failures.is_empty(),
        "bass example render checks failed:\nmeasurements:\n{}\nfailures:\n{}",
        measurements.join("\n"),
        failures.join("\n"),
    );
}

#[test]
fn bass_examples_use_bass_templates() {
    for (stem, _, source) in EXAMPLES {
        assert!(
            BASS_TEMPLATES
                .iter()
                .any(|template| source.contains(template)),
            "{stem}: example uses at least one dedicated bass template"
        );
    }

    let acid = EXAMPLES[0].2;
    assert!(acid.contains(":acid-bass"), "acid-techno uses acid-bass");
    assert!(
        acid.contains("slide-from"),
        "acid-techno demonstrates slide"
    );
    assert!(acid.contains("accent"), "acid-techno demonstrates accent");

    let wobble = EXAMPLES[3].2;
    assert!(
        wobble.contains(":wobble-bass"),
        "wobble-techno uses wobble-bass"
    );
    assert!(
        wobble.contains("lfo-rate {alt 8 12}"),
        "wobble-techno alternates 1/8 and 1/8T lfo-rate per cycle"
    );
}
