//! Prelude-level controls, accent/sustain, retrigger and state budget.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{modal_pair, BuildEnv, BuildError, Template};
use crate::value::intern::name_of_kw;

fn render(control: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :modal-voice > note [:a3] > {control} > once"));
    let output = e.run_for(1.0);
    assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-6, "{control}: audible");
    output
}

#[test]
fn modal_controls_reach_audio_editor_and_dual_output_graph() {
    let base = render("modal-structure 0.5 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "modal-structure 0.9",
        "timbre 0.9",
        "morph 0.9",
        "velocity 0.4",
        "modal-sustain 1",
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
        assert!(delta > 1.0e-7, "{control}: response {delta}");
    }
    let editor = HostManifest::spec_default()
        .editor_decl("modal-voice")
        .expect("modal editor");
    for name in [
        "freq",
        "modal-structure",
        "timbre",
        "morph",
        "velocity",
        "modal-sustain",
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
        .find(|entry| *name_of_kw(entry.name) == *"modal-voice")
        .expect("realized modal-voice");
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = Template::from_inst(&entry.def, &env).expect("modal graph");
    assert!(template.has_aux);
    assert_eq!(template.mem_total, 2 * modal_pair::STATE_FLOATS);
    let low_budget = BuildEnv {
        voice_mem: 2 * modal_pair::STATE_FLOATS - 1,
        ..env
    };
    assert_eq!(
        Template::from_inst(&entry.def, &low_budget).unwrap_err(),
        BuildError::MemExceeded
    );
}

#[test]
fn modal_second_scheduled_hit_retriggers_main() {
    let mut single = E2e::new();
    single.eval("s :modal-voice > note [:a3] > once");
    let a = single.run_for(1.1);
    let mut double = E2e::new();
    double.eval("s :modal-voice > note [:a3 :a3] > once");
    let b = double.run_for(1.1);
    assert_eq!((single.committed, double.committed), (1, 2));
    let window = 48_000..50_400;
    let a_rms = rms(&a[window.clone()]);
    let b_rms = rms(&b[window]);
    assert!(b_rms > a_rms * 1.2, "retrigger {b_rms} vs {a_rms}");
}
