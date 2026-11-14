use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn options(seconds: &str, tail: &str) -> Options {
    let output = std::env::temp_dir().join(format!(
        "vactr-render-{}-{}.wav",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    Options::parse([
        "test.vact".into(),
        output.to_string_lossy().into_owned(),
        "--seconds".into(),
        seconds.into(),
        "--tail-seconds".into(),
        tail.into(),
        "--sample-rate".into(),
        "8000".into(),
    ])
    .unwrap()
}
const TONE: &str =
    "use-bpm 120\ninst test-tone:\n\tsin-osc freq > * amp\ns :test-tone > note [:a4] > gain 0.1 > d1\n";

#[test]
fn production_onset_duration_events_and_pcm_are_exact() {
    let options = options("0.1", "0");
    let summary = render(&options, TONE).unwrap();
    assert_eq!(summary["frames"], 800);
    assert_eq!(summary["events"], 1);
    assert_eq!(summary["scheduling_preroll_frames"], 0);
    assert_eq!(summary["sections"][0]["frames"], 800);
    assert!(summary["rms"].as_f64().unwrap() > 0.01);
    let wav = std::fs::read(&options.output).unwrap();
    assert_eq!(wav.len(), 44 + 800 * 4);
    assert_eq!(&wav[20..24], &[1, 0, 2, 0]);
    let samples: Vec<i16> = wav[44..]
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    let onset = samples
        .chunks_exact(2)
        .position(|frame| frame[0] != 0)
        .unwrap();
    assert!(onset < 8, "artificial scheduling latency: {onset}");
    assert!(samples.chunks_exact(2).all(|frame| frame[0] == frame[1]));
    std::fs::remove_file(options.output).unwrap();
}

#[test]
fn natural_stop_preserves_bus_tail_without_new_events() {
    let options = options("0.1", "0.2");
    let source = TONE
        .replace(
            "s :test-tone",
            "bus :echo:\n\tdelay time: 0.03 feedback: 0.6 mix: 1\ns :test-tone",
        )
        .replace("gain 0.1 > d1", "gain 0.1 > bus :echo > d1");
    let summary = render(&options, &source).unwrap();
    assert_eq!(summary["frames"], 2400);
    assert_eq!(summary["events"], 1);
    assert!(summary["tail"]["rms"].as_f64().unwrap() > 0.001);
    std::fs::remove_file(options.output).unwrap();
}

#[test]
fn full_score_installs_without_advancing_first_onset() {
    // Seven voices plus two named buses and master after the real prelude
    // match the 64-bar genre-score startup path and its template admission.
    let prefix = include_str!("../tracks/ambient/tidal-glass.vact")
        .split("\nlet changes")
        .next()
        .unwrap();
    // Use a unity master so the score's intentional limiter lookahead does
    // not enter the startup-latency measurement. It is still a third install.
    let prefix = prefix.split("\nmaster:").next().unwrap();
    let source =
        format!("{prefix}\nmaster:\n\tgain 0\ns :score-lead > note [:a4] > gain 0.02 > d1\n");
    let options = options("0.1", "0");
    let summary = render(&options, &source).unwrap();
    assert!((1..=128).contains(&summary["installation_passes"].as_u64().unwrap()));
    assert_eq!(summary["scheduling_preroll_frames"], 0);
    assert_eq!(summary["frames"], 800);
    assert_eq!(summary["events"], 1);
    let wav = std::fs::read(&options.output).unwrap();
    let onset = wav[44..]
        .chunks_exact(4)
        .position(|frame| frame != [0; 4])
        .unwrap();
    assert!(onset < 8, "install passes delayed onset to {onset}");
    std::fs::remove_file(options.output).unwrap();
}

#[test]
fn evaluation_and_engine_install_failures_produce_no_wav() {
    let options = options("0.01", "0");
    assert!(render(&options, "unknown-function 1").is_err());
    assert!(!options.output.exists());
    let mut source = String::new();
    for index in 0..16 {
        source.push_str(&format!("bus :bus-{index}:\n\tgain 0\n"));
    }
    source.push_str(TONE);
    let error = render(&options, &source).unwrap_err();
    assert!(error.contains("acknowledge"), "{error}");
    assert!(!options.output.exists());
}

#[test]
fn invalid_cli_sizes_and_sample_faults_are_rejected() {
    for flag in ["--bpm", "--seconds"] {
        for value in ["NaN", "inf", "0", "-1"] {
            assert!(Options::parse(["a".into(), "b".into(), flag.into(), value.into()]).is_err());
        }
    }
    assert!(Options::parse(["a".into(), "a".into()]).is_err());
    assert!(frame_count(1e20, 48000).is_err());
    for sample in [f32::NAN, f32::INFINITY, 1.0001, -1.0001] {
        assert!(Metrics::new(2.0, 8000, 80).add(0, &[sample, 0.0]).is_err());
    }
}

#[test]
fn fractional_bpm_nested_patterns_have_no_late_events_at_sample_boundaries() {
    let mut options = options("0.1", "0");
    options.seconds = None;
    options.cycles = 64;
    options.rate = 48000;
    let source = TONE
        .replace("120", "126")
        .replace("note [:a4]", "note [:a4 [:a4 nil] :a4 [:a4 :a4]]");
    let summary = render(&options, &source).unwrap();
    assert_eq!(summary["frames"], 5_851_429);
    assert_eq!(summary["events"], 320);
    assert_eq!(summary["scheduling_preroll_frames"], 0);
    std::fs::remove_file(options.output).unwrap();
}

#[test]
fn outbound_timestamps_keep_nearest_samples_inside_rounding_intervals() {
    for rate in [8000, 44100, 48000, 192000] {
        assert_eq!(audit::sample_time(0.0, rate), 0.0);
        for numerator in 1..1000 {
            let time = f64::from(numerator) * 40.0 / 21.0;
            let expected = (time * f64::from(rate)).round();
            let stamped = audit::sample_time(time, rate);
            assert_eq!((stamped * f64::from(rate)).round(), expected);
            assert!(stamped > expected / f64::from(rate));
        }
    }
}
