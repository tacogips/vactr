//! End-to-end registration, rendering and editor checks for the bass templates.

use super::bass_render::{check, SR};
use super::{all_finite, E2e};
use crate::dsp::graph::UGenSpec;
use crate::dsp::ugen::{bass_voice, catalog, Node};
use crate::ns::insts;

const BASS_TEMPLATES: [&str; 6] = [
    "analog-bass",
    "acid-bass",
    "fm-bass",
    "wobble-bass",
    "sub-bass",
    "reese-bass",
];

#[test]
fn bass_templates_are_registered_last() {
    assert_eq!(
        &catalog::TEMPLATE_NAMES[catalog::TEMPLATE_NAMES.len() - BASS_TEMPLATES.len()..],
        BASS_TEMPLATES.as_slice()
    );
    assert_eq!(
        &insts::TEMPLATE_NAMES[insts::TEMPLATE_NAMES.len() - BASS_TEMPLATES.len()..],
        BASS_TEMPLATES.as_slice()
    );
}

#[test]
fn bass_catalog_ports_match_kernel_contract() {
    let catalog_ports = catalog::ports(&Node::BassCore);
    assert_eq!(catalog_ports.len(), 32);
    assert_eq!(catalog_ports.len(), bass_voice::PORTS.len());
    for (index, (catalog_port, kernel_port)) in
        catalog_ports.iter().zip(bass_voice::PORTS).enumerate()
    {
        assert_eq!(
            (catalog_port.name, catalog_port.default),
            kernel_port,
            "catalog port {index}"
        );
    }

    let build_ports = crate::dsp::build::ports(&UGenSpec::BassCore);
    assert_eq!(build_ports.len(), bass_voice::PORTS.len());
    for (index, (build_port, (kernel_name, _))) in
        build_ports.iter().zip(bass_voice::PORTS).enumerate()
    {
        assert_eq!(*build_port, kernel_name, "build port {index}");
    }
}

#[test]
fn bass_templates_render_audible_and_bounded() {
    for template in BASS_TEMPLATES {
        let mut e = E2e::new();
        e.eval(&format!("s :{template} > note [:c2 :c2 :g1 :c2] > d1"));
        let (left, right) = e.run_stereo_for(4.0);
        let result = check(&left, &right);
        assert!(e.faults.is_empty(), "{template}: {:?}", e.faults);
        assert!(e.committed > 0, "{template}: committed event");
        assert!(result.finite, "{template}: finite stereo render");
        assert!(result.rms > 1.0e-3, "{template}: RMS {}", result.rms);
        assert!(result.peak <= 1.0, "{template}: peak {}", result.peak);
        assert!(
            all_finite(&left) && all_finite(&right),
            "{template}: finite channels"
        );
    }
}

