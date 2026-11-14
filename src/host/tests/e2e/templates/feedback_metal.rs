//! End-to-end `.vact` routing for the original coupled-FM percussion voice.

use super::{all_finite, rms, E2e, HostManifest};

fn hit(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :feedback-metal-drum > note [:f3] > velocity 0.7 > metal-index 4 > metal-noise-level 0.25 > {extra} > once"
    ));
    let out = e.run_for(0.7);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0, "{extra}: event committed");
    assert!(all_finite(&out), "{extra}: finite");
    assert!(rms(&out) > 1.0e-4, "{extra}: audible");
    out
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32
}

#[test]
fn every_declared_control_reaches_audio_and_editor() {
    let base = hit("metal-ratio 2.4");
    let changes = [
        ("freq", "freq 350"),
        ("velocity", "velocity 0.25"),
        ("metal-ratio", "metal-ratio 5.3"),
        ("metal-index", "metal-index 8"),
        ("metal-feedback", "metal-feedback 0.85"),
        ("metal-mod-decay", "metal-mod-decay 0.7"),
        ("metal-body-decay", "metal-body-decay 1.2"),
        ("metal-pitch-drop", "metal-pitch-drop 2.5"),
        ("metal-noise-level", "metal-noise-level 0.8"),
        ("metal-noise-decay", "metal-noise-decay 0.5"),
        ("metal-noise-color", "metal-noise-color 400"),
        ("metal-noise-to-fm", "metal-noise-to-fm 2.0"),
        ("metal-cutoff", "metal-cutoff 350"),
        ("metal-resonance", "metal-resonance 8"),
        ("metal-drive", "metal-drive 0.9"),
        ("amp", "gain 0.2"),
    ];
    let editor = HostManifest::spec_default()
        .editor_decl("feedback-metal-drum")
        .expect("editor metadata");
    for (name, extra) in changes {
        assert!(
            editor.params.iter().any(|p| p.name == name),
            "{name}: editor"
        );
        let variant = hit(extra);
        let delta = difference(&base, &variant);
        assert!(delta > 1.0e-5, "{name}: response {delta}");
    }
}
