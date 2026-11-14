//! Live stereo master-bus input remains bounded and separate from direct stems.

use super::{bus_def, caps, config, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_bus, FaultCode, StoreKind};
use crate::dsp::effects::catalog::spec;
use crate::dsp::graph::EffectKind;
use crate::dsp::ring::encode_graph_record;
use crate::host::wire::Ctl;

fn render(rate: f32, block: usize, browser: bool, quad: bool, wet: bool) -> [Vec<f32>; 4] {
    let mut cfg = config(
        &caps(),
        if browser {
            StoreKind::Arena { bytes: 4 << 20 }
        } else {
            StoreKind::NativeArc
        },
    );
    cfg.sample_rate = rate;
    cfg.max_block = block;
    cfg.output_channels = if quad { 4 } else { 2 };
    let master = bus_def(
        0,
        vec![spec(
            EffectKind::ResonantBank,
            &[
                ("model", Ctl::Const(2.0)),
                ("internal-exciter", Ctl::Const(0.0)),
                ("external-mix", Ctl::Const(1.0)),
                ("mix", Ctl::Const(1.0)),
            ],
        )
        .unwrap()],
    );
    let input = [0.2, 0.7].repeat(block);
    let mut lanes: [Vec<f32>; 4] = std::array::from_fn(|_| Vec::new());
    if browser {
        let mut rig = BrowserRig::browser_with(cfg);
        if wet {
            let mut bytes = Vec::new();
            encode_bus(&master, true, &mut bytes).unwrap();
            let mut record = Vec::new();
            encode_graph_record(20, 1, &bytes, &mut record);
            rig.push(&record);
            let _ = rig.run(6);
        }
        for _ in 0..8 {
            for frame in rig
                .step_with_input(&input)
                .chunks_exact(if quad { 4 } else { 2 })
            {
                for (lane, &sample) in lanes.iter_mut().zip(frame) {
                    lane.push(sample);
                }
            }
        }
    } else {
        let mut rig = NativeRig::native_with(cfg);
        if wet {
            rig.install_bus(&master, true);
            let _ = rig.step();
        }
        for _ in 0..8 {
            for frame in rig
                .step_with_input(&input)
                .chunks_exact(if quad { 4 } else { 2 })
            {
                for (lane, &sample) in lanes.iter_mut().zip(frame) {
                    lane.push(sample);
                }
            }
        }
    }
    lanes
}

#[test]
fn dry_stereo_input_keeps_lanes_and_quad_stems_isolated_at_all_rates() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            for browser in [false, true] {
                for quad in [false, true] {
                    let lanes = render(rate, block, browser, quad, false);
                    assert!(lanes[0].iter().all(|x| (*x - 0.2).abs() < 1.0e-6));
                    assert!(lanes[1].iter().all(|x| (*x - 0.7).abs() < 1.0e-6));
                    assert!(lanes[2].iter().chain(&lanes[3]).all(|x| *x == 0.0));
                }
            }
        }
    }
}

#[test]
fn external_input_drives_source_sensitive_master_effect_on_both_tiers() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            for browser in [false, true] {
                let dry = render(rate, block, browser, false, false);
                let wet = render(rate, block, browser, false, true);
                let delta: f32 = dry[0].iter().zip(&wet[0]).map(|(a, b)| (a - b).abs()).sum();
                assert!(
                    delta > 0.01,
                    "effect response {rate}/{block}, browser={browser}: {delta}"
                );
                assert!(rms(&wet[0]) + rms(&wet[1]) > 1.0e-5);
                assert!(wet[0].iter().chain(&wet[1]).all(|x| x.is_finite()));
            }
        }
    }
}

#[test]
fn output_only_and_malformed_input_are_silent_with_diagnostic() {
    let mut rig = NativeRig::native();
    let block = rig.engine.config().max_block;
    assert!(rig.step().iter().all(|x| *x == 0.0));
    assert!(rig
        .step_with_input(&[0.5, 0.5].repeat(block))
        .iter()
        .any(|x| *x != 0.0));
    assert!(rig.step().iter().all(|x| *x == 0.0));
    for invalid in [
        vec![0.1; 2 * block - 1],
        vec![f32::NAN; 2 * block],
        vec![f32::INFINITY; 2 * block],
    ] {
        assert!(rig.step_with_input(&invalid).iter().all(|x| *x == 0.0));
        assert_eq!(
            rig.engine.pop_fault().map(|f| f.code),
            Some(FaultCode::InputBuffer)
        );
    }
}
