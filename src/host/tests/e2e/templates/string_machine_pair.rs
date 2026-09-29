//! Prelude-level position 6 parameter delivery and bounded state.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{string_machine_pair, BuildEnv, BuildError, Template};
use crate::value::intern::name_of_kw;

fn render(control: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :string-machine-voice > note [:a3] > {control} > once"
    ));
    let output = e.run_for(1.0);
    assert!(e.faults.is_empty(), "{control}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-4, "{control}: audible");
    output
}

#[test]
fn string_machine_pair_controls_reach_audio_editor_and_aux_graph() {
    let base = render("machine-chord 0.4 > timbre 0.5 > morph 0.5");
    for control in [
        "freq 330",
        "note [:a4]",
        "machine-chord 0.9",
        "timbre 0.9",
        "morph 0.9",
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
    let editor = HostManifest::spec_default()
        .editor_decl("string-machine-voice")
        .expect("grain pair editor");
    for name in ["freq", "machine-chord", "timbre", "morph", "amp"] {
        assert!(
            editor.params.iter().any(|param| param.name == name),
            "{name}"
        );
    }
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"string-machine-voice")
        .expect("realized string-machine-voice");
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = Template::from_inst(&entry.def, &env).expect("string-machine graph");
    assert!(template.has_aux);
    assert_eq!(template.mem_total, string_machine_pair::STATE_FLOATS);
    let low_budget = BuildEnv {
        voice_mem: string_machine_pair::STATE_FLOATS - 1,
        ..env
    };
    assert_eq!(
        Template::from_inst(&entry.def, &low_budget).unwrap_err(),
        BuildError::MemExceeded
    );
}
