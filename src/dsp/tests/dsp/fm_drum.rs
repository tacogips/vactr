//! Peaks FM drum signal roles, callback partitioning and host codecs.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{InstDef, InstId, UGenSpec};
use crate::dsp::ported::{peaks_functions, CoverageState};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{fm_drum, Inp, Kx, NodeState, RawGraph, Template, MAX_PORTS};

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![UGenSpec::FmDrum].into_boxed_slice(),
        edges: Box::new([]),
        node_params: Box::new([]),
    }
}

fn raw(values: [f32; 6], block: usize, late: usize) -> Vec<f32> {
    let store = SampleStore::default();
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 2400,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 7,
    };
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    for (input, value) in inputs.iter_mut().zip(values) {
        *input = Inp::Val(value);
    }
    let mut state = NodeState::default();
    let mut out = vec![0.0; 2400];
    let mut start = 0;
    let mut next = block - late;
    while start < out.len() {
        let end = (start + next).min(out.len());
        fm_drum::render(&inputs, &mut state, &mut out[start..end], &kx);
        start = end;
        next = block;
    }
    out
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let length = a.len() as f32;
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / length
}

fn dynamic(block: usize, late: usize) -> Vec<f32> {
    const FRAMES: usize = 2400;
    let store = SampleStore::default();
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: FRAMES,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 7,
    };
    let values: [Vec<f32>; 6] = std::array::from_fn(|port| {
        (0..FRAMES)
            .map(|i| match port {
                0 => {
                    if (i / 71) % 2 == 0 {
                        110.0
                    } else {
                        160.0
                    }
                }
                1 => {
                    if (i / 53) % 2 == 0 {
                        1.0
                    } else {
                        9.0
                    }
                }
                2 => {
                    if (i / 47) % 2 == 0 {
                        0.5
                    } else {
                        3.0
                    }
                }
                3 => {
                    if (i / 109) % 2 == 0 {
                        0.3
                    } else {
                        0.6
                    }
                }
                4 => {
                    if (i / 31) % 2 == 0 {
                        0.1
                    } else {
                        0.8
                    }
                }
                _ => {
                    if (i / 83) % 2 == 0 {
                        0.0
                    } else {
                        0.9
                    }
                }
            })
            .collect()
    });
    let mut state = NodeState::default();
    let mut output = vec![0.0; FRAMES];
    let mut start = 0;
    let mut size = block - late;
    while start < FRAMES {
        let end = (start + size).min(FRAMES);
        let mut ins = [Inp::Val(0.0); MAX_PORTS];
        for (port, values) in values.iter().enumerate() {
            ins[port] = Inp::Buf(&values[start..end]);
        }
        fm_drum::render(&ins, &mut state, &mut output[start..end], &kx);
        start = end;
        size = block;
    }
    output
}

#[test]
fn all_independent_controls_reach_single_phase_signal_stages() {
    assert_eq!(peaks_functions()[6].coverage, CoverageState::SourceStage);
    let base = [110.0, 1.5, 0.5, 0.4, 0.0, 0.0];
    let clean = raw(base, 24, 0);
    assert!(clean.iter().all(|v| v.is_finite()));
    assert!(rms(&clean) > 0.001);
    for values in [
        [220.0, 1.5, 0.5, 0.4, 0.0, 0.0],
        [110.0, 9.0, 0.5, 0.4, 0.0, 0.0],
        [110.0, 1.5, 3.0, 0.4, 0.0, 0.0],
        [110.0, 1.5, 0.5, 1.2, 0.0, 0.0],
        [110.0, 1.5, 0.5, 0.4, 1.0, 0.0],
        [110.0, 1.5, 0.5, 0.4, 0.0, 1.0],
    ] {
        let changed = raw(values, 64, 0);
        assert!(changed.iter().all(|v| v.is_finite()));
        assert!(difference(&clean, &changed) > 0.0001, "{values:?}");
    }
    let extreme = raw([96_000.0, 12.0, 4.0, 0.005, 1.0, 1.0], 256, 17);
    assert!(extreme.iter().all(|v| v.is_finite()));
}

#[test]
fn event_local_pitch_update_and_envelopes_ignore_host_partitioning() {
    let values = [110.0, 8.0, 2.0, 0.2, 0.3, 0.7];
    let reference = raw(values, 24, 0);
    for (block, late) in [(64, 0), (256, 0), (64, 17), (256, 17), (37, 11)] {
        assert_eq!(reference, raw(values, block, late), "{block}/{late}");
    }
    assert_eq!(reference, raw(values, 24, 0));
    let dynamic_reference = dynamic(24, 0);
    for (block, late) in [(64, 0), (256, 0), (64, 17), (256, 17), (37, 11)] {
        assert_eq!(
            dynamic_reference,
            dynamic(block, late),
            "dynamic {block}/{late}"
        );
    }
}

#[test]
fn native_and_browser_late_starts_render_finite_without_allocation() {
    let def = voice();
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut decoded = RawGraph::boxed();
    decode_graph(&bytes, &mut decoded, &mut BusTemplate::new()).unwrap();
    let env = NativeRig::native().engine.build_env();
    let template = Template::from_inst(&def, &env).unwrap();
    assert_eq!(template.mem_total, 0);
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut native_config = config(&caps(), StoreKind::NativeArc);
            native_config.sample_rate = rate;
            native_config.max_block = block;
            let mut native = NativeRig::native_with(native_config);
            native.install(&def);
            let _ = native.step();
            native.send(event(
                1,
                native.engine.now() + 17.0 / f64::from(rate),
                &[(ctl::FREQ, 110.0), (ctl::LEGATO, 1.0)],
            ));
            let (left, right) = native.run(20);
            assert!(left[..17].iter().all(|x| x.abs() < 1.0e-7));
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
            assert!(rms(&left) > 0.0001);

            let mut browser_config = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            browser_config.sample_rate = rate;
            browser_config.max_block = block;
            let mut browser = BrowserRig::browser_with(browser_config);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(
                1,
                browser.engine.now() + 17.0 / f64::from(rate),
                &[(ctl::FREQ, 110.0), (ctl::LEGATO, 1.0)],
            ));
            let (left, right) = browser.run(20);
            assert!(left[..17].iter().all(|x| x.abs() < 1.0e-7));
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
            assert!(rms(&left) > 0.0001);
        }
    }
}

#[test]
fn two_scheduled_hits_restart_deterministically() {
    fn sequence(second: bool) -> Vec<f32> {
        let mut cfg = config(&caps(), StoreKind::NativeArc);
        cfg.sample_rate = 48_000.0;
        cfg.max_block = 64;
        let mut rig = NativeRig::native_with(cfg);
        rig.install(&voice());
        let _ = rig.step();
        let start = rig.engine.now() + 17.0 / 48_000.0;
        let controls = &[(ctl::FREQ, 110.0), (ctl::LEGATO, 1.0)];
        rig.send(event(1, start, controls));
        if second {
            rig.send(event(1, start + 0.025, controls));
        }
        rig.run(100).0
    }
    let one = sequence(false);
    let two = sequence(true);
    assert_eq!(two, sequence(true));
    assert!(difference(&one[1600..], &two[1600..]) > 0.0001);
}
