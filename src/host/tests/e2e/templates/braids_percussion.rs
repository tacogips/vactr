//! Prelude routing for Braids kick/cymbal/snare positions 34–36.

use super::{all_finite, rms, E2e};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{braids_percussion, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(controls: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :macro-percussion-voice > note [:a3] > {controls} > once"
    ));
    let output = e.run_for(0.7);
    assert!(e.faults.is_empty(), "{controls}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-4, "{controls}: audible");
    output
}

#[test]
fn three_shapes_and_each_sound_control_reach_audio() {
    let mut previous: Option<Vec<f32>> = None;
    for shape in 34..=36 {
        let baseline = render(&format!(
            "macro-shape {shape} > macro-color 0.4 > macro-timbre 0.7 > macro-strike 0.4 > macro-sync 0.3"
        ));
        if let Some(prior) = &previous {
            assert_ne!(prior, &baseline, "selector position {shape}");
        }
        previous = Some(baseline.clone());
        for control in [
            "freq 330",
            "macro-color 0.85",
            "macro-timbre 0.25",
            "macro-strike 0.9",
            "macro-sync 0.8",
            "gain 0.4",
        ] {
            let changed = render(&format!(
                "macro-shape {shape} > macro-color 0.4 > macro-timbre 0.7 > macro-strike 0.4 > macro-sync 0.3 > {control}"
            ));
            #[allow(clippy::cast_precision_loss)]
            let delta = baseline
                .iter()
                .zip(changed)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / baseline.len() as f32;
            assert!(delta > 1.0e-6, "shape {shape}, {control}: {delta}");
        }
    }
}

#[test]
fn editor_has_custom_ids_and_install_rejects_low_memory() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"macro-percussion-voice")
        .expect("realized template");
    let editor = instrument_decls(&registry)
        .into_iter()
        .find(|decl| decl.name == "macro-percussion-voice")
        .expect("dynamic custom-header editor");
    for name in [
        "macro-shape",
        "macro-color",
        "macro-timbre",
        "macro-strike",
        "macro-sync",
    ] {
        assert!(
            editor
                .params
                .iter()
                .any(|param| param.name == name && param.ctl.is_some()),
            "{name}"
        );
    }
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = Template::from_inst(&entry.def, &env).expect("voice graph");
    assert_eq!(template.mem_total, braids_percussion::STATE_FLOATS);
    let low = BuildEnv {
        voice_mem: braids_percussion::STATE_FLOATS - 1,
        ..env
    };
    assert_eq!(
        Template::from_inst(&entry.def, &low).unwrap_err(),
        BuildError::MemExceeded
    );
}
