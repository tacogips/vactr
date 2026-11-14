//! Prelude controls and event behavior for original six-operator FM banks.

use super::{all_finite, rms, E2e};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{six_op_original, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

const BANKS: [&str; 3] = ["six-bank-a-voice", "six-bank-b-voice", "six-bank-c-voice"];

fn render(bank: &str, controls: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :{bank} > note [:a3] > {controls} > once"));
    let output = e.run_for(0.9);
    assert!(e.faults.is_empty(), "{bank}, {controls}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-4, "{bank}, {controls}: audible");
    output
}

#[test]
fn all_bank_controls_reach_audio() {
    let mut previous: Option<Vec<f32>> = None;
    for bank in BANKS {
        let baseline = render(
            bank,
            "six-patch 0.2 > timbre 0.45 > morph 0.4 > velocity 0.7 > six-sustain 0",
        );
        if let Some(prior) = &previous {
            assert_ne!(prior, &baseline, "distinct bank {bank}");
        }
        previous = Some(baseline.clone());
        for control in [
            "freq 330",
            "note [:a4]",
            "six-patch 0.8",
            "timbre 0.9",
            "morph 0.9",
            "velocity 0.2",
            "six-sustain 1",
            "gain 0.3",
        ] {
            let changed = render(
                bank,
                &format!(
                "six-patch 0.2 > timbre 0.45 > morph 0.4 > velocity 0.7 > six-sustain 0 > {control}"
            ),
            );
            #[allow(clippy::cast_precision_loss)]
            let delta = baseline
                .iter()
                .zip(changed)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / baseline.len() as f32;
            assert!(delta > 1.0e-5, "{bank}, {control}: {delta}");
        }
    }
}

#[test]
fn editor_lists_controls_and_install_rejects_insufficient_memory() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    for bank in BANKS {
        let entry = registry
            .entries()
            .find(|entry| *name_of_kw(entry.name) == *bank)
            .expect("realized bank template");
        let editor = instrument_decls(&registry)
            .into_iter()
            .find(|decl| decl.name == bank)
            .expect("bank editor metadata");
        for name in ["six-patch", "timbre", "morph", "velocity", "six-sustain"] {
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
        assert_eq!(
            template.mem_total,
            (2 * six_op_original::STATE_FLOATS)
                + 2 * crate::dsp::ugen::vactrol_gate::GATE_STATE_FLOATS
        );
        assert!(template.has_aux);
        let low = BuildEnv {
            voice_mem: (2 * six_op_original::STATE_FLOATS)
                + 2 * crate::dsp::ugen::vactrol_gate::GATE_STATE_FLOATS
                - 1,
            ..env
        };
        assert_eq!(
            Template::from_inst(&entry.def, &low).unwrap_err(),
            BuildError::MemExceeded
        );
    }
}

#[test]
fn successive_notes_retrigger_six_operator_envelopes() {
    let mut single = E2e::new();
    single.eval("s :six-bank-a-voice > note [:a3] > morph 0.15 > once");
    let one = single.run_for(1.1);
    let mut double = E2e::new();
    double.eval("s :six-bank-a-voice > note [:a3 :a3] > morph 0.15 > once");
    let two = double.run_for(1.1);
    assert!(single.faults.is_empty() && double.faults.is_empty());
    assert_eq!((single.committed, double.committed), (1, 2));
    let window = 48_000..50_400;
    let one_rms = rms(&one[window.clone()]);
    let two_rms = rms(&two[window]);
    assert!(two_rms > one_rms * 1.2, "retrigger {two_rms} vs {one_rms}");
}
