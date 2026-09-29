//! `bus`/`master` chains and routing end to end (design 12.5, 12.8.6;
//! TASK-008 criterion 4), through the real `NativeAudioHost` ring.
//!
//! Routing (which bus a voice's output sums into) is serial repair R3:
//! `src/host/caps.rs`'s `InstResolver::bus`, `src/ns/insts.rs`'s
//! implementation of it over `InstRegistry`'s declared buses, and
//! `src/sched/commit.rs::audio_events` resolving the `bus` control (a
//! `CtlRoute::Scheduler` row, otherwise skipped) into the installed bus
//! id. A chain unit's parameters actually taking effect (`gain`,
//! `threshold`, ...) is serial repair R6: `src/dsp/build.rs` now keys
//! every effect parameter (bus/master chain units and inst-body effects
//! alike) with `effects::param_ctl` (the effect-local `EFFECT_PARAM_BASE +
//! index` space `effects::param_index`/`FxUnit::configure` read), never
//! the control-table/`EXTRA_CTL_BASE` ids `param_id` gives out — those
//! silently failed `param_index`'s lookup, so no chain parameter ever
//! left its built-in default, confirmed before R6 by a routed and an
//! unrouted voice rendering bit-identical peaks.

use super::{rms, E2e};

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |m, v| m.max(v.abs()))
}

