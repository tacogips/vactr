//! Prelude-level chip controls, clocking and bounded graph state.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{chip_pair, BuildEnv, BuildError, Template};
use crate::value::intern::name_of_kw;

fn render(control: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :chip-voice > note [:a3] > {control} > once"));
    let output = e.run_for(1.0);
    assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-4, "{control}: audible");
    output
}

#[test]
fn chip_controls_reach_audio_editor_and_dual_output_graph() {
    let base = render("chip-chord 0.4 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "chip-chord 0.9",
        "timbre 0.9",
        "morph 0.9",
        "chip-clocked 1",
        "gain 0.4",
    ] {
        let changed = render(control);
        #[allow(clippy::cast_precision_loss)]
        let delta = base
            .iter()
            .zip(changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / base.len() as f32;
        assert!(delta > 1.0e-6, "{control}: response {delta}");
    }
    let clocked = render("chip-clocked 1 > chip-rate 16");
    let slow = render("chip-clocked 1 > chip-rate 4");
    assert_ne!(clocked, slow, "chip-rate reaches clocked audio");
    let editor = HostManifest::spec_default()
        .editor_decl("chip-voice")
        .expect("chip editor");
    for name in [
        "freq",
        "chip-chord",
        "timbre",
        "morph",
        "chip-clocked",
        "chip-rate",
        "amp",
    ] {
        assert!(
            editor.params.iter().any(|param| param.name == name),
            "{name}"
        );
    }
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"chip-voice")
        .expect("realized chip-voice");
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = Template::from_inst(&entry.def, &env).expect("chip graph");
    assert!(template.has_aux);
    assert_eq!(
        template.mem_total,
        (2 * chip_pair::STATE_FLOATS) + 2 * crate::dsp::ugen::vactrol_gate::GATE_STATE_FLOATS
    );
    let low_budget = BuildEnv {
        voice_mem: (2 * chip_pair::STATE_FLOATS)
            + 2 * crate::dsp::ugen::vactrol_gate::GATE_STATE_FLOATS
            - 1,
        ..env
    };
    assert_eq!(
        Template::from_inst(&entry.def, &low_budget).unwrap_err(),
        BuildError::MemExceeded
    );
}

#[test]
fn chip_scheduled_notes_start_independent_pattern_voices() {
    let mut e = E2e::new();
    e.eval("s :chip-voice > note [:a3 :c4] > chip-clocked 1 > chip-rate 12 > once");
    let output = e.run_for(1.1);
    assert_eq!(e.committed, 2);
    assert!(e.faults.is_empty());
    assert!(rms(&output[48_000..50_400]) > 1.0e-4);
}
