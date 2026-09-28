//! Immutable 36-record Stages chain boundaries and host routing.

use super::{caps, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{decode_graph, encode_inst, FaultCode, SampleStore, StoreKind};
use crate::dsp::bus::BusTemplate;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::{Edge, InstDef, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::ugen::{
    catalog, stage_linked, BuildEnv, Inp, Kx, Node, NodeState, RawGraph, Template, MAX_PORTS,
};
use crate::host::wire::Ctl;

fn rows(count: usize) -> stage_linked::StageData {
    let mut data = stage_linked::StageData::EMPTY;
    data.len = count as u8;
    for row in data.rows.iter_mut().take(count) {
        *row = [2.0, 0.0, 0.25, 0.0];
    }
    data
}

fn voice(data: stage_linked::StageData) -> InstDef {
    InstDef {
        id: InstId::new(1),
        params: Box::new([]),
        nodes: vec![
            UGenSpec::StageLinked {
                data: Some(Box::new(data)),
            },
            UGenSpec::StageLinked {
                data: Some(Box::new(data)),
            },
            UGenSpec::AuxOut,
        ]
        .into_boxed_slice(),
        edges: vec![Edge {
            from: 1,
            to: 2,
            port: 0,
        }]
        .into_boxed_slice(),
        node_params: vec![(
            1,
            catalog::port_ctl(&Node::StageLinked { slot: 0 }, 3).unwrap(),
            Ctl::Const(1.0),
        )]
        .into_boxed_slice(),
    }
}

fn direct(
    data: &stage_linked::StageData,
    frames: usize,
    gate: f32,
    st: &mut NodeState,
    mem: &mut [f32; stage_linked::STATE_FLOATS],
) -> Vec<f32> {
    direct_channel(data, frames, gate, 0.0, st, mem)
}

fn direct_channel(
    data: &stage_linked::StageData,
    frames: usize,
    gate: f32,
    channel: f32,
    st: &mut NodeState,
    mem: &mut [f32; stage_linked::STATE_FLOATS],
) -> Vec<f32> {
    direct_controls(data, frames, gate, 0.0, channel, st, mem)
}

fn direct_controls(
    data: &stage_linked::StageData,
    frames: usize,
    gate: f32,
    trigger: f32,
    channel: f32,
    st: &mut NodeState,
    mem: &mut [f32; stage_linked::STATE_FLOATS],
) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: frames,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(220.0);
    ins[1] = Inp::Val(gate);
    ins[2] = Inp::Val(trigger);
    ins[3] = Inp::Val(channel);
    let mut out = vec![0.0; frames];
    stage_linked::render(data, &ins, st, mem, &mut out, &kx);
    out
}

fn sequencer(mode: usize, steps: usize) -> stage_linked::StageData {
    let mut data = stage_linked::StageData::EMPTY;
    data.len = (steps + 1) as u8;
    data.rows[0] = [0.0, 0.0, 0.0, (mode as f32 + 0.5) / 7.0];
    for (index, row) in data.rows.iter_mut().take(steps + 1).enumerate().skip(1) {
        *row = [1.0, 0.0, index as f32 / (steps + 1) as f32, 0.0];
    }
    data
}

fn clock_step(
    data: &stage_linked::StageData,
    st: &mut NodeState,
    mem: &mut [f32; stage_linked::STATE_FLOATS],
) -> usize {
    let _ = direct(data, 1, 0.0, st, mem);
    let _ = direct(data, 1, 1.0, st, mem);
    mem[0] as usize
}

#[test]
fn source_shaped_seven_modes_and_marked_range_are_distinct() {
    let expected = [
        [2, 3, 4, 1, 2, 3],
        [3, 2, 1, 4, 3, 2],
        [2, 3, 4, 3, 2, 1],
        [2, 1, 3, 1, 4, 1],
    ];
    for (mode, path) in expected.iter().enumerate() {
        let data = sequencer(mode, 4);
        assert!(data.is_sequencer());
        let mut state = NodeState::default();
        let mut mem = [0.0; stage_linked::STATE_FLOATS];
        for &step in path {
            assert_eq!(clock_step(&data, &mut state, &mut mem), step, "mode {mode}");
        }
    }
    for mode in [4, 5] {
        let data = sequencer(mode, 4);
        let mut a = NodeState::default();
        let mut b = NodeState::default();
        let mut am = [0.0; stage_linked::STATE_FLOATS];
        let mut bm = [0.0; stage_linked::STATE_FLOATS];
        let aa: Vec<_> = (0..32)
            .map(|_| clock_step(&data, &mut a, &mut am))
            .collect();
        let bb: Vec<_> = (0..32)
            .map(|_| clock_step(&data, &mut b, &mut bm))
            .collect();
        assert_eq!(aa, bb, "deterministic random mode {mode}");
        assert!(aa.iter().all(|&i| (1..=4).contains(&i)));
        assert!(aa.windows(2).all(|w| mode == 4 || w[0] != w[1]));
        assert!(aa.windows(2).any(|w| w[0] != w[1]));
    }
    let mut addressed = sequencer(6, 4);
    addressed.rows[0][2] = 0.75;
    let mut state = NodeState::default();
    let mut mem = [0.0; stage_linked::STATE_FLOATS];
    assert_eq!(clock_step(&addressed, &mut state, &mut mem), 4);
    let aux = direct_channel(&addressed, 1, 0.0, 1.0, &mut state, &mut mem);
    assert_eq!(aux, [1.0]);
    let mut marked = sequencer(0, 4);
    marked.rows[2][1] = 1.0;
    marked.rows[3][1] = 1.0;
    let mut state = NodeState::default();
    let mut mem = [0.0; stage_linked::STATE_FLOATS];
    assert_eq!(
        (0..4)
            .map(|_| clock_step(&marked, &mut state, &mut mem))
            .collect::<Vec<_>>(),
        [3, 2, 3, 2]
    );
}

#[test]
fn sequencer_reset_slew_and_chain_fallback_remain_bounded() {
    let mut data = sequencer(0, 3);
    data.rows[1][2] = 0.1;
    data.rows[2][2] = 0.9;
    data.rows[2][3] = 1.0;
    let mut state = NodeState::default();
    let mut mem = [0.0; stage_linked::STATE_FLOATS];
    assert_eq!(clock_step(&data, &mut state, &mut mem), 2);
    let slow = direct(&data, 1, 1.0, &mut state, &mut mem)[0];
    assert!(slow < 0.2, "step secondary slews the main lane: {slow}");
    let mut fast = data;
    fast.rows[2][3] = 0.0;
    let mut fast_state = NodeState::default();
    let mut fast_mem = [0.0; stage_linked::STATE_FLOATS];
    assert_eq!(clock_step(&fast, &mut fast_state, &mut fast_mem), 2);
    assert!(direct(&fast, 1, 1.0, &mut fast_state, &mut fast_mem)[0] > slow);
    data.rows[0][2] = 0.2;
    state = NodeState::default();
    mem.fill(0.0);
    let _ = direct(&data, 1, 1.0, &mut state, &mut mem);
    assert_eq!(mem[0], 1.0, "primary resets at onset");
    assert_eq!(
        clock_step(&data, &mut state, &mut mem),
        2,
        "clock resumes in adaptation"
    );
    let _ = direct_controls(&data, 1, 0.0, 1.0, 0.0, &mut state, &mut mem);
    assert_eq!(mem[0], 1.0, "trigger edge resets the selected step");
    let mut ordinary = data;
    ordinary.rows[2][0] = 2.0;
    assert!(!ordinary.is_sequencer());
    ordinary.rows[2][0] = 1.0;
    ordinary.rows[0][1] = 1.0;
    assert!(!ordinary.is_sequencer());
}

#[test]
fn strict_stride_discrete_bounds_and_last_record() {
    let mut flat = [2.0, 0.0, 0.3, 0.5].repeat(36);
    assert_eq!(stage_linked::StageData::from_flat(&flat).unwrap().len, 36);
    for bad in [vec![], vec![0.0], [0.0, 0.0, 0.0, 0.0].repeat(37)] {
        assert!(stage_linked::StageData::from_flat(&bad).is_err());
    }
    for (index, value) in [(0, 1.5), (1, 0.5), (2, -0.01), (3, 1.01), (143, f32::NAN)] {
        let old = flat[index];
        flat[index] = value;
        assert!(
            stage_linked::StageData::from_flat(&flat).is_err(),
            "index {index}"
        );
        flat[index] = old;
    }
    let mut a = rows(36);
    a.rows[35][2] = 0.1;
    let mut b = a;
    b.rows[35][2] = 0.9;
    let mut st_a = NodeState::default();
    let mut st_b = NodeState::default();
    let mut mem_a = [0.0; stage_linked::STATE_FLOATS];
    let mut mem_b = [0.0; stage_linked::STATE_FLOATS];
    let sa = direct(&a, 5000, 1.0, &mut st_a, &mut mem_a);
    let sb = direct(&b, 5000, 1.0, &mut st_b, &mut mem_b);
    assert!((sa[4500] - sb[4500]).abs() > 0.3);
    assert_eq!(mem_a[0], 35.0);
}

#[test]
fn loop_exit_and_step_edge_transition() {
    let mut looped = rows(3);
    looped.rows[1][1] = 1.0;
    looped.rows[1][2] = 0.8;
    let mut st = NodeState::default();
    let mut mem = [0.0; stage_linked::STATE_FLOATS];
    let _ = direct(&looped, 2000, 1.0, &mut st, &mut mem);
    assert_eq!(mem[0], 1.0);
    let _ = direct(&looped, 1, 0.0, &mut st, &mut mem);
    assert_eq!(mem[0], 2.0);
    let mut steps = rows(2);
    steps.rows[0] = [1.0, 0.0, 0.8, 0.2];
    st = NodeState::default();
    mem.fill(0.0);
    let _ = direct(&steps, 100, 1.0, &mut st, &mut mem);
    assert_eq!(mem[0], 0.0);
    let _ = direct(&steps, 1, 0.0, &mut st, &mut mem);
    let _ = direct(&steps, 1, 1.0, &mut st, &mut mem);
    assert_eq!(mem[0], 1.0);
}

#[test]
fn native_browser_codec_rates_blocks_and_exact_budget() {
    let mut data = rows(3);
    data.rows[0] = [0.0, 0.0, 0.15, 0.5];
    data.rows[1] = [2.0, 1.0, 0.85, 0.25];
    let def = voice(data);
    let mut bytes = Vec::new();
    encode_inst(&def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    assert_eq!(raw.n_stage_payloads, 2);
    assert_eq!(raw.stage_payloads[0], data);
    let budget = 2 * stage_linked::STATE_FLOATS;
    let env = BuildEnv {
        sr: 96_000.0,
        caps: caps(),
        voice_mem: budget,
    };
    assert_eq!(Template::from_inst(&def, &env).unwrap().mem_total, budget);
    assert!(Template::from_inst(
        &def,
        &BuildEnv {
            voice_mem: budget - 1,
            ..env
        }
    )
    .is_err());
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut cfg = config(&caps(), StoreKind::NativeArc);
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut native = NativeRig::native_with(cfg);
            native.install(&def);
            let _ = native.step();
            native.send(event(1, native.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (main, aux) = native.run(16);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
            let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            cfg.sample_rate = sr;
            cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(cfg);
            let mut record = Vec::new();
            encode_graph_record(1, 1, &bytes, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(1, browser.engine.now(), &[(ctl::FREQ, 220.0)]));
            let (main, aux) = browser.run(16);
            assert!(main.iter().chain(&aux).all(|v| v.is_finite()));
            assert!(rms(&main) > 1.0e-7 && rms(&aux) > 1.0e-7 && main != aux);
        }
    }
}

#[test]
fn thirty_six_step_sequencer_last_row_survives_both_hosts_and_rates() {
    let mut low = sequencer(6, 35);
    low.rows[0][2] = 1.0;
    low.rows[35][2] = 0.1;
    let mut high = low;
    high.rows[35][2] = 0.9;
    for data in [low, high] {
        let def = voice(data);
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert_eq!(raw.stage_payloads[0].rows[35][2], data.rows[35][2]);
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            for block in [64, 256] {
                let mut cfg = config(&caps(), StoreKind::NativeArc);
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut native = NativeRig::native_with(cfg);
                native.install(&def);
                let _ = native.step();
                native.send(event(1, native.engine.now(), &[]));
                let (main, aux) = native.run(8);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(rms(&main) > 1.0e-5 && rms(&aux) > 1.0e-5);
                let mut cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
                cfg.sample_rate = sr;
                cfg.max_block = block;
                let mut browser = BrowserRig::browser_with(cfg);
                let mut record = Vec::new();
                encode_graph_record(1, 1, &bytes, &mut record);
                browser.push(&record);
                let _ = browser.run(6);
                browser.send(event(1, browser.engine.now(), &[]));
                let (main, aux) = browser.run(8);
                assert!(main.iter().chain(&aux).all(|x| x.is_finite()));
                assert!(rms(&main) > 1.0e-5 && rms(&aux) > 1.0e-5);
            }
        }
    }
}

#[test]
fn versioned_node_decoder_rejects_bad_length_version_and_third_slot() {
    let data = rows(36);
    let mut bytes = Vec::new();
    catalog::put_spec(
        &mut catalog::Out(&mut bytes),
        &UGenSpec::StageLinked {
            data: Some(Box::new(data)),
        },
    )
    .unwrap();
    assert_eq!(bytes.len(), 3 + 36 * 4 * 4);
    assert_eq!(&bytes[..3], &[89, stage_linked::WIRE_VERSION, 36]);
    let decode =
        |b: &[u8], raw: &mut RawGraph| catalog::get_node(&mut catalog::In { b, pos: 0 }, raw);
    let mut raw = RawGraph::boxed();
    let mut wrong = bytes.clone();
    wrong[1] += 1;
    assert_eq!(decode(&wrong, &mut raw), Err(FaultCode::BadRecord));
    wrong = bytes.clone();
    wrong[2] = 37;
    assert_eq!(decode(&wrong, &mut raw), Err(FaultCode::BadRecord));
    assert_eq!(
        decode(&bytes[..bytes.len() - 1], &mut raw),
        Err(FaultCode::BadRecord)
    );
    assert_eq!(decode(&bytes, &mut raw), Ok(()));
    assert_eq!(decode(&bytes, &mut raw), Ok(()));
    assert_eq!(decode(&bytes, &mut raw), Err(FaultCode::GraphTooLarge));
}