/// `10^(db / 20)`: the linear amplitude ratio of a dB gain.
fn db_ratio(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[test]
fn resonant_bank_all_controls_compile_from_vact_and_last_mix_changes_audio() {
    let mut wet = E2e::new();
    wet.eval("bus :rings:\n\tresonant-bank model: 2 structure: 0.7 brightness: 0.8 damping: 0.4 position: 0.6 note: 2 tonic: 1 fm: 0 chord: 3 polyphony: 3 strum: 0 internal-exciter: 0 internal-strum: 1 internal-note: 1 external-mix: 1 gate: 1 mix: 1");
    wet.eval("s :analog > note [:c4] > bus :rings > once");
    let wet_audio = wet.run_for(0.25);
    assert!(wet.faults.is_empty());
    assert!(rms(&wet_audio) > 1.0e-5);

    let mut dry = E2e::new();
    dry.eval("bus :rings:\n\tresonant-bank model: 2 structure: 0.7 brightness: 0.8 damping: 0.4 position: 0.6 note: 2 tonic: 1 fm: 0 chord: 3 polyphony: 3 strum: 0 internal-exciter: 0 internal-strum: 1 internal-note: 1 external-mix: 1 gate: 1 mix: 0");
    dry.eval("s :analog > note [:c4] > bus :rings > once");
    let dry_audio = dry.run_for(0.25);
    assert!(dry.faults.is_empty());
    let difference: f32 = wet_audio
        .iter()
        .zip(&dry_audio)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(
        difference > 0.1,
        "17th effect parameter reaches the render path"
    );
}

#[test]
fn stream_control_effects_and_all_parameters_are_codeable() {
    let controls = "shape: 0.2 response: 0.8 global-attack: 0.3 global-decay: 0.6 alternate: 1 linked: 1 excite-source: 0 excite: 0.3 trigger: 0 gate: 1 threshold: 0.1 cutoff-min: 100 cutoff-max: 9000";
    for name in ["stream-envelope", "stream-vactr"] {
        let editor = crate::types::manifest::HostManifest::spec_default()
            .editor_decl(name)
            .unwrap();
        for param in crate::dsp::effects::dynamic_control::PARAMS {
            assert!(
                editor.params.iter().any(|p| p.name == param.name),
                "{name}/{}",
                param.name
            );
        }
        let mut wet = E2e::new();
        wet.eval(&format!("bus :stream:\n\t{name} {controls} mix: 1"));
        wet.eval("s :analog > note [:c4] > bus :stream > once");
        let wet_audio = wet.run_for(0.3);
        assert!(wet.faults.is_empty(), "{name}: {:?}", wet.faults);
        assert!(rms(&wet_audio) > 1.0e-6, "{name}");
        let mut dry = E2e::new();
        dry.eval(&format!("bus :stream:\n\t{name} {controls} mix: 0"));
        dry.eval("s :analog > note [:c4] > bus :stream > once");
        let dry_audio = dry.run_for(0.3);
        assert!(dry.faults.is_empty());
        let delta: f32 = wet_audio
            .iter()
            .zip(&dry_audio)
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(delta > 0.1, "{name} last mix control reaches audio");
    }
}

#[test]
fn keyframe_mixer_is_public_vact_code_and_reports_unknown_controls() {
    let control_text =
        "rate: 4 shape: 0.3 spread: 0.8 shape-spread: 0.7 coupling: 0.2 offset: 0.15 mix: 1";
    let editor = crate::types::manifest::HostManifest::spec_default()
        .editor_decl("keyframe-mixer")
        .unwrap();
    let kind = crate::dsp::graph::EffectKind::KeyframeMixer;
    for param in crate::dsp::effects::params(kind) {
        let meta = editor
            .params
            .iter()
            .find(|meta| meta.name == param.name)
            .unwrap();
        assert_eq!(meta.default, param.default, "{} default", param.name);
        assert_eq!(meta.label, crate::dsp::meta::label_of(param.name));
    }
    assert_eq!(editor.params.len(), crate::dsp::effects::params(kind).len());

    let mut wet = E2e::new();
    wet.eval(&format!("bus :quad:\n\tkeyframe-mixer {control_text}"));
    let parsed_bus = wet
        .reg
        .borrow()
        .buses()
        .find_map(|(_, entry)| {
            entry
                .def
                .chain
                .iter()
                .any(|effect| effect.kind == kind)
                .then(|| (*entry.def).clone())
        })
        .expect("the source-defined bus is registered");
    wet.eval("s :analog > note [:c4] > bus :quad > once");
    let wet_audio = wet.run_for(0.3);
    assert!(wet.faults.is_empty(), "{:?}", wet.faults);
    assert!(rms(&wet_audio) > 1.0e-6);

    let browser_audio = crate::dsp::tests::dsp::quad_mixer::render_browser_bus(&parsed_bus);
    assert!(browser_audio
        .0
        .iter()
        .chain(&browser_audio.1)
        .all(|sample| sample.is_finite()));
    assert!(rms(&browser_audio.0) + rms(&browser_audio.1) > 1.0e-6);

    let mut dry = E2e::new();
    dry.eval("bus :quad:\n\tkeyframe-mixer rate: 4 shape: 0.3 spread: 0.8 shape-spread: 0.7 coupling: 0.2 offset: 0.15 mix: 0");
    dry.eval("s :analog > note [:c4] > bus :quad > once");
    let dry_audio = dry.run_for(0.3);
    assert!(dry.faults.is_empty(), "{:?}", dry.faults);
    let difference: f32 = wet_audio
        .iter()
        .zip(&dry_audio)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(difference > 0.1, "mix controls conventional wet/dry blend");

    let mut unknown = E2e::new();
    assert!(unknown
        .ev
        .eval_str(
            "bus :bad:\n\tkeyframe-mixer rate: 4 mystery: 0.3",
            crate::host::tests::e2e::SOURCE
        )
        .unwrap()
        .iter()
        .any(|outcome| outcome
            .value
            .as_ref()
            .err()
            .is_some_and(|failure| { failure.message.contains("unknown parameter `mystery:`") })));
}

#[test]
fn stream_follower_and_compressor_controls_compile_and_mix_reaches_audio() {
    for (name, controls) in [
        ("stream-follower", "shape: 0.2 response: 0.8 global-attack: 0.3 global-decay: 0.6 alternate: 1 linked: 1 excite-source: 0 excite: 0.3 cutoff-min: 100 cutoff-max: 9000"),
        ("stream-compressor", "threshold: 0.4 amount: 0.7 global-attack: 0.3 global-decay: 0.6 global-threshold: 0.4 global-amount: 0.7 alternate: 1 linked: 1 excite-source: 0 excite: 0.3"),
    ] {
        let editor = crate::types::manifest::HostManifest::spec_default()
            .editor_decl(name)
            .unwrap();
        let defs = crate::dsp::effects::params(
            crate::dsp::graph::EffectKind::from_name(name).unwrap(),
        );
        for param in defs {
            assert!(editor.params.iter().any(|p| p.name == param.name), "{name}/{}", param.name);
        }
        let mut wet = E2e::new();
        wet.eval(&format!("bus :stream:\n\t{name} {controls} mix: 1"));
        wet.eval("s :analog > note [:c4] > bus :stream > once");
        let wet_audio = wet.run_for(0.3);
        assert!(wet.faults.is_empty(), "{name}: {:?}", wet.faults);
        assert!(rms(&wet_audio) > 1.0e-6, "{name}");
        let mut dry = E2e::new();
        dry.eval(&format!("bus :stream:\n\t{name} {controls} mix: 0"));
        dry.eval("s :analog > note [:c4] > bus :stream > once");
        let dry_audio = dry.run_for(0.3);
        assert!(dry.faults.is_empty());
        let delta: f32 = wet_audio.iter().zip(&dry_audio).map(|(a, b)| (a - b).abs()).sum();
        assert!(delta > 0.1, "{name} mix reaches audio");
    }
}

#[test]
fn final_streams_filter_and_lorenz_controls_compile_from_vact() {
    for (name, controls) in [
        (
            "stream-filter",
            "offset: 0.6 amount: 0.8 excite-source: 0 excite: 0.2 cutoff-min: 120 cutoff-max: 9000",
        ),
        (
            "stream-lorenz",
            "rate: 0.7 balance: 0.3 excite-source: 0 excite: 0.2 cutoff-min: 120 cutoff-max: 9000",
        ),
    ] {
        let editor = crate::types::manifest::HostManifest::spec_default()
            .editor_decl(name)
            .unwrap();
        let defs =
            crate::dsp::effects::params(crate::dsp::graph::EffectKind::from_name(name).unwrap());
        for param in defs {
            assert!(
                editor.params.iter().any(|p| p.name == param.name),
                "{name}/{}",
                param.name
            );
        }
        assert!(!editor
            .params
            .iter()
            .any(|p| p.name == "alternate" || p.name == "linked"));
        let mut wet = E2e::new();
        wet.eval(&format!("bus :stream:\n\t{name} {controls} mix: 1"));
        wet.eval("s :analog > note [:c4] > bus :stream > once");
        let wet_audio = wet.run_for(0.3);
        assert!(wet.faults.is_empty(), "{name}: {:?}", wet.faults);
        assert!(rms(&wet_audio) > 1.0e-6);
        let mut dry = E2e::new();
        dry.eval(&format!("bus :stream:\n\t{name} {controls} mix: 0"));
        dry.eval("s :analog > note [:c4] > bus :stream > once");
        let dry_audio = dry.run_for(0.3);
        assert!(dry.faults.is_empty());
        let delta: f32 = wet_audio
            .iter()
            .zip(&dry_audio)
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(delta > 0.1, "{name} mix reaches audio");
    }
}

#[test]
fn elements_bank_patch_and_performance_controls_compile_from_vact() {
    let controls = "ex-env-shape: 0.4 ex-bow-level: 0.2 ex-bow-timbre: 0.6 ex-blow-level: 0.7 ex-blow-meta: 0.4 ex-blow-timbre: 0.8 ex-strike-level: 0.6 ex-strike-meta: 0.3 ex-strike-timbre: 0.7 ex-signature: 0.4 ex-geometry: 0.6 ex-brightness: 0.8 ex-damping: 0.4 ex-position: 0.6 ex-res-mod-frequency: 0.3 ex-res-mod-offset: 0.7 ex-reverb-diffusion: 0.5 ex-reverb-lp: 0.6 ex-space: 0.4 ex-modulation-frequency: 0.5 ex-gate: 1 ex-note: 2 ex-modulation: 0.2 ex-strength: 0.8 ex-model: 0 external-blend: 1";
    let mut wet = E2e::new();
    wet.eval(&format!("bus :elements:\n\texciter-bank {controls} mix: 1"));
    wet.eval("s :analog > note [:c4] > bus :elements > once");
    let wet_audio = wet.run_for(0.25);
    assert!(wet.faults.is_empty());
    assert!(rms(&wet_audio) > 1.0e-6);
    let mut dry = E2e::new();
    dry.eval(&format!("bus :elements:\n\texciter-bank {controls} mix: 0"));
    dry.eval("s :analog > note [:c4] > bus :elements > once");
    let dry_audio = dry.run_for(0.25);
    assert!(dry.faults.is_empty());
    let delta: f32 = wet_audio
        .iter()
        .zip(&dry_audio)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(delta > 0.1, "final mix control reaches audio");
}

#[test]
fn elements_alternate_and_space_freeze_compile_from_vact() {
    let mut e = E2e::new();
    e.eval("bus :exciter-alt:\n\texciter-bank ex-alternate: 1 ex-space: 1.9 ex-bow-level: 0.4 external-blend: 0 mix: 1");
    e.eval("s :analog > note [:c4] > bus :exciter-alt > once");
    let audio = e.run_for(0.25);
    assert!(e.faults.is_empty());
    assert!(rms(&audio) > 1.0e-6);
}

#[test]
fn a_bus_definition_compiles_and_installs() {
    let mut e = E2e::new();
    // `eval` already asserts no drain fault by itself (it panics on one);
    // `e.faults` is filled only by ticking (`run_for`/`render_committed`),
    // so it is checked below, once a voice actually renders through the
    // bus, to catch a real runtime fault too, not just a compile one.
    e.eval("bus :drums:\n\tgain -18");
    let id = e
        .reg
        .borrow()
        .bus(crate::value::intern::intern_kw("drums"))
        .map(|b| b.id);
    assert!(id.is_some(), "registered under its name");

    e.eval("s :analog > note [:c4] > bus :drums > once");
    e.run_for(1.0);
    assert!(
        e.faults.is_empty(),
        "no faults once routed and rendered: {:?}",
        e.faults
    );
}

#[test]
fn a_routed_voice_is_measurably_quieter_than_unrouted() {
    let mut unrouted = E2e::new();
    unrouted.eval("s :analog > note [:c4] > once");
    let unrouted_peak = peak(&unrouted.run_for(1.0));

    let mut routed = E2e::new();
    routed.eval("bus :drums:\n\tgain -18");
    routed.eval("s :analog > note [:c4] > bus :drums > once");
    let routed_peak = peak(&routed.run_for(1.0));

    let ratio = routed_peak / unrouted_peak;
    let want = db_ratio(-18.0); // ~0.126
    assert!(
        (ratio - want).abs() < want * 0.3,
        "routed/unrouted peak ratio {ratio} vs -18 dB ({want})"
    );
}

#[test]
fn master_lowers_both_a_routed_and_an_unrouted_slot() {
    let mut baseline = E2e::new();
    baseline.eval("s :analog > note [:c4] > once");
    let base_peak = peak(&baseline.run_for(1.0));

    let mut unrouted = E2e::new();
    unrouted.eval("master:\n\tgain -12");
    unrouted.eval("s :analog > note [:c4] > once");
    let unrouted_peak = peak(&unrouted.run_for(1.0));

    let mut routed = E2e::new();
    routed.eval("master:\n\tgain -12");
    routed.eval("bus :drums:\n\tgain -6");
    routed.eval("s :analog > note [:c4] > bus :drums > once");
    let routed_peak = peak(&routed.run_for(1.0));

    let want_unrouted = db_ratio(-12.0); // ~0.251, master only
    let got_unrouted = unrouted_peak / base_peak;
    assert!(
        (got_unrouted - want_unrouted).abs() < want_unrouted * 0.3,
        "master -12 dB, unrouted: {got_unrouted} vs {want_unrouted}"
    );

    let want_routed = db_ratio(-12.0) * db_ratio(-6.0); // ~0.126, bus + master
    let got_routed = routed_peak / base_peak;
    assert!(
        (got_routed - want_routed).abs() < want_routed * 0.3,
        "bus -6 dB + master -12 dB, routed: {got_routed} vs {want_routed}"
    );
}

#[test]
fn a_bus_redefinition_swaps_to_the_new_gain_with_zero_allocation_and_no_dropout() {
    let mut e = E2e::new();
    e.eval("bus :drums:\n\tgain -18");
    // `legato 1` holds the ADSR gate open for the whole step, and a long
    // release keeps the tail well past the next retrigger: the routed
    // voice stays continuously audible, so a gap can only be the swap.
    e.eval("s :analog > note [:c4] > legato 1 > release 5 > bus :drums > d1");
    let before = e.run_for(1.0);
    let before_peak = peak(&before);
    assert!(before_peak > 1.0e-3, "sounds before the swap");

    // Redefines the chain while the slot keeps playing: the generation +
    // refcount lifecycle (design 12.5) retires the old chain once nothing
    // routes to it, without a gap in the routed voice's output. `run_for`
    // already asserts zero allocation on every render.
    e.eval("bus :drums:\n\tgain -6");
    let during = e.run_for(2.0);
    assert!(
        during.iter().all(|v| v.is_finite()),
        "finite across the swap"
    );
    // No silent gap: every 5 ms block (240 frames) has some signal.
    for block in during.chunks(240) {
        assert!(peak(block) > 0.0, "no dropout: a silent block");
    }
    assert_eq!(
        e.faults.len(),
        0,
        "no faults across the swap: {:?}",
        e.faults
    );

    // -18 dB -> -6 dB is a real level change: about 4x louder (12 dB).
    let after_peak = peak(&e.run_for(1.0));
    let ratio = after_peak / before_peak;
    assert!(
        ratio > db_ratio(12.0) * 0.7,
        "the new -6 dB chain is louder than the old -18 dB one: ratio {ratio}"
    );
}

#[test]
fn room_maps_onto_the_bus_unit_parameter() {
    let mut zero = E2e::new();
    zero.eval("s :analog > note [:c4] > room 0 > once");
    let zero_out = zero.run_for(1.0);

    let mut plain = E2e::new();
    plain.eval("s :analog > note [:c4] > once");
    let plain_out = plain.run_for(1.0);

    // `room 0` is transparent: close to the same signal as no `room` at
    // all (both default to 0 anyway, but this exercises the control path).
    let zero_peak = peak(&zero_out);
    let plain_peak = peak(&plain_out);
    assert!(
        (zero_peak - plain_peak).abs() < plain_peak * 0.2,
        "room 0 is transparent: {zero_peak} vs {plain_peak}"
    );

    let mut wet = E2e::new();
    wet.eval("s :analog > note [:c4] > room 0.3 > once");
    let wet_out = wet.run_for(1.0);
    assert!(wet_out.iter().all(|v| v.is_finite()));

    // `room 0.3` audibly changes the signal (the reverb tail extends the
    // energy well past where the dry voice's own envelope has decayed).
    let tail_energy = |s: &[f32]| -> f32 { s[s.len() - 4800..].iter().map(|v| v * v).sum() };
    assert!(
        tail_energy(&wet_out) > tail_energy(&zero_out) * 2.0,
        "room 0.3 leaves an audible tail: {} vs room 0's {}",
        tail_energy(&wet_out),
        tail_energy(&zero_out)
    );
}

#[test]
fn dual_mod_bus_is_exposed_in_vact() {
    fn render(algorithm: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!("bus :mod:\n\tdual-mod algorithm: {algorithm} timbre: 0.7 drive: 0.6 carrier-wave: 2 carrier-frequency: 330"));
        e.eval("s :phase-drum > pan 0.3 > bus :mod > once");
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(e.committed > 0);
        assert!(output.iter().all(|x| x.is_finite()));
        output
    }
    let a = render(0.0);
    let b = render(3.0);
    let vocoder = render(6.0);
    let frozen = render(8.0);
    let delta = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
    assert!(
        delta > 1.0e-4,
        "algorithm control changes routed audio: {delta}"
    );
    let upper_delta = vocoder
        .iter()
        .zip(&frozen)
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / vocoder.len() as f32;
    assert!(
        upper_delta > 1.0e-4,
        "algorithm 8 reaches the frozen vocoder path: {upper_delta}"
    );
}

