//! Separate string-choir template, controls, FX and fixed memory contract.

use super::{all_finite, rms, E2e};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{string_choir, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(controls: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :string-choir-voice > note [:a3] > {controls} > once"
    ));
    let output = e.run_for(0.35);
    assert!(e.faults.is_empty(), "{controls}: {:?}", e.faults);
    assert!(e.committed > 0, "{controls}");
    assert!(all_finite(&output));
    assert!(rms(&output) > 1.0e-6, "{controls}: audible");
    output
}

#[test]
fn six_fx_roles_and_every_declared_control_change_audio() {
    let settings = "choir-structure 0.4 > choir-brightness 0.5 > choir-damping 0.6 > choir-position 0.5 > choir-strum 0.7 > choir-internal-exciter 0 > choir-internal-strum 1 > choir-internal-note 1 > choir-tonic 0 > choir-note 4 > choir-fm 0 > choir-chord 3 > choir-polyphony 2";
    let mut modes = Vec::new();
    for fx in 0..=5 {
        let output = render(&format!("{settings} > choir-fx {fx}"));
        for earlier in &modes {
            assert_ne!(earlier, &output, "FX {fx} aliases an earlier role");
        }
        modes.push(output);
    }
    let mut chords = Vec::new();
    for chord in 0..=10 {
        let output = render(&format!("{settings} > choir-fx 4 > choir-chord {chord}"));
        for earlier in &chords {
            assert_ne!(earlier, &output, "chord {chord} aliases an earlier choice");
        }
        chords.push(output);
    }
    let baseline = render(&format!("{settings} > choir-fx 4"));
    for control in [
        "freq 330",
        "choir-structure 0.9",
        "choir-brightness 0.9",
        "choir-damping 0.1",
        "choir-position 0.9",
        "choir-strum 0.3",
        "choir-internal-exciter 1",
        "choir-internal-strum 0",
        "choir-internal-note 0",
        "choir-tonic 5",
        "choir-note 9",
        "choir-fm 5",
        "choir-chord 8",
        "choir-polyphony 4",
        "choir-fx 0",
        "gain 0.4",
    ] {
        let output = render(&format!("{settings} > choir-fx 4 > {control}"));
        #[allow(clippy::cast_precision_loss)]
        let delta = baseline
            .iter()
            .zip(output)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / baseline.len() as f32;
        assert!(delta > 1.0e-7, "{control}: {delta}");
    }
}

#[test]
fn editor_exposes_controls_and_each_rate_preflights_fixed_state() {
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"string-choir-voice")
        .unwrap();
    let editor = instrument_decls(&registry)
        .into_iter()
        .find(|decl| decl.name == "string-choir-voice")
        .unwrap();
    for name in [
        "choir-structure",
        "choir-brightness",
        "choir-damping",
        "choir-position",
        "choir-strum",
        "choir-internal-exciter",
        "choir-internal-strum",
        "choir-internal-note",
        "choir-tonic",
        "choir-note",
        "choir-fm",
        "choir-chord",
        "choir-polyphony",
        "choir-fx",
    ] {
        assert!(
            editor
                .params
                .iter()
                .any(|p| p.name == name && p.ctl.is_some()),
            "{name}"
        );
    }
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let total = 2 * string_choir::mem_len(sr);
        let env = BuildEnv {
            sr,
            caps: CapabilitySet::native(),
            voice_mem: total,
        };
        let template = Template::from_inst(&entry.def, &env).unwrap();
        assert!(template.has_aux);
        assert_eq!(template.mem_total, total);
        assert_eq!(
            Template::from_inst(
                &entry.def,
                &BuildEnv {
                    voice_mem: total - 1,
                    ..env
                }
            )
            .unwrap_err(),
            BuildError::MemExceeded
        );
    }
}
