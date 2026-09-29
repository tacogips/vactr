//! Clocked-noise reset, independent filters and native/browser codec.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, clock_noise_pair, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn voice() -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::ClockNoisePair,
            UGenSpec::ClockNoisePair,
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,

            output: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![(
            1,
            catalog::port_ctl(&Node::ClockNoisePair, 4).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn assert_paths(l: &[f32], r: &[f32]) {
    assert!(l.iter().chain(r).all(|x| x.is_finite()));
    assert!(rms(l) > 0.0001 && rms(r) > 0.0001);
    #[allow(clippy::cast_precision_loss)]
    let delta = l.iter().zip(r).map(|(a, b)| (a - b).abs()).sum::<f32>() / l.len() as f32;
    assert!(delta > 0.001, "main/aux filter outputs differ: {delta}");
}

fn direct(sr: f32, values: [f32; 4], mode: f32, block: usize, offset: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: 2048,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 123,
    };
    let mut controls = [Inp::Val(0.0); MAX_PORTS];
    for (port, value) in controls.iter_mut().zip(values) {
        *port = Inp::Val(value);
    }
    controls[4] = Inp::Val(mode);
    let mut state = NodeState::default();
    let mut audio = vec![0.0; 2048];
    let mut pos = 0;
    let mut size = block - offset;
    while pos < audio.len() {
        let end = (pos + size).min(audio.len());
        clock_noise_pair::render(&controls, &mut state, &mut audio[pos..end], &kx);
        pos = end;
        size = block;
    }
    audio
}

fn difference(a: &[f32], b: &[f32]) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let size = a.len() as f32;
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / size
}

fn dynamic_clock(block: usize, offset: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 1024,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 123,
    };
    let timbre: Vec<f32> = (0..1024)
        .map(|i| if (i / 37) % 2 == 0 { 0.2 } else { 0.8 })
        .collect();
    let mut state = NodeState::default();
    let mut audio = vec![0.0; 1024];
    let mut pos = 0;
    let mut size = block - offset;
    while pos < audio.len() {
        let end = (pos + size).min(audio.len());
        let mut controls = [Inp::Val(0.0); MAX_PORTS];
        controls[0] = Inp::Val(220.0);
        controls[1] = Inp::Val(0.7);
        controls[2] = Inp::Buf(&timbre[pos..end]);
        controls[3] = Inp::Val(0.5);
        controls[4] = Inp::Val(1.0);
        clock_noise_pair::render(&controls, &mut state, &mut audio[pos..end], &kx);
        pos = end;
        size = block;
    }
    audio
}

#[test]
fn source_stage_controls_and_three_filter_roles_affect_audio() {
    let base = [220.0, 0.5, 0.5, 0.5];
    let main = direct(48_000.0, base, 0.0, 24, 0);
    let aux = direct(48_000.0, base, 1.0, 24, 0);
    assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
    assert!(difference(&main, &aux) > 0.001);
    for values in [
        [440.0, 0.5, 0.5, 0.5],
        [220.0, 0.0, 0.5, 0.5],
        [220.0, 1.0, 0.5, 0.5],
        [220.0, 0.5, 0.0, 0.5],
        [220.0, 0.5, 1.0, 0.5],
        [220.0, 0.5, 0.5, 0.0],
        [220.0, 0.5, 0.5, 1.0],
    ] {
        let changed = direct(48_000.0, values, 0.0, 64, 0);
        assert!(changed.iter().all(|x| x.is_finite()));
        assert!(difference(&main, &changed) > 0.0001, "{values:?}");
    }
    let lowpass = direct(48_000.0, [220.0, 0.0, 0.5, 0.5], 0.0, 24, 0);
    let bandpass = direct(48_000.0, [220.0, 0.5, 0.5, 0.5], 0.0, 24, 0);
    let negative_highpass = direct(48_000.0, [220.0, 1.0, 0.5, 0.5], 0.0, 24, 0);
    assert!(difference(&lowpass, &bandpass) > 0.001);
    assert!(difference(&bandpass, &negative_highpass) > 0.001);
}