#[test]
fn dual_mod_channel_drives_are_codeable_in_vact() {
    fn render(carrier: f32, modulator: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "bus :mod:\n\tdual-mod algorithm: 0 timbre: 0.5 drive: 1 carrier-wave: 0 carrier-frequency: 330 carrier-drive: {carrier} modulator-drive: {modulator}"
        ));
        e.eval("s :phase-drum > pan 0.3 > bus :mod > once");
        let output = e.run_for(1.0);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(e.committed > 0);
        assert!(output.iter().all(|x| x.is_finite()));
        output
    }
    let both = render(1.0, 1.0);
    let carrier_off = render(0.0, 1.0);
    let modulator_off = render(1.0, 0.0);
    let delta = |a: &[f32], b: &[f32]| {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
    };
    assert!(delta(&both, &carrier_off) > 1.0e-4);
    assert!(delta(&both, &modulator_off) > 1.0e-4);
}

#[test]
fn dual_mod_source_carrier_pairs_are_codeable_in_vact() {
    fn render(algorithm: f32, wave: u8) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "bus :mod:\n\tdual-mod algorithm: {algorithm} timbre: 0.5 drive: 0.8 carrier-wave: {wave} carrier-frequency: 330 carrier-drive: 1 modulator-drive: 1"
        ));
        e.eval("s :phase-drum > pan 0.3 > bus :mod > once");
        let output = e.run_for(0.5);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(e.committed > 0);
        assert!(output.iter().all(|x| x.is_finite()));
        output
    }
    for wave in 1..=3 {
        let xmod = render(0.0, wave);
        let vocoder = render(7.0, wave);
        let delta = xmod
            .iter()
            .zip(&vocoder)
            .map(|(x, y)| (x - y).abs())
            .sum::<f32>()
            / xmod.len() as f32;
        assert!(delta > 1.0e-4, "wave {wave} selects distinct source roles");
    }
}

