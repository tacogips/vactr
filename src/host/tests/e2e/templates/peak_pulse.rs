//! Public Peaks pulse/ball template and editor controls.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{peak_pulse, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :peak-pulse-voice > note [:a3] > {extra} > once"
    ));
    let audio = e.run_for(0.35);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn all_three_modes_and_alternates_render_from_vact() {
    let shaper = render("pulse-mode 0");
    let random = render("pulse-mode 1");
    let ball = render("pulse-mode 2");
    for signal in [&shaper, &random, &ball] {
        assert!(rms(signal) > 1.0e-5);
    }
    assert!(delta(&shaper, &random) > 1.0e-4);
    assert!(delta(&shaper, &ball) > 1.0e-4);
    for extra in [
        "pulse-mode 0 > pulse-half 1",
        "pulse-mode 0 > pulse-delay 0.8",
        "pulse-mode 0 > pulse-duration 0.8",
        "pulse-mode 0 > pulse-repeats 6",
        "pulse-mode 1 > pulse-half 1",
        "pulse-mode 1 > pulse-accept 0",
        "pulse-mode 1 > pulse-repeat-prob 0.9",
        "pulse-mode 1 > pulse-randomness 0.9",
        "pulse-mode 2 > pulse-half 1",
        "pulse-mode 2 > pulse-gravity 0.9",
        "pulse-mode 2 > pulse-loss 0.1",
        "pulse-mode 2 > pulse-amplitude 0.2",
        "pulse-mode 2 > pulse-velocity 0.8",
        "pulse-mode 0 > pulse-gate 1",
    ] {
        assert!(all_finite(&render(extra)), "{extra}");
    }
    assert!(delta(&shaper, &render("pulse-mode 0 > pulse-duration 0.8")) > 1.0e-4);
    assert!(delta(&random, &render("pulse-mode 1 > pulse-accept 0")) > 1.0e-4);
    assert!(delta(&ball, &render("pulse-mode 2 > pulse-gravity 0.9")) > 1.0e-4);
}

#[test]
fn editor_last_control_and_two_node_memory_budget_are_visible() {
    let editor = HostManifest::spec_default()
        .editor_decl("peak-pulse-voice")
        .unwrap();
    for name in [
        "freq",
        "pulse-mode",
        "pulse-half",
        "pulse-gate",
        "pulse-trigger",
        "pulse-delay",
        "pulse-duration",
        "pulse-interval",
        "pulse-repeats",
        "pulse-accept",
        "pulse-repeat-prob",
        "pulse-randomness",
        "pulse-gravity",
        "pulse-loss",
        "pulse-amplitude",
        "pulse-velocity",
        "amp",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|x| *name_of_kw(x.name) == *"peak-pulse-voice")
        .unwrap();
    let declared = instrument_decls(&registry)
        .into_iter()
        .find(|d| d.name == "peak-pulse-voice")
        .unwrap();
    for name in [
        "pulse-mode",
        "pulse-half",
        "pulse-trigger",
        "pulse-velocity",
    ] {
        assert!(
            declared
                .params
                .iter()
                .any(|p| p.name == name && p.ctl.is_some()),
            "{name}"
        );
    }
    let budget = 2 * peak_pulse::STATE_FLOATS;
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