#[allow(clippy::cast_precision_loss)]
fn frame_rms(left: &[f32], right: &[f32]) -> Vec<f32> {
    let frame_size = usize::try_from(SR).expect("sample rate fits usize") / 100;
    left.chunks_exact(frame_size)
        .zip(right.chunks_exact(frame_size))
        .map(|(left, right)| {
            let mono: Vec<f32> = left
                .iter()
                .zip(right)
                .map(|(&left_sample, &right_sample)| (left_sample + right_sample) * 0.5)
                .collect();
            super::rms(&mono)
        })
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn autocorrelation_peak_lag(frames: &[f32]) -> usize {
    let mean = frames.iter().sum::<f32>() / frames.len() as f32;
    (20..=80)
        .map(|lag| {
            let correlation = frames[..frames.len() - lag]
                .iter()
                .zip(&frames[lag..])
                .map(|(&a, &b)| (a - mean) * (b - mean))
                .sum::<f32>();
            (lag, correlation)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(lag, _)| lag)
        .expect("the lag search is non-empty")
}

#[test]
fn bass_wobble_follows_tempo() {
    for (bpm, expected_lag) in [(120, 50), (150, 40)] {
        let mut e = E2e::new();
        e.eval(&format!(
            "use-bpm {bpm}\ns :wobble-bass > note [:c1] > gate-length 64 > lfo-depth 1 > d1"
        ));
        let (left, right) = e.run_stereo_for(4.0);
        let result = check(&left, &right);
        assert!(e.faults.is_empty(), "{bpm} BPM: {:?}", e.faults);
        assert!(e.committed > 0, "{bpm} BPM: committed event");
        assert!(result.finite, "{bpm} BPM: finite stereo render");
        assert!(result.rms > 1.0e-3, "{bpm} BPM: RMS {}", result.rms);

        let frames = frame_rms(&left, &right);
        let measured_lag = autocorrelation_peak_lag(&frames);
        assert!(
            measured_lag.abs_diff(expected_lag) <= 3,
            "{bpm} BPM: expected {expected_lag} frames, measured {measured_lag}"
        );
    }
}

#[test]
fn bass_slide_pattern_renders() {
    let mut e = E2e::new();
    e.eval(
        "s :acid-bass > note [:c2 :c3 :c2 :eb2] > slide-from [0 -12 12 -3] > accent [1 0 0 1] > cut 1 > d1",
    );
    let (left, right) = e.run_stereo_for(2.0);
    let result = check(&left, &right);
    assert!(e.faults.is_empty(), "acid slide: {:?}", e.faults);
    assert!(e.committed > 0, "acid slide: committed event");
    assert!(result.finite, "acid slide: finite stereo render");
    assert!(result.rms > 1.0e-3, "acid slide: RMS {}", result.rms);
    assert!(result.peak <= 1.0, "acid slide: peak {}", result.peak);
    assert!(
        all_finite(&left) && all_finite(&right),
        "acid slide: finite channels"
    );
}

fn expected_default(param: &crate::dsp::meta::ParamMeta, value: &str) -> f32 {
    if !param.choices.is_empty() {
        let keyword = value.strip_prefix(':').unwrap_or(value);
        let index = param
            .choices
            .iter()
            .position(|choice| *choice == keyword)
            .unwrap_or_else(|| panic!("{}: unknown enum default {value}", param.name));
        #[allow(clippy::cast_precision_loss)]
        {
            index as f32
        }
    } else {
        match value {
            "true" => 1.0,
            "false" => 0.0,
            _ => value
                .parse::<f32>()
                .unwrap_or_else(|error| panic!("{}: invalid default {value}: {error}", param.name)),
        }
    }
}

#[test]
fn bass_editor_defaults_match_headers() {
    const DEFAULTS: &[(&str, &[(&str, &str)])] = &[
        (
            "analog-bass",
            &[
                ("wave", "saw"),
                ("cutoff", "420"),
                ("res", "0.35"),
                ("env-mod", "2.5"),
                ("env-decay", "0.18"),
                ("accent", "0"),
                ("sub-level", "0.35"),
                ("drive", "0.25"),
                ("gate-length", "0.9"),
                ("amp-attack", "0.002"),
                ("amp-decay", "0.4"),
                ("sustain", "0.85"),
                ("release", "0.04"),
                ("slide-from", "0"),
                ("slide-time", "0.06"),
            ],
        ),
        (
            "acid-bass",
            &[
                ("wave", "saw"),
                ("cutoff", "320"),
                ("res", "0.72"),
                ("env-mod", "3.2"),
                ("env-decay", "0.22"),
                ("accent", "0"),
                ("slide-from", "0"),
                ("slide-time", "0.06"),
                ("drive", "0.3"),
                ("gate-length", "0.55"),
                ("release", "0.02"),
            ],
        ),
        (
            "fm-bass",
            &[
                ("ratio", "1"),
                ("index", "2.5"),
                ("fm-feedback", "0.35"),
                ("fold", "0"),
                ("bit-depth", "16"),
                ("cutoff", "2400"),
                ("res", "0.1"),
                ("env-mod", "1"),
                ("env-decay", "0.15"),
                ("accent", "0"),
                ("drive", "0.1"),
                ("gate-length", "0.8"),
                ("amp-decay", "0.25"),
                ("sustain", "0.45"),
                ("release", "0.05"),
                ("slide-from", "0"),
                ("slide-time", "0.05"),
            ],
        ),
        (
            "wobble-bass",
            &[
                ("wave", "saw"),
                ("detune", "0.12"),
                ("sub-level", "0.4"),
                ("cutoff", "180"),
                ("res", "0.45"),
                ("drive", "0.35"),
                ("lfo-wave", "sine"),
                ("lfo-rate", "4"),
                ("lfo-depth", "0.8"),
                ("lfo-offset", "0"),
                ("lfo-retrigger", "true"),
                ("lfo-sync", "true"),
                ("gate-length", "4"),
                ("release", "0.08"),
                ("slide-from", "0"),
                ("slide-time", "0.08"),
            ],
        ),
        (
            "sub-bass",
            &[
                ("wave", "sine"),
                ("cutoff", "300"),
                ("res", "0"),
                ("drive", "0.15"),
                ("click-level", "0.2"),
                ("gate-length", "1.5"),
                ("amp-attack", "0.003"),
                ("release", "0.06"),
                ("slide-from", "0"),
                ("slide-time", "0.05"),
            ],
        ),
        (
            "reese-bass",
            &[
                ("wave", "saw"),
                ("detune", "0.18"),
                ("sub-level", "0.3"),
                ("cutoff", "650"),
                ("res", "0.2"),
                ("env-mod", "0.8"),
                ("env-decay", "0.6"),
                ("drive", "0.3"),
                ("gate-length", "3.5"),
                ("amp-attack", "0.01"),
                ("release", "0.12"),
                ("slide-from", "0"),
                ("slide-time", "0.1"),
            ],
        ),
    ];

    for (template, defaults) in DEFAULTS {
        let declaration = crate::dsp::meta::decl_for(template)
            .unwrap_or_else(|| panic!("{template}: editor declaration"));
        for &(name, value) in defaults.iter() {
            let param = declaration
                .params
                .iter()
                .find(|param| param.name == name)
                .unwrap_or_else(|| panic!("{template}.{name}: editor parameter"));
            let expected = expected_default(param, value);
            assert_eq!(
                param.default, expected,
                "{template}.{name}: expected header default {value}"
            );
        }
    }
}