#[test]
fn shift_pair_bus_controls_and_editor_metadata_are_codeable() {
    let name = "shift-pair";
    let decl = crate::types::manifest::HostManifest::spec_default()
        .editor_decl(name)
        .unwrap();
    for param in crate::dsp::effects::shift_pair::PARAMS {
        assert!(
            decl.params.iter().any(|p| p.name == param.name),
            "{}",
            param.name
        );
    }
    let render = |shift: f32, mix: f32| {
        let mut e = E2e::new();
        e.eval(&format!("bus :shift:\n\tshift-pair carrier-wave: 1 shift-pot: {shift} shift-cv: 0 phase-shift: 0 timbre: 1 feedback: 0.2 dry-wet: 1 mix: {mix}"));
        e.eval("s :analog > note [:c4] > pan 0.3 > bus :shift > once");
        let out = e.run_for(0.5);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(rms(&out) > 1.0e-6);
        out
    };
    let dry = render(0.5, 0.0);
    let centered = render(0.5, 1.0);
    let shifted = render(0.6, 1.0);
    let delta = |a: &[f32], b: &[f32]| {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
    };
    assert!(delta(&dry, &centered) > 1.0e-4);
    assert!(delta(&centered, &shifted) > 1.0e-4);
}

#[test]
fn texture_grain_bus_controls_reach_stereo_audio_from_vact() {
    fn render(position: f32, mix: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "bus :texture:\n\ttexture-grain position: {position} size: 0.3 pitch: 0 density: 0.9 texture: 0.7 stereo-spread: 0.5 feedback: 0.1 reverb: 0.2 mix: {mix} freeze: 0 trigger: 0 gate: 1"
        ));
        e.eval("s :analog > note [:c4] > legato 1 > bus :texture > once");
        let audio = e.run_for(0.7);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(audio.iter().all(|x| x.is_finite()));
        audio
    }
    let dry = render(0.5, 0.0);
    let wet = render(0.5, 1.0);
    let moved = render(0.05, 1.0);
    let difference = |a: &[f32], b: &[f32]| {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
    };
    assert!(difference(&dry, &wet) > 1.0e-5, "mix affects output");
    assert!(difference(&wet, &moved) > 1.0e-6, "position affects output");
}

