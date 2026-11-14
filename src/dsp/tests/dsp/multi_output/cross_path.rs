use super::{
    bits, browser_rig, decoded_template, default_ctls, fm_graph, load_browser, load_native,
    mono_graph, native_rig, render_windowed, stereo_frames, stereo_graph, va_filter_graph,
};
use crate::dsp::effects::prim::{balance_gains, pan_gains};
use crate::dsp::graph::{InstDef, InstId};
use crate::dsp::tests::dsp::{ctl, event, rms, NativeRig};
use crate::dsp::ugen::Template;
use crate::sched::slots::CtlId;

const RATE: f32 = 48_000.0;
const BLOCK: usize = 128;

fn render_native(
    def: &InstDef,
    frames: Option<&[f32]>,
    controls: &[(CtlId, f32)],
) -> (Vec<f32>, Vec<f32>) {
    let mut rig = native_rig(RATE, BLOCK);
    load_native(&mut rig, def, frames);
    render_windowed(&mut rig, controls, RATE, BLOCK)
}

fn render_browser(
    def: &InstDef,
    frames: Option<&[f32]>,
    controls: &[(CtlId, f32)],
) -> (Vec<f32>, Vec<f32>) {
    let mut rig = browser_rig(RATE, BLOCK);
    load_browser(&mut rig, def, frames);
    render_windowed(&mut rig, controls, RATE, BLOCK)
}

fn render_tier(
    browser: bool,
    def: &InstDef,
    frames: Option<&[f32]>,
    controls: &[(CtlId, f32)],
) -> (Vec<f32>, Vec<f32>) {
    if browser {
        render_browser(def, frames, controls)
    } else {
        render_native(def, frames, controls)
    }
}

fn assert_audible_and_finite(left: &[f32], right: &[f32], check_right: bool) {
    assert!(left.iter().chain(right).all(|sample| sample.is_finite()));
    assert!(rms(left) > 1.0e-5, "left signal is audible");
    if check_right {
        assert!(rms(right) > 1.0e-5, "right signal is audible");
    }
}

fn assert_path_bits_equal(actual: &[f32], expected: &[f32], label: &str) {
    let first_difference = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual.to_bits() != expected.to_bits());
    assert_eq!(actual.len(), expected.len(), "{label}: sample count");
    if let Some(index) = first_difference {
        panic!(
            "{label}: first differing sample {index}: actual={}, expected={}",
            actual[index], expected[index]
        );
    }
}

#[test]
fn native_and_decoded_templates_keep_multi_output_shapes() {
    let defs = [
        (mono_graph(), false, false, "mono"),
        (va_filter_graph(true, 1.0), false, true, "va pair"),
        (stereo_graph(), true, false, "stereo"),
        (fm_graph(true), false, true, "fm pair"),
    ];
    let env = NativeRig::native().engine.build_env();
    for (def, stereo, has_aux, name) in defs {
        let native = Template::from_inst(&def, &env).unwrap();
        let decoded = decoded_template(&def, &env);
        assert_eq!(decoded.nodes(), native.nodes(), "{name}: node shapes");
        assert_eq!(decoded.stereo, native.stereo, "{name}: stereo flag");
        assert_eq!(decoded.has_aux, native.has_aux, "{name}: aux flag");
        assert_eq!(decoded.n_slices, native.n_slices, "{name}: slice count");
        assert_eq!(native.stereo, stereo, "{name}: expected stereo shape");
        assert_eq!(native.has_aux, has_aux, "{name}: expected aux shape");
    }
}

#[test]
fn native_and_browser_multi_output_renders_are_bitwise_equal() {
    let frames = stereo_frames(false, false);
    let defs = [
        (mono_graph(), None, false, "mono"),
        (va_filter_graph(true, 1.0), None, true, "va pair"),
        (stereo_graph(), Some(frames.as_slice()), true, "stereo"),
        (fm_graph(true), None, true, "fm pair"),
    ];
    for (def, samples, check_right, name) in defs {
        let controls = default_ctls();
        let native = render_native(&def, samples, &controls);
        let browser = render_browser(&def, samples, &controls);
        assert_audible_and_finite(&native.0, &native.1, check_right);
        assert_audible_and_finite(&browser.0, &browser.1, check_right);
        assert_eq!(
            bits(&native.0),
            bits(&browser.0),
            "{name}: left tier parity"
        );
        assert_eq!(
            bits(&native.1),
            bits(&browser.1),
            "{name}: right tier parity"
        );
    }
}

