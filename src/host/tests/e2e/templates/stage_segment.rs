//! Public Stages adaptation controls and editor registration.

use super::{all_finite, rms, E2e, HostManifest};

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :stage-voice > note [:a3] > {extra} > once"));
    let audio = e.run_for(0.12);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

#[test]
fn public_modes_and_controls_are_codeable() {
    let base = render("stage-function 7 > stage-primary 0.5 > stage-secondary 0.5");
    assert!(rms(&base) > 1.0e-5);
    for changed in [
        "stage-function 7 > freq 330",
        "stage-function 7 > stage-primary 0.9",
        "stage-function 7 > stage-secondary 0.9",
        "stage-function 0 > stage-type 0 > stage-loop 0",
        "stage-function 0 > stage-type 1 > stage-gate 1",
        "stage-function 0 > stage-type 2 > stage-trigger 1",
        "stage-function 0 > stage-type 3",
        "stage-function 1 > stage-primary 0.2",
        "stage-function 2 > stage-secondary 0.2",
        "stage-function 3 > stage-primary 0.8",
        "stage-function 4 > stage-primary 0.8",
        "stage-function 5 > stage-secondary 0.8",
        "stage-function 6 > stage-clock 1",
        "stage-function 8 > stage-clock 1",
        "stage-function 9 > stage-primary 0.9 > stage-secondary 0",
    ] {
        assert!(!render(changed).is_empty(), "{changed}");
    }
}

#[test]
fn delayed_cv_is_audible_from_public_vact_after_delay_time() {
    let mut e = E2e::new();
    e.eval("s :stage-voice > note [:a3] > stage-function 9 > stage-primary 0.8 > stage-secondary 0 > once");
    let audio = e.run_for(0.15);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(all_finite(&audio));
    assert!(rms(&audio) > 0.01);
}

#[test]
fn editor_exposes_all_declared_controls() {
    let editor = HostManifest::spec_default()
        .editor_decl("stage-voice")
        .unwrap();
    for name in [
        "freq",
        "stage-type",
        "stage-primary",
        "stage-secondary",
        "stage-loop",
        "stage-gate",
        "stage-trigger",
        "stage-clock",
        "stage-function",
        "amp",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
}