#[test]
fn texture_stretch_bus_controls_reach_audio_from_vact() {
    fn render(position: f32, mix: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "bus :stretch:\n\ttexture-stretch position: {position} size: 0.4 pitch: 0 density: 0.8 texture: 0.7 stereo-spread: 0.5 feedback: 0.1 reverb: 0.2 mix: {mix} freeze: 0 trigger: 0 gate: 1"
        ));
        e.eval("s :analog > note [:c4] > legato 1 > bus :stretch > once");
        let audio = e.run_for(0.7);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(audio.iter().all(|x| x.is_finite()));
        audio
    }
    let dry = render(0.5, 0.0);
    let wet = render(0.5, 1.0);
    let moved = render(0.05, 1.0);
    let difference = |a: &[f32], b: &[f32]| {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
    };
    assert!(difference(&dry, &wet) > 1.0e-5, "mix affects output");
    assert!(difference(&wet, &moved) > 1.0e-6, "position affects output");
}

#[test]
fn texture_loop_bus_controls_reach_audio_from_vact() {
    fn render(position: f32, mix: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "bus :loop:\n\ttexture-loop position: {position} size: 0.4 pitch: 0 density: 0.8 texture: 0.7 stereo-spread: 0.5 feedback: 0.1 reverb: 0.2 mix: {mix} freeze: 0 trigger: 0 gate: 1"
        ));
        e.eval("s :analog > note [:c4] > legato 1 > bus :loop > once");
        let audio = e.run_for(0.7);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(audio.iter().all(|x| x.is_finite()));
        audio
    }
    let dry = render(0.5, 0.0);
    let wet = render(0.5, 1.0);
    let moved = render(0.05, 1.0);
    let difference = |a: &[f32], b: &[f32]| {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
    };
    assert!(difference(&dry, &wet) > 1.0e-5, "mix affects output");
    assert!(difference(&wet, &moved) > 1.0e-6, "position affects output");
}

