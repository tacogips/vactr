//! Public original number-station voice controls and metadata.

use super::{all_finite, rms, E2e, HostManifest};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{number_station, BuildEnv, BuildError, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :number-station-voice > note [:a3] > {extra} > once"
    ));
    let audio = e.run_for(0.3);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

fn delta(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn tone_voice_digit_and_public_controls_reach_audio() {
    let tone = render("station-voice 0");
    let speech = render("station-voice 1 > station-digit 3");
    assert!(rms(&tone) > 1.0e-5 && rms(&speech) > 1.0e-5);
    assert!(delta(&tone, &speech) > 1.0e-4);
    for (base, changed) in [
        ("station-voice 0", "station-voice 0 > station-tone 0.9"),
        ("station-voice 0", "station-voice 0 > station-noise 0"),
        ("station-voice 0", "station-voice 0 > station-drive 0.9"),
        (
            "station-voice 1 > station-digit 3",
            "station-voice 1 > station-digit 8",
        ),
        (
            "station-voice 1 > station-digit 3",
            "station-voice 1 > station-half 1",
        ),
        (
            "station-voice 1 > station-digit 3",
            "station-voice 1 > station-auto 1 > station-transition 1",
        ),
    ] {
        assert!(delta(&render(base), &render(changed)) > 1.0e-5, "{changed}");
    }
    assert!(rms(&render("station-gate 0 > station-trigger 0")) < 1.0e-6);
}

#[test]
fn editor_controls_and_exact_budget_are_visible() {
    let editor = HostManifest::spec_default()
        .editor_decl("number-station-voice")
        .unwrap();
    for name in [
        "freq",
        "station-voice",
        "station-half",
        "station-tone",
        "station-transition",
        "station-noise",
        "station-drive",
        "station-digit",
        "station-auto",
        "station-gate",
        "station-trigger",
        "amp",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
    let e = E2e::new();
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|x| *name_of_kw(x.name) == *"number-station-voice")
        .unwrap();
    let declared = instrument_decls(&registry)
        .into_iter()
        .find(|d| d.name == "number-station-voice")
        .unwrap();
    for name in [
        "station-digit",
        "station-transition",
        "station-gate",
        "station-trigger",
    ] {
        assert!(
            declared
                .params
                .iter()
                .any(|p| p.name == name && p.ctl.is_some()),
            "{name}"
        );
    }
    let budget = 2 * number_station::STATE_FLOATS;
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
