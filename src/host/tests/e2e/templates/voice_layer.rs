//! Transparent Plaits voice-layer wiring and opt-in behavior.

use crate::dsp::caps::CapabilitySet;
use crate::dsp::controls;
use crate::dsp::graph::UGenSpec;
use crate::dsp::ugen::{BuildEnv, Node, Template};
use crate::types::manifest::HostManifest;
use crate::value::intern::name_of_kw;

use super::super::{all_finite, rms, E2e};

const TEMPLATES: &[&str] = &[
    "filter-voice",
    "phase-pair-voice",
    "six-bank-a-voice",
    "six-bank-b-voice",
    "six-bank-c-voice",
    "terrain-voice",
    "string-machine-voice",
    "chip-voice",
    "analog-pair-voice",
    "shape-voice",
    "fm-pair-voice",
    "grain-pair-voice",
    "spectrum-voice",
    "wave-grid-voice",
    "chord-layer-voice",
    "speech-voice",
    "swarm-voice",
    "clock-noise-voice",
    "particle-voice",
    "string-voice",
    "modal-voice",
    "dual-kick-voice",
    "dual-snare-voice",
    "dual-hat-voice",
];

fn render(name: &str, controls: &str) -> (Vec<f32>, Vec<f32>) {
    render_with_definition(name, None, controls)
}

fn render_with_definition(
    name: &str,
    definition: Option<&str>,
    controls: &str,
) -> (Vec<f32>, Vec<f32>) {
    let mut e = E2e::new();
    if let Some(source) = definition {
        e.eval(source);
    }
    let pattern = if controls.is_empty() {
        format!("s :{name} > note [:a3] > once")
    } else {
        format!("s :{name} > note [:a3] > {controls} > once")
    };
    e.eval(&pattern);
    let out = e.run_stereo_for(0.5);
    assert!(e.faults.is_empty(), "{name}/{controls}: {:?}", e.faults);
    assert!(e.committed > 0, "{name}/{controls}: event committed");
    assert!(
        all_finite(&out.0) && all_finite(&out.1),
        "{name}/{controls}: finite"
    );
    out
}

fn bitwise_eq(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>), context: &str) {
    assert_eq!(a.0.len(), b.0.len(), "{context}: left length");
    assert_eq!(a.1.len(), b.1.len(), "{context}: right length");
    for (channel, x, y) in [("left", &a.0, &b.0), ("right", &a.1, &b.1)] {
        assert_eq!(x.len(), y.len(), "{context}/{channel}");
        for (frame, (x, y)) in x.iter().zip(y).enumerate() {
            assert_eq!(x.to_bits(), y.to_bits(), "{context}/{channel}/{frame}");
        }
    }
}

fn window_rms(samples: &[f32], start: usize, end: usize) -> f32 {
    rms(&samples[start..end])
}

fn event_onset(samples: &[f32]) -> usize {
    samples
        .iter()
        .position(|sample| *sample != 0.0)
        .expect("rendered event has a nonzero sample")
}

#[test]
fn off_mode_is_bitwise_identical_for_every_plaits_template() {
    for name in TEMPLATES {
        let implicit = render(name, "");
        let explicit = render(name, "lpg-mode :off");
        bitwise_eq(&implicit, &explicit, name);
    }
}

const PRE_CLOCK_NOISE: &str = "inst pre-clock-noise noise-harmonics: float = 0.5 timbre: float = 0.5 morph: float = 0.5:\n\tclock-noise-pair freq noise-harmonics: noise-harmonics timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {clock-noise-pair freq noise-harmonics: noise-harmonics timbre: timbre morph: morph mode: 1 > * amp > aux-out}";
const PRE_DUAL_SNARE: &str = "inst pre-dual-snare snare-harmonics: float = 0.5 timbre: float = 0.5 morph: float = 0.5 snare-sustain: float = 0:\n\tsnare-pair-core freq snare-harmonics: snare-harmonics timbre: timbre morph: morph velocity: velocity mode: 0 snare-sustain: snare-sustain > * amp\n\t\t> + {snare-pair-core freq snare-harmonics: snare-harmonics timbre: timbre morph: morph velocity: velocity mode: 1 snare-sustain: snare-sustain > * amp > aux-out}";
const PRE_SWARM: &str = "inst pre-swarm swarm-spread: float = 0.5 timbre: float = 0.5 morph: float = 0.5 swarm-continuous: float = 0:\n\tswarm-pair-core freq swarm-spread: swarm-spread timbre: timbre morph: morph mode: 0 swarm-continuous: swarm-continuous > * amp\n\t\t> + {swarm-pair-core freq swarm-spread: swarm-spread timbre: timbre morph: morph mode: 1 swarm-continuous: swarm-continuous > * amp > aux-out}";
const PRE_SPEECH: &str = "inst pre-speech speech-harmonics: float = 0 timbre: float = 0.5 morph: float = 0.5 velocity: float = 1 speech-sustain: float = 0:\n\tspeech-original freq speech-harmonics: speech-harmonics timbre: timbre morph: morph velocity: velocity speech-sustain: speech-sustain mode: 0 > * amp\n\t\t> + {speech-original freq speech-harmonics: speech-harmonics timbre: timbre morph: morph velocity: velocity speech-sustain: speech-sustain mode: 1 > * amp > aux-out}";