#[test]
fn texture_spectral_bus_controls_reach_audio_from_vact() {
    fn render(position: f32, mix: f32) -> Vec<f32> {
        let mut e = E2e::new();
        e.eval(&format!(
            "bus :spectral:\n\ttexture-spectral position: {position} size: 0.4 pitch: 0 density: 0.8 texture: 0.7 stereo-spread: 0.5 feedback: 0.1 reverb: 0.2 mix: {mix} freeze: 0 trigger: 0 gate: 1"
        ));
        e.eval("s :analog > note [:c4] > legato 1 > bus :spectral > once");
        let audio = e.run_for(0.7);
        assert!(e.faults.is_empty(), "{:?}", e.faults);
        assert!(audio.iter().all(|x| x.is_finite()));
        audio
    }
    let dry = render(0.5, 0.0);
    let wet = render(0.5, 1.0);
    let moved = render(0.05, 1.0);
    let difference = |a: &[f32], b: &[f32]| {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
    };
    assert!(difference(&dry, &wet) > 1.0e-5, "mix affects output");
    assert!(difference(&wet, &moved) > 1.0e-6, "position affects output");
}

#[test]
fn all_texture_quality_values_are_codeable_on_every_bus_mode() {
    for effect in [
        "texture-grain",
        "texture-stretch",
        "texture-loop",
        "texture-spectral",
    ] {
        for quality in 0..4 {
            let mut e = E2e::new();
            e.eval(&format!(
                "bus :texture-quality:\n\t{effect} position: 0.5 size: 0.4 pitch: 0 density: 0.8 texture: 0.7 stereo-spread: 0.5 feedback: 0.1 reverb: 0.2 mix: 1 freeze: 0 trigger: 0 gate: 1 quality: {quality}"
            ));
            e.eval("s :analog > note [:c4] > legato 1 > bus :texture-quality > once");
            let audio = e.run_for(0.35);
            assert!(
                e.faults.is_empty(),
                "{effect} quality {quality}: {:?}",
                e.faults
            );
            assert!(audio.iter().all(|x| x.is_finite()));
            assert!(rms(&audio) > 1.0e-6, "{effect} quality {quality}: audible");
        }
    }
}
