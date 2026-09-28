//! Public Peaks motion template controls and editor metadata.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{peak_function, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :peak-motion-voice > note [:a3] > {extra} > once"
    ));
    let audio = e.run_for(0.25);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

#[test]
fn public_modes_and_controls_reach_audio() {
    let env = render("peak-mode 0 > peak-attack 0.1 > peak-decay 0.1");
    assert!(rms(&env) > 1.0e-5);
    for changed in [
        "peak-mode 0 > peak-attack 0.9",
        "peak-mode 0 > peak-decay 0.9",
        "peak-mode 0 > peak-half 1",
        "peak-mode 0 > freq 330",
        "peak-mode 1 > peak-rate 0.1",
        "peak-mode 1 > peak-shape 2",
        "peak-mode 1 > peak-color 0.8",
        "peak-mode 1 > peak-level 0.4",
        "peak-mode 1 > peak-reset-phase 0.5",
        "peak-mode 1 > peak-half 1 > peak-preset 6",
        "peak-mode 2 > peak-shape 4",
        "peak-mode 2 > peak-sync 1",
    ] {
        let signal = render(changed);
        assert!(rms(&signal) > 1.0e-6, "{changed}");
    }
    // Without two tap edges, the tap LFO intentionally runs at its fallback
    // rate. Period measurement is verified in the direct kernel test.
}

#[test]
fn editor_last_control_and_two_node_budget_are_visible() {
    let editor = HostManifest::spec_default()
        .editor_decl("peak-motion-voice")
        .unwrap();
    for name in [
        "freq",
        "peak-attack",
        "peak-decay",
        "peak-sustain",
        "peak-release",
        "peak-rate",
        "peak-shape",
        "peak-color",
        "peak-reset-phase",
        "peak-level",
        "peak-half",
        "peak-gate",
        "peak-trigger",
        "peak-tap",
        "peak-sync",
        "peak-mode",
        "peak-preset",
        "amp",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| *name_of_kw(entry.name) == *"peak-motion-voice")
        .unwrap();
    let declared = instrument_decls(&registry)
        .into_iter()
        .find(|d| d.name == "peak-motion-voice")
        .unwrap();
    for name in ["peak-preset", "peak-tap", "peak-trigger", "peak-sync"] {
        assert!(
            declared
                .params
                .iter()
                .any(|p| p.name == name && p.ctl.is_some()),
            "{name}"
        );
    }
    let budget = 2 * peak_function::STATE_FLOATS;
    for sr in [44_100.0, 48_000.0, 96_000.0] {
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
