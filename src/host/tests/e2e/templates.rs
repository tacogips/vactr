//! Every prelude synthesis template, compiled from `src/prelude/templates.vact`
//! through a real `Evaluator` + `InstRegistry`, installed into a real
//! `Engine` and rendered through `Runtime` (design 12.4, 12.8.6, 12.8.12;
//! TASK-008 criterion 2), plus the `HostManifest` editor-metadata coverage
//! of design 13.5.
//!
//! Serial repair R2 fixed the BE-FINAL STOP finding (the `ports()`-vs-
//! runtime-catalog port drift in `src/dsp/build.rs`, and the unresolved
//! `bank`/`table`/`source` header default in `src/ns/insts.rs` /
//! `src/sched/commit.rs`): all seven templates now render audible output.

mod analog_pair;
mod bass;
mod bass_examples;
mod bass_presets;
mod bass_render;
mod braids_cloud;
mod braids_digital;
mod braids_filter;
mod braids_five;
mod braids_fm;
mod braids_formant;
mod braids_noise;
mod braids_percussion;
mod braids_physical;
mod braids_struck;
mod braids_subsync;
mod braids_triple;
mod braids_wave_bank;
mod braids_wave_line;
mod chip;
mod chord_pair;
mod coverage;
mod digital_drum;
mod digital_kit;
mod digital_metal;
mod elements;
mod feedback_metal;
mod fm1_examples;
mod fm1_voices;
mod fm_mod_algorithm;
mod frame_keyframe;
mod frame_lfo;
mod golden;
mod grain_pair;
mod migrated_pairs;
mod modal;
mod number_station;
mod particle;
mod peak_function;
mod peak_pulse;
mod quad_stems;
mod rings_part;
mod select_output;
mod shape_pair;
mod six_op_original;
mod speech_original;
mod stage_chain;
mod stage_linked;
mod stage_segment;
mod string_choir;
mod string_machine_pair;
mod string_voice;
mod table_terrain_pair;
mod terrain_pair;
mod tidal_function;
mod tidal_poly;
mod voice_engines;
mod voice_layer;

use std::sync::Arc;

use super::{all_finite, rms, E2e};
use crate::dsp::graph::EffectKind;
use crate::dsp::ugen::catalog::UGEN_NAMES;
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::insts::TEMPLATE_NAMES;
use crate::types::manifest::HostManifest;
use crate::value::intern::intern_kw;
use crate::vm::fail::{FailCode, Failure};

/// A deterministic, non-silent loader for the sample-, table- and
/// grain-source-backed templates (`sampler`, `wavetable`, `granular`): one
/// second of a 220 Hz sine at 48 kHz, mono, regardless of what resource is
/// asked for.
#[derive(Default)]
struct SynthLoader;

impl SampleLoader for SynthLoader {
    fn load(&mut self, _src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let n = 48_000;
        let frames: Vec<f32> = (0..n)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let t = i as f32 / 48_000.0;
                (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.8
            })
            .collect();
        Ok(Arc::new(SampleData {
            rate: 48_000,
            channels: 1,
            frames: frames.into_boxed_slice(),
        }))
    }
}

/// Each of the seven prelude templates compiles from source, registers
/// under its name, installs into the engine (a render commits at least one
/// event and reports no unexpected fault), and renders finite, non-silent,
/// zero-allocation audio. A persistent slot (not `once`) is used because
/// the resource-backed templates (`sampler`, `wavetable`, `granular`, R2b)
/// request their sample asynchronously: the first cycle or two may drop
/// with a transient `host-unavailable` ("still loading") fault while the
/// `SampleTable` gate opens (driven open here by ticking and rendering
/// through several cycles); a later cycle's event then renders for real.
#[test]
fn every_template_installs_and_renders_non_silent() {
    for name in TEMPLATE_NAMES {
        let mut e = E2e::with_loader(Box::new(SynthLoader));
        assert!(
            e.reg.borrow().id_of(intern_kw(name)).is_some(),
            "{name}: registered under its name"
        );
        e.eval(&format!("s :{name} > note [:c4] > d1"));
        // 4 cycles (8 s at the 120 bpm default): well past the gate's
        // one-render round trip, with several commit attempts after it.
        let left = e.run_for(8.0);
        for f in &e.faults {
            assert_eq!(
                f.code,
                FailCode::HostUnavailable,
                "{name}: unexpected fault {f:?}"
            );
        }
        assert!(all_finite(&left), "{name}: every sample finite");
        assert!(e.committed > 0, "{name}: installed (an event committed)");
        let level = rms(&left);
        assert!(level > 1.0e-3, "{name}: sounds (rms {level})");
    }
}

/// The mean absolute sample-to-sample difference: a cheap high-frequency
/// energy proxy (more high content moves adjacent samples further apart).
fn hf_energy(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let sum: f32 = samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
    #[allow(clippy::cast_precision_loss)]
    {
        sum / (samples.len() - 1) as f32
    }
}