#[test]
fn pre_wiring_seeds_and_default_renders_are_preserved() {
    let cases = [
        (
            "clock-noise-voice",
            "pre-clock-noise",
            PRE_CLOCK_NOISE,
            Node::ClockNoisePair,
        ),
        (
            "dual-snare-voice",
            "pre-dual-snare",
            PRE_DUAL_SNARE,
            Node::SnarePair,
        ),
        ("swarm-voice", "pre-swarm", PRE_SWARM, Node::SwarmPair),
        (
            "speech-voice",
            "pre-speech",
            PRE_SPEECH,
            Node::SpeechOriginal,
        ),
    ];
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    for (name, copy, source, kernel) in cases {
        let mut e = E2e::new();
        e.eval(source);
        let old_entry = e
            .reg
            .borrow()
            .entries()
            .find(|entry| *name_of_kw(entry.name) == *copy)
            .expect("pre-wiring copy registered")
            .clone();
        let current_entry = e
            .reg
            .borrow()
            .entries()
            .find(|entry| *name_of_kw(entry.name) == *name)
            .expect("wired template registered")
            .clone();
        let old = Template::from_inst(&old_entry.def, &env).expect("old template builds");
        let current = Template::from_inst(&current_entry.def, &env).expect("wired template builds");
        let old_nodes: Vec<_> = old
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, node)| node.node == kernel)
            .collect();
        let current_nodes: Vec<_> = current
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, node)| node.node == kernel)
            .collect();
        assert_eq!(old_nodes.len(), current_nodes.len(), "{name}: kernel count");
        for ((old_index, _), (current_index, _)) in old_nodes.iter().zip(&current_nodes) {
            assert_eq!(
                current.seed_ordinal(*current_index),
                u16::try_from(*old_index).expect("node index fits seed ordinal"),
                "{name}: compiled seed ordinal"
            );
        }
        drop(e);
        bitwise_eq(
            &render(name, ""),
            &render_with_definition(copy, Some(source), ""),
            &format!("{name}/{copy}"),
        );
    }
}

#[test]
fn gate_structure_and_chip_clocked_mapping_are_present() {
    let e = E2e::new();
    let entry = |name: &str| {
        e.reg
            .borrow()
            .entries()
            .find(|entry| *name_of_kw(entry.name) == *name)
            .expect("template registered")
            .clone()
    };
    let fm = entry("fm-pair-voice");
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let fm_template = Template::from_inst(&fm.def, &env).expect("fm-pair template");
    assert_eq!(
        fm_template.nodes().last().map(|node| node.node),
        Some(Node::VactrolGate)
    );
    let aux_index = fm
        .def
        .nodes
        .iter()
        .position(|node| *node == UGenSpec::AuxOut)
        .unwrap();
    let aux_source = fm
        .def
        .edges
        .iter()
        .find(|edge| usize::from(edge.to) == aux_index && edge.port == 0)
        .expect("aux-out input");
    let lane1 = fm
        .def
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| **node == UGenSpec::VactrolGate)
        .find_map(|(gate, _)| {
            fm.def
                .edges
                .iter()
                .find(|edge| usize::from(edge.to) == gate && edge.port == 7)
                .filter(|edge| fm.def.nodes[usize::from(edge.from)] == UGenSpec::Const(1.0))
                .map(|_| gate)
        })
        .expect("lane-1 aux gate");
    assert_eq!(
        usize::from(aux_source.from),
        lane1,
        "aux-out reads lane-1 gate"
    );

    let chip = entry("chip-voice");
    let clocked = controls::row("chip-clocked").expect("chip control").ctl;
    let gate_indexes: Vec<_> = chip
        .def
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| (*node == UGenSpec::VactrolGate).then_some(i))
        .collect();
    assert_eq!(gate_indexes.len(), 2);
    for gate in gate_indexes {
        assert!(chip.def.edges.iter().any(|edge| {
            usize::from(edge.to) == gate
                && edge.port == 8
                && chip.def.nodes[usize::from(edge.from)] == UGenSpec::Param(clocked)
        }));
    }
}

