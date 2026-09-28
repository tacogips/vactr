//! Prelude-level Tides2 selector/control routing and editor metadata.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{tidal_poly, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :tides2-voice > note [:a3] > {extra} > once"));
    let audio = e.run_for(0.3);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

#[test]
fn all_modes_and_sound_controls_are_codeable_and_responsive() {
    let base = render("poly-mode 1 > poly-output-mode 2 > poly-range 1");
    assert!(rms(&base) > 1.0e-5);
    for control in [
        "freq 330",
        "poly-width 0.9",
        "poly-shape 0.9",
        "poly-smoothness 0.9",
        "poly-mode 0",
        "poly-mode 2",
        "poly-output-mode 0",
        "poly-output-mode 1",
        "poly-output-mode 3",
        "poly-range 0",
        "gain 0.4",
    ] {
        let changed = render(control);
        let delta: f32 = base.iter().zip(&changed).map(|(a, b)| (a - b).abs()).sum();
        assert!(delta > 1.0e-5, "{control}: {delta}");
    }
    let shifted_base = render("poly-main-channel 2 > poly-shift 0.5");
    let shifted = render("poly-main-channel 2 > poly-shift 0.8");
    assert_ne!(
        shifted_base, shifted,
        "shift changes the selected phase lane"
    );
    let lane_zero = render("poly-shift 0.8 > poly-main-channel 0");
    let lane_three = render("poly-shift 0.8 > poly-main-channel 3");
    assert_ne!(lane_zero, lane_three, "main selector reaches audio");
}

#[test]
fn both_channel_selectors_and_gate_clock_reach_editor_and_graph() {
    let editor = HostManifest::spec_default()
        .editor_decl("tides2-voice")
        .unwrap();
    for name in [
        "freq",
        "poly-width",
        "poly-shape",
        "poly-smoothness",
        "poly-shift",
        "poly-gate",
        "poly-clock",
        "poly-mode",
        "poly-output-mode",
        "poly-range",
        "poly-main-channel",
        "poly-aux-channel",
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
        .find(|entry| *name_of_kw(entry.name) == *"tides2-voice")
        .unwrap();
    let declared = instrument_decls(&registry)
        .into_iter()
        .find(|decl| decl.name == "tides2-voice")
        .unwrap();
    for name in [
        "poly-main-channel",
        "poly-aux-channel",
        "poly-gate",
        "poly-clock",
    ] {
        assert!(
            declared
                .params
                .iter()
                .any(|param| param.name == name && param.ctl.is_some()),
            "{name}"
        );
    }
    for extra in ["poly-gate 0", "poly-clock 1", "poly-aux-channel 3"] {
        assert!(all_finite(&render(extra)));
    }
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let budget = 2 * tidal_poly::STATE_FLOATS;
        let env = BuildEnv {
            sr,
            caps: CapabilitySet::native(),
            voice_mem: budget,
        };
        let built = Template::from_inst(&entry.def, &env).unwrap();
        assert_eq!(built.mem_total, budget);
        assert!(built.has_aux);
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
