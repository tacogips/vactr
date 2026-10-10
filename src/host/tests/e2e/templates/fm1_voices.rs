//! FM1V-30 registration, rendering, tempo, memory, editor and completion coverage.

use super::{all_finite, E2e};
use crate::complete::{complete, Snapshot};
use crate::dsp::graph::UGenSpec;
use crate::dsp::meta::{self, Curve};
use crate::dsp::ugen::{catalog, BuildEnv, BuildError, Node, Template};
use crate::ns::insts;
use crate::value::intern::name_of_kw;

const BASS: [&str; 6] = [
    "analog-bass",
    "acid-bass",
    "fm-bass",
    "wobble-bass",
    "sub-bass",
    "reese-bass",
];
struct VoiceCase {
    name: &'static str,
    spec: UGenSpec,
    node: Node,
    defaults: &'static [(&'static str, f32)],
    expected_names: &'static [&'static str],
    stepped: &'static [&'static str],
}

const VOICES: [VoiceCase; 6] = [
    VoiceCase {
        name: "kalimba",
        spec: UGenSpec::KalimbaCore,
        node: Node::KalimbaCore,
        defaults: &[
            ("kalimba-beat", 1.5),
            ("kalimba-hardness", 0.5),
            ("kalimba-decay", 2.5),
            ("kalimba-damping", 0.5),
            ("kalimba-body", 0.3),
            ("kalimba-buzz", 0.0),
        ],
        expected_names: &[
            "freq",
            "kalimba-beat",
            "kalimba-hardness",
            "kalimba-decay",
            "kalimba-damping",
            "kalimba-body",
            "kalimba-buzz",
            "amp",
        ],
        stepped: &[],
    },
    VoiceCase {
        name: "tonewheel-organ",
        spec: UGenSpec::TonewheelCore,
        node: Node::TonewheelCore,
        defaults: &[
            ("drawbar1", 8.0),
            ("drawbar2", 8.0),
            ("drawbar3", 8.0),
            ("drawbar4", 0.0),
            ("drawbar5", 0.0),
            ("drawbar6", 0.0),
            ("drawbar7", 0.0),
            ("drawbar8", 0.0),
            ("drawbar9", 0.0),
            ("organ-click", 0.3),
            ("organ-perc", 0.0),
            ("organ-perc-slow", 0.0),
            ("organ-perc-soft", 0.0),
            ("organ-perc-trigger", 1.0),
            ("organ-vibrato", 0.0),
            ("gate-length", 4.0),
        ],
        expected_names: &[
            "freq",
            "drawbar1",
            "drawbar2",
            "drawbar3",
            "drawbar4",
            "drawbar5",
            "drawbar6",
            "drawbar7",
            "drawbar8",
            "drawbar9",
            "organ-click",
            "organ-perc",
            "organ-perc-slow",
            "organ-perc-soft",
            "organ-perc-trigger",
            "organ-vibrato",
            "gate-length",
            "amp",
        ],
        stepped: &[
            "drawbar1",
            "drawbar2",
            "drawbar3",
            "drawbar4",
            "drawbar5",
            "drawbar6",
            "drawbar7",
            "drawbar8",
            "drawbar9",
            "organ-perc",
            "organ-perc-slow",
            "organ-perc-soft",
            "organ-perc-trigger",
            "organ-vibrato",
        ],
    },
    VoiceCase {
        name: "hurdy-gurdy",
        spec: UGenSpec::HurdyGurdyCore,
        node: Node::HurdyGurdyCore,
        defaults: &[
            ("gurdy-wheel", 0.5),
            ("gurdy-pressure", 0.5),
            ("gurdy-melody", 1.0),
            ("gurdy-bourdon", 0.6),
            ("gurdy-fifth", 0.4),
            ("gurdy-trompette", 0.5),
            ("gurdy-drone-key", 43.0),
            ("gurdy-buzz", 0.6),
            ("gurdy-buzz-threshold", 0.5),
            ("gurdy-strokes", 0.0),
            ("gurdy-stroke-depth", 0.5),
            ("gate-length", 8.0),
        ],
        expected_names: &[
            "freq",
            "gurdy-wheel",
            "gurdy-pressure",
            "gurdy-melody",
            "gurdy-bourdon",
            "gurdy-fifth",
            "gurdy-trompette",
            "gurdy-drone-key",
            "gurdy-buzz",
            "gurdy-buzz-threshold",
            "gurdy-strokes",
            "gurdy-stroke-depth",
            "gate-length",
            "amp",
        ],
        stepped: &["gurdy-drone-key", "gurdy-strokes"],
    },
    VoiceCase {
        name: "vosim",
        spec: UGenSpec::VosimCore,
        node: Node::VosimCore,
        defaults: &[
            ("vosim-formant", 900.0),
            ("vosim-pulses", 3.0),
            ("vosim-decay", 0.7),
        ],
        expected_names: &[
            "freq",
            "vosim-formant",
            "vosim-pulses",
            "vosim-decay",
            "attack",
            "decay",
            "sustain",
            "release",
            "amp",
        ],
        stepped: &["vosim-pulses"],
    },
    VoiceCase {
        name: "gendyn",
        spec: UGenSpec::GendynCore,
        node: Node::GendynCore,
        defaults: &[
            ("gendyn-points", 12.0),
            ("gendyn-amp-step", 0.2),
            ("gendyn-dur-step", 0.1),
            ("gendyn-dist", 1.0),
            ("gendyn-spread", 0.1),
        ],
        expected_names: &[
            "freq",
            "gendyn-points",
            "gendyn-amp-step",
            "gendyn-dur-step",
            "gendyn-dist",
            "gendyn-spread",
            "attack",
            "decay",
            "sustain",
            "release",
            "amp",
        ],
        stepped: &["gendyn-points", "gendyn-dist"],
    },
    VoiceCase {
        name: "scanned",
        spec: UGenSpec::ScannedCore,
        node: Node::ScannedCore,
        defaults: &[
            ("scan-stiffness", 0.5),
            ("scan-damping", 0.3),
            ("scan-centering", 0.1),
            ("scan-hammer", 0.3),
            ("scan-position", 0.5),
            ("scan-update", 400.0),
        ],
        expected_names: &[
            "freq",
            "scan-stiffness",
            "scan-damping",
            "scan-centering",
            "scan-hammer",
            "scan-position",
            "scan-update",
            "attack",
            "decay",
            "sustain",
            "release",
            "amp",
        ],
        stepped: &[],
    },
];