/// `s :analog > note [:a4] > cutoff 100 > once` vs `cutoff 12000`: the
/// `inst` header parameter `cutoff` (a control, per serial repair R1's
/// `inst control` native) is an ordinary pattern control, so a low and a
/// high cutoff render measurably different output (the ladder filter
/// passes far less of `:a4`'s harmonic content at 100 Hz than at 12 kHz).
#[test]
fn a_template_parameter_is_an_ordinary_pattern_control() {
    let mut low = E2e::new();
    low.eval("s :analog > note [:a4] > cutoff 100 > once");
    let low_out = low.run_for(1.0);

    let mut high = E2e::new();
    high.eval("s :analog > note [:a4] > cutoff 12000 > once");
    let high_out = high.run_for(1.0);

    let low_hf = hf_energy(&low_out);
    let high_hf = hf_energy(&high_out);
    assert!(all_finite(&low_out) && all_finite(&high_out));
    assert!(
        rms(&low_out) > 1.0e-3 && rms(&high_out) > 1.0e-3,
        "both sound"
    );
    assert!(
        high_hf > low_hf * 3.0,
        "cutoff changes the output: low cutoff hf={low_hf}, high cutoff hf={high_hf}"
    );
}

/// A user-authored name absent from the global control table is compiled as
/// a pattern step, encoded through the installed header schema, and heard.
#[test]
fn custom_header_parameter_reaches_audio() {
    fn render(bias: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval("inst custom-voice pitch-bias: float = 1:\n\tsin-osc {* freq pitch-bias} > * {env-perc 0.001 0.2} > * amp");
        e.eval(&format!(
            "s :custom-voice > freq 220 > pitch-bias {bias} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(e.committed > 0);
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4);
        output
    }
    let low = render(1.0);
    let high = render(2.0);
    let delta: f32 = low.iter().zip(&high).map(|(a, b)| (a - b).abs()).sum();
    assert!(
        delta / low.len() as f32 > 1.0e-3,
        "custom control changes audio"
    );
}

#[test]
fn vact_instrument_routes_independent_main_and_aux() {
    let mut e = E2e::new();
    e.eval("inst two-out:\n\tsin-osc freq > + {aux-out {tri freq}}");
    e.eval("s :two-out > freq 220 > pan 0.5 > once");
    let (main, aux) = e.run_stereo_for(0.8);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&main) && all_finite(&aux));
    assert!(rms(&main) > 1.0e-3 && rms(&aux) > 1.0e-3);
    let delta = main
        .iter()
        .zip(&aux)
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / main.len() as f32;
    assert!(delta > 1.0e-2, "main/aux are distinct: {delta}");

    let mut left = E2e::new();
    left.eval("inst two-out:\n\tsin-osc freq > + {aux-out {tri freq}}");
    left.eval("s :two-out > freq 220 > pan 0 > once");
    let (main_left, aux_left) = left.run_stereo_for(0.8);
    assert!(left.faults.is_empty(), "{:?}", left.faults);
    // Owner decision: main/aux pairs use unity-center balance; pan 0 keeps
    // main at unity and silences aux.
    assert_eq!(main_left, main);
    assert!(rms(&main_left) > 1.0e-3);
    assert!(rms(&aux_left) < 1.0e-5);
}

#[test]
fn three_analytic_percussion_instruments_respond_to_every_control() {
    fn hit(instrument: &str, control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!("s :{instrument} > {control} > once"));
        let output = e.run_for(0.8);
        assert!(
            e.faults.is_empty(),
            "{instrument} {control}: {:?}",
            e.faults
        );
        assert!(e.committed > 0, "{instrument} {control}: committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-4, "{instrument} {control}: audible");
        output
    }
    for (instrument, baseline, variants) in [
        (
            "low-drum",
            "kick-frequency 60",
            &[
                "kick-frequency 110",
                "kick-punch 0.9",
                "kick-tone 0.9",
                "kick-decay 1.0",
            ][..],
        ),
        (
            "wire-drum",
            "snare-frequency 180",
            &[
                "snare-frequency 280",
                "snare-tone 0.9",
                "snare-snappy 0.9",
                "snare-decay 0.8",
            ][..],
        ),
        (
            "metal-hat",
            "hat-frequency 3900",
            &[
                "hat-frequency 6000",
                "hat-tone 0.9",
                "hat-decay 0.5",
                "hat-metal 0.1",
            ][..],
        ),
    ] {
        let base = hit(instrument, baseline);
        for control in variants {
            let changed = hit(instrument, control);
            let difference = base
                .iter()
                .zip(&changed)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / base.len() as f32;
            assert!(
                difference > 1.0e-4,
                "{instrument} {control}: response {difference}"
            );
        }
    }
}

