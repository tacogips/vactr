//! Prelude-level Tides1 control routing and editor schema.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{tidal_function, BuildEnv, BuildError, Template};
use crate::value::intern::name_of_kw;

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :tidal-voice > note [:a3] > {extra} > once"));
    let audio = e.run_for(0.3);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

#[test]
fn modes_ranges_and_shape_controls_reach_audio() {
    let base = render("tide-mode 1 > tide-range 0");
    assert!(rms(&base) > 1.0e-5);
    for control in [
        "freq 330",
        "tide-pitch 12",
        "tide-shape 0.9",
        "tide-slope 0.9",
        "tide-smoothness 0.9",
        "tide-ratio 2",
        "tide-mode 0",
        "tide-mode 2",
        "tide-range 1",
        "tide-range 2",
        "gain 0.4",
    ] {
        let changed = render(control);
        let delta: f32 = base.iter().zip(&changed).map(|(a, b)| (a - b).abs()).sum();
        assert!(delta > 1.0e-5, "{control}: {delta}");
    }
}

#[test]
fn clock_gate_freeze_flags_and_last_selector_are_codeable_and_editor_visible() {
    let editor = HostManifest::spec_default()
        .editor_decl("tidal-voice")
        .unwrap();
    for name in [
        "freq",
        "tide-shape",
        "tide-slope",
        "tide-smoothness",
        "tide-ratio",
        "tide-sync",
        "tide-gate",
        "tide-clock",
        "tide-freeze",
        "tide-mode",
        "tide-range",
        "tide-pitch",
        "tide-output",
        "amp",
    ] {
        assert!(
            editor.params.iter().any(|param| param.name == name),
            "{name}"
        );
    }
    for selector in [0, 1, 2, 3] {
        let audio = render(&format!(
            "tide-mode 1 > tide-output {selector} > tide-sync 1 > tide-clock 1"
        ));
        assert!(all_finite(&audio));
    }
    for control in ["tide-gate 0", "tide-freeze 1"] {
        assert!(all_finite(&render(control)));
    }
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"tidal-voice")
        .unwrap();
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let budget = 2 * tidal_function::STATE_FLOATS;
        let env = BuildEnv {
            sr,
            caps: CapabilitySet::native(),
            voice_mem: budget,
        };
        let template = Template::from_inst(&entry.def, &env).unwrap();
        assert_eq!(template.mem_total, budget);
        assert!(template.has_aux);
        assert_eq!(
            Template::from_inst(
                &entry.def,
                &BuildEnv {
                    voice_mem: budget - 1,
                    ..env
                }
            )
            .unwrap_err(),
            BuildError::MemExceeded
        );
    }
}