#[test]
fn fm1_voices_registered_after_bass() {
    let expected = [
        "kalimba",
        "tonewheel-organ",
        "hurdy-gurdy",
        "vosim",
        "gendyn",
        "scanned",
    ];
    for names in [catalog::TEMPLATE_NAMES, &insts::TEMPLATE_NAMES] {
        assert_eq!(&names[names.len() - 12..names.len() - 6], &BASS);
        assert_eq!(&names[names.len() - 6..], &expected);
    }
}

#[test]
fn fm1_voices_catalog_ports_match_kernels() {
    for voice in VOICES {
        assert_eq!(
            Node::from_spec(&voice.spec),
            voice.node,
            "{} node",
            voice.name
        );
        let ports = catalog::ports(&voice.node);
        let kernel_ports: &[(&str, f32)] = match voice.name {
            "kalimba" => &crate::dsp::ugen::kalimba::PORTS,
            "tonewheel-organ" => &crate::dsp::ugen::tonewheel::PORTS,
            "hurdy-gurdy" => &crate::dsp::ugen::hurdy_gurdy::PORTS,
            "vosim" => &crate::dsp::ugen::vosim::PORTS,
            "gendyn" => &crate::dsp::ugen::gendyn::PORTS,
            _ => &crate::dsp::ugen::scanned::PORTS,
        };
        assert_eq!(ports.len(), kernel_ports.len(), "{} port count", voice.name);
        for (port, &(port_name, default)) in ports.iter().zip(kernel_ports) {
            assert_eq!(
                (port.name, port.default),
                (port_name, default),
                "{} port",
                voice.name
            );
        }
        let build_ports = crate::dsp::build::ports(&voice.spec);
        assert_eq!(build_ports.len(), kernel_ports.len());
        for (actual, (expected, _)) in build_ports.iter().zip(kernel_ports) {
            assert_eq!(*actual, *expected, "{} lowering port", voice.name);
        }
    }
}

#[test]
fn fm1_voices_templates_render_audible_and_bounded() {
    for voice in VOICES {
        let mut e = E2e::new();
        e.eval(&format!("s :{} > note [:c4 :e4 :g4 :c5] > d1", voice.name));
        let output = e.run_for(2.0);
        assert!(e.faults.is_empty(), "{}: {:?}", voice.name, e.faults);
        assert!(e.committed > 0, "{}: event committed", voice.name);
        assert!(all_finite(&output), "{}: finite", voice.name);
        let rms = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();
        let peak = output.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(rms > 1.0e-3, "{}: RMS {rms}", voice.name);
        assert!(peak <= 1.0, "{}: peak {peak}", voice.name);
    }
}

fn audible_window(template: &str, bpm: u32, duration: f64) -> (f64, f64) {
    let mut e = E2e::new();
    e.eval(&format!(
        "use-bpm {bpm}\ns :{template} > note [:c4] > gate-length 8 > once"
    ));
    let output = e.run_for(duration);
    assert!(e.faults.is_empty(), "{template} at {bpm}: {:?}", e.faults);
    let first = output
        .iter()
        .position(|x| x.abs() > 1.0e-4)
        .expect("audible onset");
    let last = output
        .iter()
        .rposition(|x| x.abs() > 1.0e-4)
        .expect("audible tail");
    (first as f64 / 48_000.0, last as f64 / 48_000.0)
}