#[test]
fn analytic_percussion_editor_schema_includes_every_header_knob() {
    let e = E2e::new();
    let reg = e.reg.borrow();
    let decls = crate::session::editors::instrument_decls(&reg);
    for (name, params) in [
        (
            "low-drum",
            &["kick-frequency", "kick-punch", "kick-tone", "kick-decay"][..],
        ),
        (
            "wire-drum",
            &[
                "snare-frequency",
                "snare-tone",
                "snare-snappy",
                "snare-decay",
            ][..],
        ),
        (
            "metal-hat",
            &["hat-frequency", "hat-tone", "hat-decay", "hat-metal"][..],
        ),
    ] {
        let decl = decls
            .iter()
            .find(|d| d.name == name)
            .expect("installed editor declaration");
        for param in params {
            assert!(
                decl.params
                    .iter()
                    .any(|p| p.name == *param && p.ctl.is_some()),
                "{name} {param}"
            );
        }
    }
}

#[test]
fn custom_header_rejects_wrong_value_type() {
    let mut e = E2e::new();
    e.eval("inst typed-voice pitch-bias: float = 1:\n\tsin-osc {* freq pitch-bias} > * {env-perc 0.001 0.2} > * amp");
    e.eval("s :typed-voice > pitch-bias true > once");
    e.run_for(1.0);
    assert!(e.faults.iter().any(|f| f.code == FailCode::Type));
    assert_eq!(e.committed, 0);
}

#[test]
fn event_control_capacity_is_an_error() {
    let mut e = E2e::new();
    let header = (0..33)
        .map(|i| format!("p{i}: float = 0"))
        .collect::<Vec<_>>()
        .join(" ");
    e.eval(&format!(
        "inst wide-voice {header}:\n\tsin-osc freq > * {{env-perc 0.001 0.2}} > * amp"
    ));
    let steps = (0..33)
        .map(|i| format!("p{i} 1"))
        .collect::<Vec<_>>()
        .join(" > ");
    e.eval(&format!("s :wide-voice > {steps} > once"));
    e.run_for(1.0);
    assert!(e.faults.iter().any(|f| f.code == FailCode::TooManyControls));
    assert_eq!(e.committed, 0);
}

#[test]
fn undeclared_instrument_control_is_not_silent() {
    let mut e = E2e::new();
    e.eval("inst donor extra-knob: float = 1:\n\tsin-osc freq > * {env-perc 0.001 0.2} > * amp");
    e.eval("inst recipient:\n\tsin-osc freq > * {env-perc 0.001 0.2} > * amp");
    e.eval("s :recipient > extra-knob 2 > once");
    e.run_for(1.0);
    assert!(e
        .faults
        .iter()
        .any(|f| f.code == FailCode::Type && f.message.contains("extra-knob")));
    assert_eq!(e.committed, 0);
}

#[test]
fn declared_type_overrides_global_control_domain() {
    let mut e = E2e::new();
    e.eval("inst typed-cutoff cutoff: int = 100:\n\tsin-osc freq > * {env-perc 0.001 0.2} > * amp");
    e.eval("s :typed-cutoff > cutoff 100.5 > once");
    e.run_for(1.0);
    assert!(e
        .faults
        .iter()
        .any(|f| f.code == FailCode::Type && f.message.contains("cutoff")));
    assert_eq!(e.committed, 0);
}

/// A pattern control reaches every independent FM percussion port through
/// evaluation, scheduling, graph installation, and the audio callback.
#[test]
fn fm_drum_pattern_controls_change_the_render() {
    fn hit(control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!("s :phase-drum > note [:a2] > {control} > once"));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: event committed");
        assert!(all_finite(&output));
        assert!(rms(&output) > 1.0e-3, "{control}: audible");
        output
    }

    let base = hit("freq 110");
    for control in [
        "freq 240",
        "fm-amount 9",
        "pitch-sweep 3",
        "decay 1.2",
        "drum-noise 0.8",
        "drive 0.9",
    ] {
        let changed = hit(control);
        let difference: f32 = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-4, "{control}: response {difference}");
    }
}