#[test]
fn stereo_input_channels_remain_independent_on_both_tiers() {
    for browser in [false, true] {
        for (left_zero, right_zero) in [(false, true), (true, false)] {
            let frames = stereo_frames(left_zero, right_zero);
            let output = render_tier(browser, &stereo_graph(), Some(&frames), &default_ctls());
            let (silent, live) = if right_zero {
                (&output.1, &output.0)
            } else {
                (&output.0, &output.1)
            };
            assert!(
                silent.iter().all(|sample| *sample == 0.0),
                "browser={browser}"
            );
            assert!(rms(live) > 1.0e-5, "browser={browser}: live channel");
        }
    }
}

#[test]
fn main_and_aux_outputs_are_independent_on_both_tiers() {
    for browser in [false, true] {
        let zero = render_tier(browser, &va_filter_graph(true, 0.0), None, &default_ctls());
        let full = render_tier(browser, &va_filter_graph(true, 1.0), None, &default_ctls());
        assert!(
            zero.0.iter().all(|sample| *sample == 0.0),
            "browser={browser}: main muted"
        );
        assert_eq!(
            bits(&zero.1),
            bits(&full.1),
            "browser={browser}: aux unchanged"
        );
        assert!(rms(&full.0) > 1.0e-5 && rms(&full.1) > 1.0e-5);
    }
}

#[test]
fn stereo_and_main_aux_balance_pan_use_unity_center() {
    let frames = stereo_frames(false, false);
    for def in [va_filter_graph(true, 1.0), stereo_graph()] {
        let samples = if def
            .nodes
            .iter()
            .any(|node| matches!(node, crate::dsp::graph::UGenSpec::SamplePlay(_)))
        {
            Some(frames.as_slice())
        } else {
            None
        };
        for pan in [0.0, 0.25, 0.5, 1.0] {
            let mut controls = default_ctls().to_vec();
            controls.push((ctl::PAN, pan));
            let native = render_native(&def, samples, &controls);
            let browser = render_browser(&def, samples, &controls);
            assert_eq!(
                bits(&native.0),
                bits(&browser.0),
                "pan={pan}: left tier parity"
            );
            assert_eq!(
                bits(&native.1),
                bits(&browser.1),
                "pan={pan}: right tier parity"
            );
            let center = if pan == 0.5 {
                native.clone()
            } else {
                let mut center_controls = default_ctls().to_vec();
                center_controls.push((ctl::PAN, 0.5));
                render_native(&def, samples, &center_controls)
            };
            let gains = balance_gains(pan);
            for ((left, right), (center_left, center_right)) in native
                .0
                .iter()
                .zip(&native.1)
                .zip(center.0.iter().zip(&center.1))
            {
                assert_eq!(*left, *center_left * gains.0, "pan={pan}: left balance");
                assert_eq!(*right, *center_right * gains.1, "pan={pan}: right balance");
            }
        }
    }
}

#[test]
fn mono_voice_keeps_equal_power_pan() {
    for pan in [0.0, 0.2] {
        let mut controls = default_ctls().to_vec();
        controls.push((ctl::PAN, pan));
        let native = render_native(&mono_graph(), None, &controls);
        let browser = render_browser(&mono_graph(), None, &controls);
        assert_eq!(
            bits(&native.0),
            bits(&browser.0),
            "pan={pan}: left tier parity"
        );
        assert_eq!(
            bits(&native.1),
            bits(&browser.1),
            "pan={pan}: right tier parity"
        );
        if pan == 0.2 {
            let mut left_controls = default_ctls().to_vec();
            left_controls.push((ctl::PAN, 0.0));
            let left = render_native(&mono_graph(), None, &left_controls).0;
            let gains = pan_gains(pan);
            for (index, (actual_l, actual_r)) in native.0.iter().zip(&native.1).enumerate() {
                assert!((*actual_l - left[index] * gains.0).abs() <= 1.0e-6);
                assert!((*actual_r - left[index] * gains.1).abs() <= 1.0e-6);
            }
        }
    }
}

fn orbit_controls(send: f32) -> [(CtlId, f32); 7] {
    [
        (ctl::FREQ, 220.0),
        (ctl::ATTACK, 0.0),
        (ctl::DECAY, 0.005),
        (ctl::RELEASE, 0.001),
        (CtlId::new(38), send),
        (CtlId::new(39), 0.1),
        (CtlId::new(40), 0.3),
    ]
}