#[test]
fn fm1_voices_organ_and_gurdy_follow_tempo() {
    let (organ120_start, organ120_end) = audible_window("tonewheel-organ", 120, 2.0);
    let (organ150_start, organ150_end) = audible_window("tonewheel-organ", 150, 2.0);
    let organ120 = organ120_end - organ120_start;
    let organ150 = organ150_end - organ150_start;
    assert!(
        (organ120 - 1.0).abs() <= 0.02,
        "organ 120 BPM audible duration {organ120}"
    );
    assert!(
        (organ150 - 0.8).abs() <= 0.02,
        "organ 150 BPM audible duration {organ150}"
    );
    let (gurdy120_start, gurdy120) = audible_window("hurdy-gurdy", 120, 4.0);
    let (gurdy150_start, gurdy150) = audible_window("hurdy-gurdy", 150, 4.0);
    assert!(
        ((gurdy120 - gurdy150) - 0.2).abs() <= 0.05,
        "gurdy tempo delta {}; 120 BPM audible window ({gurdy120_start}, {gurdy120}), 150 BPM ({gurdy150_start}, {gurdy150})",
        gurdy120 - gurdy150,
    );
}

#[test]
fn fm1_voices_mem_exceeded_one_float_less() {
    let e = E2e::new();
    let reg = e.reg.borrow();
    for voice in VOICES {
        let entry = reg
            .entries()
            .find(|entry| name_of_kw(entry.name).as_ref() == voice.name)
            .expect("template entry");
        let env = BuildEnv {
            sr: 48_000.0,
            caps: crate::dsp::caps::CapabilitySet::native(),
            voice_mem: 24_000,
        };
        let template = Template::from_inst(&entry.def, &env).expect("fits 24000 floats");
        assert!(
            template.mem_total <= 24_000,
            "{}: {}",
            voice.name,
            template.mem_total
        );
        let low = BuildEnv {
            voice_mem: template.mem_total - 1,
            ..env
        };
        assert_eq!(
            Template::from_inst(&entry.def, &low).unwrap_err(),
            BuildError::MemExceeded,
            "{}",
            voice.name
        );
    }
}

#[test]
fn fm1_voices_editor_metadata() {
    let e = E2e::new();
    let reg = e.reg.borrow();
    for voice in VOICES {
        let decl = meta::decl_for(voice.name).expect("metadata declaration");
        assert_eq!(
            decl.params.iter().map(|p| p.name).collect::<Vec<_>>(),
            voice.expected_names,
            "{} parameter order",
            voice.name
        );
        let entry = reg
            .entries()
            .find(|entry| name_of_kw(entry.name).as_ref() == voice.name)
            .expect("template entry");
        for &(param_name, default) in voice.defaults {
            let row = entry
                .params
                .iter()
                .find(|p| name_of_kw(p.name).as_ref() == param_name)
                .expect("header parameter");
            let actual = entry
                .def
                .params
                .iter()
                .find(|(ctl, _)| *ctl == row.ctl)
                .map(|(_, v)| *v)
                .expect("header value");
            let actual = match actual {
                crate::host::wire::Ctl::Const(v) => v,
                crate::host::wire::Ctl::Cell(_) => {
                    panic!("{}.{param_name} unexpected cell", voice.name)
                }
            };
            assert!(
                (actual - default).abs() < 1.0e-6,
                "{}.{param_name} header default {actual} != {default}",
                voice.name
            );
            let meta = decl
                .params
                .iter()
                .find(|p| p.name == param_name)
                .expect("parameter metadata");
            assert!(
                (meta.default - default).abs() < 1.0e-6,
                "{}.{param_name} default {} != {default}",
                voice.name,
                meta.default
            );
            if voice.stepped.contains(&param_name) {
                assert_eq!(
                    meta.curve,
                    Curve::Stepped,
                    "{}.{param_name} curve",
                    voice.name
                );
            } else {
                assert_ne!(
                    meta.curve,
                    Curve::Stepped,
                    "{}.{param_name} curve",
                    voice.name
                );
            }
        }
    }
}

#[test]
fn fm1_voices_completion_offers_templates_and_ugens() {
    let snapshot = Snapshot::builtin();
    for voice in VOICES {
        let prefix = &voice.name[..voice.name.chars().take(3).map(char::len_utf8).sum()];
        let text = format!("s :{prefix}");
        let result = complete(&text, text.len(), &snapshot, 100);
        assert!(
            result
                .items
                .iter()
                .any(|item| item.label == format!(":{}", voice.name)),
            "template completion {}",
            voice.name
        );
    }
    for voice in VOICES {
        let ugen = catalog::ugen_name(&voice.spec);
        let prefix = &ugen[..ugen.len().min(8)];
        let result = complete(prefix, prefix.len(), &snapshot, 100);
        assert!(
            result.items.iter().any(|item| item.label == ugen),
            "UGen completion {ugen}"
        );
    }
}
