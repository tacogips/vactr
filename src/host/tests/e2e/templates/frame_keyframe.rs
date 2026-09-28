//! Checked immutable Frames keyframe payload through public `.vact` and host paths.

use super::{all_finite, rms, E2e, HostManifest};

fn render(extra: &str) -> (Vec<f32>, E2e) {
    let mut e = E2e::new();
    e.eval(&format!(
        "s :frame-keyframe-voice > note [:a3] > {extra} > once"
    ));
    let audio = e.run_for(0.04);
    (audio, e)
}

#[test]
fn authoring_editor_and_separate_lanes() {
    let (a, e) = render("frame-position 0.25 > frame-main-channel 0 > frame-aux-channel 1");
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(all_finite(&a));
    assert!(rms(&a) > 0.001);
    let (b, e) = render("frame-position 0.75 > frame-main-channel 3 > frame-aux-channel 2");
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!((rms(&a) - rms(&b)).abs() > 0.005);
    let editor = HostManifest::spec_default()
        .editor_decl("frame-keyframe-voice")
        .unwrap();
    for name in [
        "frame-position",
        "frame-ease1",
        "frame-ease2",
        "frame-ease3",
        "frame-ease4",
        "frame-response1",
        "frame-response2",
        "frame-response3",
        "frame-response4",
        "frame-main-channel",
        "frame-aux-channel",
    ] {
        assert!(editor.params.iter().any(|p| p.name == name), "{name}");
    }
}

#[test]
fn flat_list_authoring_checks_64th_and_rejects_bad_65th_or_order() {
    let mut e = E2e::new();
    let list = (0..64)
        .map(|i| format!("{} 0.2 0.4 0.6 0.8", i as f32 / 63.0))
        .collect::<Vec<_>>()
        .join(" ");
    let source = |name: &str, rows: &str| {
        format!(
        "inst {name} frames: [float] = [{rows}]:\n\tframe-keyframe-core frames: frames frame-position: 0.5 frame-channel: 0"
    )
    };
    e.eval(&source("sixty-four-frames", &list));
    for bad in [
        format!("{list} 1 0.2 0.4 0.6 0.8"),
        String::from("0.5 0 0 0 0 0.5 1 1 1 1"),
        String::from("0.8 0 0 0 0 0.2 1 1 1 1"),
        String::from("0 0 0 0 0 1 1 1 1 2"),
    ] {
        let outcomes =
            e.ev.eval_str(&source("bad-keyframes", &bad), super::super::SOURCE)
                .unwrap();
        assert!(outcomes.iter().any(|o| o.value.is_err()), "{bad}");
    }
}

#[test]
fn every_lane_easing_and_response_control_changes_audio() {
    for lane in 0..4 {
        let base =
            format!("frame-position 0.3 > frame-main-channel {lane} > frame-aux-channel {lane}");
        let (reference, _) = render(&base);
        let ease = format!("{base} > frame-ease{} 2", lane + 1);
        let (shaped, _) = render(&ease);
        assert!((rms(&reference) - rms(&shaped)).abs() > 0.001, "{ease}");
        let response = format!("{base} > frame-response{} 1", lane + 1);
        let (curved, _) = render(&response);
        assert!((rms(&reference) - rms(&curved)).abs() > 0.001, "{response}");
    }
}

#[test]
fn opt_in_quad_vact_examples_compile_to_four_output_graphs() {
    use crate::dsp::caps::CapabilitySet;
    use crate::dsp::ugen::{BuildEnv, Template};
    use crate::value::intern::name_of_kw;
    let mut e = E2e::new();
    let forms =
        e.ev.eval_str(
            include_str!("../../../../../examples/quad-stems.vact"),
            super::super::SOURCE,
        )
        .unwrap();
    assert!(forms.iter().all(|f| f.value.is_ok()), "{forms:?}");
    let registry = e.reg.borrow();
    for name in ["frame-keyframe-quad-voice", "tides2-quad-voice"] {
        let entry = registry
            .entries()
            .find(|entry| &*name_of_kw(entry.name) == name)
            .unwrap();
        let t = Template::from_inst(
            &entry.def,
            &BuildEnv {
                sr: 48_000.0,
                caps: CapabilitySet::native(),
                voice_mem: 48_000,
            },
        )
        .unwrap();
        assert!(t.has_aux && t.has_quad, "{name}");
    }
}
