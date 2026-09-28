//! Codeable Rings Part adaptation controls and preallocated template state.

use super::{all_finite, rms, E2e};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{rings_part, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(controls: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :rings-voice > note [:a3] > {controls} > once"));
    let output = e.run_for(0.3);
    assert!(e.faults.is_empty(), "{controls}: {:?}", e.faults);
    assert!(e.committed > 0, "{controls}");
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-6, "{controls}: audible");
    output
}

#[test]
fn all_six_roles_and_exposed_sound_controls_reach_audio() {
    let mut previous: Option<Vec<f32>> = None;
    for model in 0..=5 {
        let controls = format!("rings-model {model} > rings-structure 0.4 > rings-brightness 0.5 > rings-damping 0.3 > rings-position 0.4 > rings-strum 0.7 > rings-internal-exciter 1 > rings-internal-strum 1 > rings-internal-note 1 > rings-tonic 0 > rings-note 4 > rings-fm 0 > rings-chord 3 > rings-polyphony 2");
        let baseline = render(&controls);
        if let Some(prior) = &previous {
            assert_ne!(*prior, baseline, "model {model}");
        }
        previous = Some(baseline);
    }
    let controls = "rings-model 4 > rings-structure 0.4 > rings-brightness 0.5 > rings-damping 0.3 > rings-position 0.4 > rings-strum 0.7 > rings-internal-exciter 1 > rings-internal-strum 1 > rings-internal-note 1 > rings-tonic 0 > rings-note 4 > rings-fm 0 > rings-chord 3 > rings-polyphony 2";
    let baseline = render(controls);
    for changed in [
        "freq 330",
        "rings-structure 0.8",
        "rings-brightness 0.9",
        "rings-damping 0.85",
        "rings-position 0.8",
        "rings-strum 0.2",
        "rings-internal-strum 0",
        "rings-internal-note 0",
        "rings-tonic 5",
        "rings-note 9",
        "rings-fm 5",
        "rings-chord 8",
        "rings-polyphony 4",
        "gain 0.4",
    ] {
        let output = render(&format!("{controls} > {changed}"));
        #[allow(clippy::cast_precision_loss)]
        let delta = baseline
            .iter()
            .zip(output)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / baseline.len() as f32;
        assert!(delta > 1.0e-7, "{changed}: {delta}");
    }
    let mut muted = E2e::new();
    muted.eval(&format!(
        "s :rings-voice > note [:a3] > {controls} > rings-internal-exciter 0 > once"
    ));
    let silence = muted.run_for(0.3);
    assert!(rms(&silence) < rms(&baseline) * 0.01);
}

#[test]
fn editor_exposes_all_controls_and_memory_is_preflighted() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"rings-voice")
        .unwrap();
    let editor = instrument_decls(&registry)
        .into_iter()
        .find(|decl| decl.name == "rings-voice")
        .unwrap();
    for name in [
        "rings-model",
        "rings-structure",
        "rings-brightness",
        "rings-damping",
        "rings-position",
        "rings-strum",
        "rings-internal-exciter",
        "rings-internal-strum",
        "rings-internal-note",
        "rings-tonic",
        "rings-note",
        "rings-fm",
        "rings-chord",
        "rings-polyphony",
    ] {
        assert!(
            editor
                .params
                .iter()
                .any(|p| p.name == name && p.ctl.is_some()),
            "{name}"
        );
    }
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let template = Template::from_inst(&entry.def, &env).unwrap();
    assert_eq!(template.mem_total, 2 * rings_part::mem_len(48_000.0));
    assert!(template.has_aux);
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        let required = 2 * rings_part::mem_len(rate);
        let supported = BuildEnv {
            sr: rate,
            voice_mem: required,
            ..env
        };
        assert_eq!(
            Template::from_inst(&entry.def, &supported)
                .unwrap()
                .mem_total,
            required
        );
        assert_eq!(
            Template::from_inst(
                &entry.def,
                &BuildEnv {
                    voice_mem: required - 1,
                    ..supported
                }
            )
            .unwrap_err(),
            BuildError::MemExceeded
        );
    }
}

#[test]
fn successive_events_reset_deterministic_excitation() {
    let controls = "rings-model 2 > rings-strum 0.8 > rings-polyphony 1";
    let first = render(controls);
    assert_eq!(first, render(controls));
    let mut one = E2e::new();
    one.eval(&format!("s :rings-voice > note [:a3] > {controls} > once"));
    let one_out = one.run_for(1.1);
    let mut two = E2e::new();
    two.eval(&format!(
        "s :rings-voice > note [:a3 :a3] > {controls} > once"
    ));
    let two_out = two.run_for(1.1);
    assert!(one.faults.is_empty() && two.faults.is_empty());
    assert_eq!((one.committed, two.committed), (1, 2));
    let window = 48_000..50_400;
    assert!(rms(&two_out[window.clone()]) > rms(&one_out[window]) * 1.1);
}