#[test]
fn both_paths_keep_reset_and_clock_state_across_callback_partitions() {
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for mode in [0.0, 1.0] {
            let values = [220.0, 0.61, 0.7, 0.8];
            let reference = direct(sr, values, mode, 24, 0);
            for (block, offset) in [(64, 0), (256, 0), (64, 17), (256, 17), (37, 11)] {
                assert_eq!(
                    reference,
                    direct(sr, values, mode, block, offset),
                    "{sr} {mode} {block} {offset}"
                );
            }
            assert_eq!(reference, direct(sr, values, mode, 24, 0));
        }
    }
    let reference = dynamic_clock(24, 0);
    for (block, offset) in [(64, 0), (256, 0), (64, 17), (256, 17)] {
        assert_eq!(
            reference,
            dynamic_clock(block, offset),
            "clock increment state: {block}/{offset}"
        );
    }
}

#[test]
fn node_reset_restarts_clock_and_seeded_hold_at_event_start() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 64,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 123,
    };
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    inputs[0] = Inp::Val(220.0);
    inputs[1] = Inp::Val(0.5);
    inputs[2] = Inp::Val(0.5);
    inputs[3] = Inp::Val(0.5);
    let mut state = NodeState::default();
    let mut first = [0.0; 64];
    let mut continued = [0.0; 64];
    let mut reset = [0.0; 64];
    clock_noise_pair::render(&inputs, &mut state, &mut first, &kx);
    clock_noise_pair::render(&inputs, &mut state, &mut continued, &kx);
    // Voice::start resets NodeState; a retrigger with the same seed restarts
    // phase, held sample and filter state exactly.
    state = NodeState::default();
    clock_noise_pair::render(&inputs, &mut state, &mut reset, &kx);
    assert_eq!(first, reset);
    assert_ne!(continued, reset);
}

#[test]
fn two_scheduled_hits_reseed_deterministically() {
    fn sequence(second: bool) -> Vec<f32> {
        let mut cfg = config(&caps(), StoreKind::NativeArc);
        cfg.sample_rate = 48_000.0;
        cfg.max_block = 64;
        let mut rig = NativeRig::native_with(cfg);
        rig.install(&voice());
        let _ = rig.step();
        let start = rig.engine.now() + 17.0 / 48_000.0;
        let controls = &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)];
        rig.send(event(1, start, controls));
        if second {
            rig.send(event(1, start + 0.04, controls));
        }
        rig.run(100).0
    }
    let once = sequence(false);
    let twice = sequence(true);
    assert_eq!(twice, sequence(true));
    assert!(once.iter().chain(&twice).all(|x| x.is_finite()));
    assert!(difference(&once[2_000..], &twice[2_000..]) > 0.0001);
}

#[test]
fn native_clock_noise_outputs_survive_rates_and_blocks() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = NativeRig::native_with(cfg);
            rig.install(&voice());
            let _ = rig.step();
            rig.send(event(
                1,
                rig.engine.now() + 17.0 / f64::from(rate),
                &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = rig.run(20);
            assert!(l[..17].iter().all(|x| x.abs() < 1.0e-7));
            assert_paths(&l, &r);
        }
    }
}

#[test]
fn browser_clock_noise_codec_preserves_two_outputs() {
    let def = voice();
    let env = NativeRig::native().engine.build_env();
    let native = Template::from_inst(&def, &env).unwrap();
    assert!(native.has_aux);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    let mut bus = BusTemplate::new();
    decode_graph(&bytes, &mut raw, &mut bus).unwrap();
    let mut decoded = Template::boxed();
    decoded.build(&raw, &env).unwrap();
    assert_eq!(decoded.nodes(), native.nodes());

    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = rate;
            cfg.max_block = block;
            let mut rig = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            rig.push(&record);
            let _ = rig.run(6);
            rig.send(event(
                1,
                rig.engine.now() + 17.0 / f64::from(rate),
                &[(ctl::FREQ, 220.0), (ctl::LEGATO, 1.0)],
            ));
            let (l, r) = rig.run(20);
            assert!(l[..17].iter().all(|x| x.abs() < 1.0e-7));
            assert_paths(&l, &r);
        }
    }
}
