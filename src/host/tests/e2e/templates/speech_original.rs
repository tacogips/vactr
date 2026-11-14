//! Prelude controls and event behavior for original speech models.

use super::{all_finite, rms, E2e};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{speech_original, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(controls: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :speech-voice > note [:a3] > {controls} > once"));
    let output = e.run_for(0.9);
    assert!(e.faults.is_empty(), "{controls}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-4, "{controls}: audible");
    output
}

#[test]
fn speech_controls_reach_audio() {
    let baseline =
        render("speech-harmonics 0.2 > timbre 0.45 > morph 0.4 > velocity 0.7 > speech-sustain 0");
    for control in [
        "freq 330",
        "note [:a4]",
        "speech-harmonics 0.8",
        "timbre 0.9",
        "morph 0.9",
        "velocity 0.2",
        "speech-sustain 1",
        "gain 0.3",
    ] {
        let changed = render(&format!(
                "speech-harmonics 0.2 > timbre 0.45 > morph 0.4 > velocity 0.7 > speech-sustain 0 > {control}"
            ));
        #[allow(clippy::cast_precision_loss)]
        let delta = baseline
            .iter()
            .zip(changed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / baseline.len() as f32;
        assert!(delta > 1.0e-5, "{control}: {delta}");
    }
}

#[test]
fn editor_lists_controls_and_install_rejects_insufficient_memory() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    for bank in ["speech-voice"] {
        let entry = registry
            .entries()
            .find(|entry| *name_of_kw(entry.name) == *bank)
            .expect("realized bank template");
        let editor = instrument_decls(&registry)
            .into_iter()
            .find(|decl| decl.name == bank)
            .expect("bank editor metadata");
        for name in [
            "speech-harmonics",
            "timbre",
            "morph",
            "velocity",
            "speech-sustain",
        ] {
            assert!(
                editor
                    .params
                    .iter()
                    .any(|p| p.name == name && p.ctl.is_some()),
                "{bank}: {name}"
            );
        }
        let env = BuildEnv {
            sr: 48_000.0,
            caps: CapabilitySet::native(),
            voice_mem: 24_000,
        };
        let template = Template::from_inst(&entry.def, &env).expect("voice graph");
        assert_eq!(template.mem_total, 2 * speech_original::STATE_FLOATS);
        assert!(template.has_aux);
        let low = BuildEnv {
            voice_mem: 2 * speech_original::STATE_FLOATS - 1,
            ..env
        };
        assert_eq!(
            Template::from_inst(&entry.def, &low).unwrap_err(),
            BuildError::MemExceeded
        );
    }
}

#[test]
fn successive_notes_retrigger_speech_envelopes() {
    let mut single = E2e::new();
    single.eval("s :speech-voice > note [:a3] > morph 0.15 > once");
    let one = single.run_for(1.1);
    let mut double = E2e::new();
    double.eval("s :speech-voice > note [:a3 :a3] > morph 0.15 > once");
    let two = double.run_for(1.1);
    assert!(single.faults.is_empty() && double.faults.is_empty());
    assert_eq!((single.committed, double.committed), (1, 2));
    let window = 48_000..50_400;
    let one_rms = rms(&one[window.clone()]);
    let two_rms = rms(&two[window]);
    assert!(two_rms > one_rms * 1.2, "retrigger {two_rms} vs {one_rms}");
}