#[test]
fn ping_level_bypass_and_chip_modes_have_expected_audio_shapes() {
    let (ping, _) = render("fm-pair-voice", "lpg-mode :ping");
    let onset = event_onset(&ping);
    let early = window_rms(&ping, onset, onset + 48);
    let body = window_rms(&ping, onset + 96, onset + 480);
    assert!(early < body, "ping attack: {early} >= {body}");
    let (short_decay, _) = render("fm-pair-voice", "lpg-mode :ping > lpg-decay 0.2");
    let (long_decay, _) = render("fm-pair-voice", "lpg-mode :ping > lpg-decay 0.8");
    assert!(
        window_rms(
            &short_decay,
            event_onset(&short_decay) + 2_880,
            event_onset(&short_decay) + 4_800
        ) < window_rms(
            &long_decay,
            event_onset(&long_decay) + 2_880,
            event_onset(&long_decay) + 4_800
        ),
        "decay response"
    );

    let (low_level, _) = render("clock-noise-voice", "lpg-mode :level > velocity 0.5");
    let (high_level, _) = render("clock-noise-voice", "lpg-mode :level > velocity 1");
    assert!(
        window_rms(
            &low_level,
            event_onset(&low_level) + 960,
            event_onset(&low_level) + 3_840
        ) < window_rms(
            &high_level,
            event_onset(&high_level) + 960,
            event_onset(&high_level) + 3_840
        ),
        "level velocity response"
    );

    let (off, _) = render("dual-kick-voice", "");
    let (ping, _) = render("dual-kick-voice", "lpg-mode :ping");
    for (a, b) in off.iter().zip(&ping) {
        assert!(
            (b - 0.8 * a).abs() <= 1.0e-6 + 1.0e-6 * a.abs(),
            "enveloped bypass"
        );
    }

    let (clocked_off, _) = render("chip-voice", "chip-clocked 1");
    let (clocked_ping, _) = render("chip-voice", "chip-clocked 1 > lpg-mode :ping");
    for (a, b) in clocked_off.iter().zip(&clocked_ping) {
        assert!(
            (b - 0.5 * a).abs() <= 1.0e-6 + 1.0e-6 * a.abs(),
            "clocked bypass"
        );
    }
    let (unclocked_off, _) = render("chip-voice", "chip-clocked 0");
    let (unclocked_ping, _) = render("chip-voice", "chip-clocked 0 > lpg-mode :ping");
    assert_ne!(
        unclocked_off
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        unclocked_ping
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        "unclocked chip ping differs"
    );
}

#[test]
fn every_template_is_finite_and_audible_in_ping_and_level_modes() {
    for name in TEMPLATES {
        for mode in ["ping", "level"] {
            let (left, right) = render(name, &format!("lpg-mode :{mode}"));
            assert!(rms(&left) > 1.0e-4, "{name}/{mode}: left silent");
            assert!(rms(&right) > 1.0e-4, "{name}/{mode}: right silent");
        }
    }
}

#[test]
fn editor_metadata_and_opt_in_example_are_available() {
    let editor = HostManifest::spec_default()
        .editor_decl("fm-pair-voice")
        .expect("fm-pair editor");
    let mode = editor
        .params
        .iter()
        .find(|param| param.name == "lpg-mode")
        .unwrap();
    assert_eq!(mode.choices, ["off", "ping", "level"]);
    assert_eq!(mode.default, 0.0);
    for (name, default) in [("lpg-decay", 0.5), ("lpg-color", 0.5)] {
        let param = editor
            .params
            .iter()
            .find(|param| param.name == name)
            .unwrap();
        assert_eq!(param.default, default, "{name}");
    }

    let mut e = E2e::new();
    e.eval(include_str!("../../../../../examples/voice-layer.vact"));
    let audio = e.run_stereo_for(0.5);
    assert!(all_finite(&audio.0) && all_finite(&audio.1));
    assert!(rms(&audio.0) > 1.0e-4 && rms(&audio.1) > 1.0e-4);
}
