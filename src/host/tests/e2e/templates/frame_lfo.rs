//! Public Frames poly-LFO controls and two selectable host output lanes.

use super::{all_finite, rms, E2e, HostManifest};

fn render(extra: &str) -> Vec<f32> {
    let mut e = E2e::new();
    e.eval(&format!("s :frame-lfo-voice > note [:a3] > {extra} > once"));
    let audio = e.run_for(0.08);
    assert!(e.faults.is_empty(), "{extra}: {:?}", e.faults);
    assert!(e.committed > 0);
    assert!(all_finite(&audio));
    audio
}

#[test]
fn public_controls_and_editor_ranges_are_available() {
    let base =
        render("frame-shape 0 > frame-spread 0.75 > frame-main-channel 0 > frame-aux-channel 1");
    assert!(rms(&base) > 0.01);
    for changed in [
        "freq 330",
        "frame-shape 0.9",
        "frame-spread 0.1",
        "frame-shape-spread 0.9",
        "frame-coupling 0.9",
        "frame-offset 0.3",
        "frame-main-channel 3",
        "frame-aux-channel 2",
    ] {
        let got = render(changed);
        assert!(rms(&got) > 0.001, "{changed}");
    }
    let editor = HostManifest::spec_default()
        .editor_decl("frame-lfo-voice")
        .unwrap();
    for name in [
        "freq",
        "frame-shape",
        "frame-spread",
        "frame-shape-spread",
        "frame-coupling",
        "frame-offset",
        "frame-main-channel",
        "frame-aux-channel",
        "amp",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
    for name in ["frame-main-channel", "frame-aux-channel"] {
        let p = editor.params.iter().find(|p| p.name == name).unwrap();
        assert_eq!(p.range, (0.0, 3.0));
    }
}