/// The three independent percussion components retain their own parameter
/// sets through the entire `.vact` to audio path. The muted components stay
/// installed, so this also exercises the full graph and its control table.
#[test]
fn fusion_drum_component_controls_change_the_render() {
    fn hit(mute: &str, control: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "s :fusion-drum > note [:a4] > velocity 0.5 > fm-index 4 > fm-keytrack 1 > {mute} > {control} > once"
        ));
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
        assert!(e.committed > 0, "{control}: committed");
        assert!(all_finite(&output), "{control}: finite");
        assert!(rms(&output) > 1.0e-5, "{control}: audible");
        output
    }

    for (mute, controls) in [
        (
            "noise-level -90 > fm-level -90",
            &[
                "fb-delay 5",
                "fb-feedback 0.9",
                "fb-decay 1.2",
                "fb-cutoff 800",
                "fb-q 12",
                "fb-velocity 0",
                "fb-level -30",
            ][..],
        ),
        (
            "fb-level -90 > fm-level -90",
            &[
                "noise-attack 0.08",
                "noise-hold 0.2",
                "noise-decay 1.2",
                "noise-cutoff 500",
                "noise-q 12",
                "noise-pitch-env 2400",
                "noise-velocity 0",
                "noise-level -30",
            ][..],
        ),
        (
            "fb-level -90 > noise-level -90",
            &[
                "fm-tuning 220",
                "fm-keytrack -1",
                "fm-ratio 8",
                "fm-index 0",
                "fm-attack 0.08",
                "fm-hold 0.2",
                "fm-decay 0.15",
                "fm-pitch-env 2400",
                "fm-velocity 0",
                "fm-level -30",
            ][..],
        ),
    ] {
        let base = hit(mute, "freq 440");
        for control in controls {
            let changed = hit(mute, control);
            #[allow(clippy::cast_precision_loss)]
            let difference = base
                .iter()
                .zip(&changed)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / base.len() as f32;
            assert!(difference > 1.0e-6, "{control}: response {difference}");
        }
    }
}

#[test]
fn fusion_drum_accepts_every_component_control_on_one_event() {
    let mut e = E2e::new();
    let controls = [
        "velocity 0.7",
        "fb-delay 120",
        "fb-feedback 0.96",
        "fb-decay 0.3",
        "fb-cutoff 4000",
        "fb-q 2",
        "fb-velocity 0.8",
        "fb-level -12",
        "noise-attack 0.01",
        "noise-hold 0.02",
        "noise-decay 0.4",
        "noise-cutoff 3500",
        "noise-q 2",
        "noise-pitch-env 600",
        "noise-velocity 0.8",
        "noise-level -12",
        "fm-tuning 220",
        "fm-keytrack 1",
        "fm-ratio 2",
        "fm-index 4",
        "fm-attack 0.01",
        "fm-hold 0.02",
        "fm-decay 0.4",
        "fm-pitch-env 600",
        "fm-velocity 0.8",
        "fm-level -12",
    ];
    e.eval(&format!(
        "s :fusion-drum > note [:a4] > {} > once",
        controls.join(" > ")
    ));
    let output = e.run_for(1.0);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-4);

    let manifest = HostManifest::spec_default();
    let editor = manifest
        .editor_decl("fusion-drum")
        .expect("fusion-drum editor");
    for control in controls {
        let name = control.split_whitespace().next().expect("control name");
        assert!(
            editor.params.iter().any(|p| p.name == name),
            "{name}: editor parameter"
        );
    }
}

#[test]
fn fusion_drum_note_velocity_and_gain_reach_audio() {
    fn render(source: &str) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(source);
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{source}: {:?}", e.faults);
        assert!(all_finite(&output));
        output
    }
    let base =
        render("s :fusion-drum > note [:a4] > velocity 0.5 > fm-keytrack 1 > fm-index 4 > once");
    let variants = [
        ("velocity", "s :fusion-drum > note [:a4] > velocity 1 > fm-keytrack 1 > fm-index 4 > once"),
        ("gain", "s :fusion-drum > note [:a4] > velocity 0.5 > fm-keytrack 1 > fm-index 4 > gain 0.2 > once"),
        ("note", "s :fusion-drum > note [:a3] > velocity 0.5 > fm-keytrack 1 > fm-index 4 > once"),
    ];
    for (name, source) in variants {
        let changed = render(source);
        #[allow(clippy::cast_precision_loss)]
        let difference = base
            .iter()
            .zip(&changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(difference > 1.0e-5, "{name}: response {difference}");
    }
}

/// `HostManifest::spec_default` declares an `EditorDecl` (design 13.5) for
/// every template, every effect and every ugen the catalog defines, and
/// `template_params` surfaces `cutoff` and `position`.
#[test]
fn every_builtin_has_editor_metadata_through_the_host_manifest() {
    let manifest = HostManifest::spec_default();
    for name in TEMPLATE_NAMES {
        assert!(manifest.editor_decl(name).is_some(), "{name}: editor decl");
    }
    for kind in EffectKind::ALL {
        let name = kind.name();
        assert!(manifest.editor_decl(name).is_some(), "{name}: editor decl");
    }
    for name in UGEN_NAMES {
        assert!(manifest.editor_decl(name).is_some(), "{name}: editor decl");
    }
    let params = manifest.template_params();
    assert!(params.contains(&"cutoff"), "template_params has `cutoff`");
    assert!(
        params.contains(&"position"),
        "template_params has `position`"
    );
}