fn render_orbit_native(def: &InstDef, frames: Option<&[f32]>, send: f32) -> (Vec<f32>, Vec<f32>) {
    let mut rig = native_rig(RATE, BLOCK);
    load_native(&mut rig, def, frames);
    let _ = rig.step();
    assert!(rig.engine.template(InstId::new(1)).is_some());
    if frames.is_some() {
        assert!(
            rig.engine.store().get(41).is_some(),
            "native sample installed"
        );
    }
    rig.send(event(1, rig.engine.now(), &orbit_controls(send)));
    let mut output = rig.run(1);
    assert_eq!(rig.engine.active_voices(), 1, "native orbit voice starts");
    let mut tail = rig.run(79);
    output.0.append(&mut tail.0);
    output.1.append(&mut tail.1);
    output
}

fn render_orbit_browser(def: &InstDef, frames: Option<&[f32]>, send: f32) -> (Vec<f32>, Vec<f32>) {
    let mut rig = browser_rig(RATE, BLOCK);
    load_browser(&mut rig, def, frames);
    for _ in 0..6 {
        let sample_ready = frames.is_none() || rig.engine.store().get(41).is_some();
        if rig.engine.template(InstId::new(1)).is_some() && sample_ready {
            break;
        }
        let _ = rig.step();
    }
    assert!(rig.engine.template(InstId::new(1)).is_some());
    if frames.is_some() {
        assert!(
            rig.engine.store().get(41).is_some(),
            "browser sample installed"
        );
    }
    rig.send(event(1, rig.engine.now(), &orbit_controls(send)));
    let mut output = rig.run(1);
    assert_eq!(rig.engine.active_voices(), 1, "browser orbit voice starts");
    let mut tail = rig.run(79);
    output.0.append(&mut tail.0);
    output.1.append(&mut tail.1);
    output
}

#[test]
fn orbit_send_is_cross_tier_and_aux_channel_independent() {
    let frames = stereo_frames(false, false);
    // The sample graph has no envelope node; end dry playback before the
    // 100 ms echo window so that the no-send twin proves the energy is echo.
    let stereo_orbit_frames = &frames[..512];
    for (name, def, samples, check_right) in [
        ("VA pair", va_filter_graph(true, 1.0), None, true),
        ("stereo", stereo_graph(), Some(stereo_orbit_frames), false),
    ] {
        let native = render_orbit_native(&def, samples, 0.8);
        let browser = render_orbit_browser(&def, samples, 0.8);
        assert_path_bits_equal(&native.0, &browser.0, "orbit: left tier parity");
        assert_path_bits_equal(&native.1, &browser.1, "orbit: right tier parity");
        assert!(native
            .0
            .iter()
            .chain(&native.1)
            .all(|sample| sample.is_finite()));
        assert!(
            rms(&native.0[4_800..5_200]) > 1.0e-3,
            "left echo is present"
        );
        let no_send = render_orbit_native(&def, samples, 0.0);
        assert!(
            rms(&no_send.0[4_800..5_200]) < 1.0e-6,
            "{name}: dry voice has ended; rms={}",
            rms(&no_send.0[4_800..5_200])
        );
        if check_right {
            assert!(rms(&native.1[4_800..5_200]) > 1.0e-3, "aux echo is present");
            assert!(
                rms(&no_send.1[4_800..5_200]) < 1.0e-6,
                "{name}: aux dry voice has ended; rms={}",
                rms(&no_send.1[4_800..5_200])
            );
        }
    }
}

#[test]
fn orbit_does_not_cross_feed_stereo_right_zero() {
    for browser in [false, true] {
        let frames = stereo_frames(false, true);
        let output = if browser {
            render_orbit_browser(&stereo_graph(), Some(&frames), 0.8)
        } else {
            render_orbit_native(&stereo_graph(), Some(&frames), 0.8)
        };
        assert!(
            output.1.iter().all(|sample| *sample == 0.0),
            "browser={browser}"
        );
        assert!(
            rms(&output.0) > 1.0e-5,
            "browser={browser}: left remains audible; rms={}, first nonzero sample={:?}",
            rms(&output.0),
            output.0.iter().position(|sample| *sample != 0.0)
        );
    }
}
