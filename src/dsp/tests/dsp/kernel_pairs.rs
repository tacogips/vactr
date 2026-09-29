//! Pair kernels publish complementary outputs without advancing state twice.

use std::sync::Arc;

use crate::dsp::arena::SampleStore;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::graph::BankRef;
use crate::dsp::ugen::{
    analog_pair, chord_pair, fm_pair, sample, shape_pair, stage_chain, string_machine_pair,
    table_terrain_pair, terrain_pair, va_filter, Inp, Kx, NodeState, MAX_PORTS,
};
use crate::host::caps::SampleData;

type MemoryLegacy =
    for<'a> fn(&[Inp<'a>; MAX_PORTS], &mut NodeState, &mut [f32], &mut [f32], &Kx<'a>);
type MemoryPair =
    for<'a> fn(&[Inp<'a>; MAX_PORTS], &mut NodeState, &mut [f32], &mut [f32], &mut [f32], &Kx<'a>);
type StateLegacy = for<'a> fn(&[Inp<'a>; MAX_PORTS], &mut NodeState, &mut [f32], &Kx<'a>);
type StatePair = for<'a> fn(&[Inp<'a>; MAX_PORTS], &mut NodeState, &mut [f32], &mut [f32], &Kx<'a>);

fn assert_bits_eq(actual: &[f32], expected: &[f32], context: &str) {
    assert_eq!(actual.len(), expected.len(), "{context}: lengths");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{context}: frame {index}"
        );
    }
}

fn check_memory_pair(
    mut inputs: [Inp<'_>; MAX_PORTS],
    memory_len: usize,
    legacy: MemoryLegacy,
    pair: MemoryPair,
    kx: &Kx<'_>,
) {
    let original = inputs;
    for parameter_set in 0..2 {
        inputs = original;
        if parameter_set == 1 {
            for input in &mut inputs[..4] {
                *input = Inp::Val(input.first() * 0.83 + 0.13);
            }
        }
        for selector in [0.0, 1.0] {
            inputs[4] = Inp::Val(selector);
            let mut pair_state = NodeState::default();
            let mut main_state = pair_state;
            let mut aux_state = pair_state;
            let mut pair_mem = vec![0.0; memory_len];
            let mut main_mem = pair_mem.clone();
            let mut aux_mem = pair_mem.clone();
            for block_len in [64, 97] {
                for block_index in 0..4 {
                    let mut main = vec![0.0; block_len];
                    let mut aux = vec![0.0; block_len];
                    let mut legacy_main = vec![0.0; block_len];
                    let mut legacy_aux = vec![0.0; block_len];
                    pair(
                        &inputs,
                        &mut pair_state,
                        &mut pair_mem,
                        &mut main,
                        &mut aux,
                        kx,
                    );
                    legacy(
                        &inputs,
                        &mut main_state,
                        &mut main_mem,
                        &mut legacy_main,
                        kx,
                    );
                    let mut complementary = inputs;
                    complementary[4] = Inp::Val(1.0 - selector);
                    legacy(
                        &complementary,
                        &mut aux_state,
                        &mut aux_mem,
                        &mut legacy_aux,
                        kx,
                    );
                    let context =
                        format!("selector={selector}, block_len={block_len}, block={block_index}");
                    assert_bits_eq(&main, &legacy_main, &format!("main {context}"));
                    assert_bits_eq(&aux, &legacy_aux, &format!("aux {context}"));
                    assert_eq!(pair_state, main_state, "main state {context}");
                    assert_eq!(pair_state, aux_state, "aux state {context}");
                    assert_bits_eq(&pair_mem, &main_mem, &format!("main memory {context}"));
                    assert_bits_eq(&pair_mem, &aux_mem, &format!("aux memory {context}"));
                }
            }
        }
    }
}

fn check_state_pair(
    mut inputs: [Inp<'_>; MAX_PORTS],
    legacy: StateLegacy,
    pair: StatePair,
    kx: &Kx<'_>,
) {
    let original = inputs;
    for parameter_set in 0..2 {
        inputs = original;
        if parameter_set == 1 {
            for input in &mut inputs[..4] {
                *input = Inp::Val(input.first() * 0.83 + 0.13);
            }
        }
        for selector in [0.0, 1.0] {
            inputs[4] = Inp::Val(selector);
            let mut pair_state = NodeState::default();
            let mut main_state = pair_state;
            let mut aux_state = pair_state;
            for block_len in [64, 97] {
                for block_index in 0..4 {
                    let mut main = vec![0.0; block_len];
                    let mut aux = vec![0.0; block_len];
                    let mut legacy_main = vec![0.0; block_len];
                    let mut legacy_aux = vec![0.0; block_len];
                    pair(&inputs, &mut pair_state, &mut main, &mut aux, kx);
                    legacy(&inputs, &mut main_state, &mut legacy_main, kx);
                    let mut complementary = inputs;
                    complementary[4] = Inp::Val(1.0 - selector);
                    legacy(&complementary, &mut aux_state, &mut legacy_aux, kx);
                    let context =
                        format!("selector={selector}, block_len={block_len}, block={block_index}");
                    assert_bits_eq(&main, &legacy_main, &format!("main {context}"));
                    assert_bits_eq(&aux, &legacy_aux, &format!("aux {context}"));
                    assert_eq!(pair_state, main_state, "main state {context}");
                    assert_eq!(pair_state, aux_state, "aux state {context}");
                }
            }
        }
    }
}

#[test]
fn nine_pair_kernels_match_both_legacy_selectors_over_consecutive_blocks() {
    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 8192,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 17,
    };

    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(330.0);
    ins[1] = Inp::Val(0.27);
    ins[2] = Inp::Val(0.68);
    ins[3] = Inp::Val(0.41);
    check_state_pair(ins, fm_pair::render, fm_pair::render_pair, &kx);
    check_memory_pair(
        ins,
        analog_pair::STATE_FLOATS,
        analog_pair::render,
        analog_pair::render_pair,
        &kx,
    );
    check_memory_pair(
        ins,
        chord_pair::STATE_FLOATS,
        chord_pair::render,
        chord_pair::render_pair,
        &kx,
    );
    check_memory_pair(
        ins,
        table_terrain_pair::STATE_FLOATS,
        table_terrain_pair::render,
        table_terrain_pair::render_pair,
        &kx,
    );
    check_memory_pair(
        ins,
        terrain_pair::STATE_FLOATS,
        terrain_pair::render,
        terrain_pair::render_pair,
        &kx,
    );
    check_memory_pair(
        ins,
        string_machine_pair::STATE_FLOATS,
        string_machine_pair::render,
        string_machine_pair::render_pair,
        &kx,
    );
    check_memory_pair(
        ins,
        shape_pair::STATE_FLOATS,
        shape_pair::render,
        shape_pair::render_pair,
        &kx,
    );

    let mut stage = [Inp::Val(0.0); MAX_PORTS];
    stage[0] = Inp::Val(220.0);
    stage[1] = Inp::Val(2.0);
    stage[2] = Inp::Val(1.0);
    stage[3] = Inp::Val(0.0);
    stage[5] = Inp::Val(0.0);
    stage[6] = Inp::Val(0.0);
    stage[7] = Inp::Val(0.8);
    stage[8] = Inp::Val(0.4);
    stage[9] = Inp::Val(0.0);
    stage[10] = Inp::Val(0.0);
    stage[11] = Inp::Val(0.6);
    stage[12] = Inp::Val(0.7);
    stage[13] = Inp::Val(0.0);
    check_memory_pair(
        stage,
        stage_chain::STATE_FLOATS,
        stage_chain::render,
        stage_chain::render_pair,
        &kx,
    );

    let mut va = [Inp::Val(0.0); MAX_PORTS];
    va[0] = Inp::Val(0.4);
    va[1] = Inp::Val(1200.0);
    va[2] = Inp::Val(0.3);
    va[3] = Inp::Val(0.7);
    check_state_pair(va, va_filter::filter, va_filter::filter_pair, &kx);
}

#[test]
fn va_filter_pair_reads_alternating_selector_per_sample() {
    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 64,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let selectors: Vec<f32> = (0..64).map(|index| f32::from(index % 2 == 1)).collect();
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    inputs[0] = Inp::Val(0.6);
    inputs[1] = Inp::Val(900.0);
    inputs[2] = Inp::Val(0.35);
    inputs[3] = Inp::Val(0.8);
    inputs[4] = Inp::Buf(&selectors);
    let mut pair_state = NodeState::default();
    let mut legacy_state = pair_state;
    let mut main = [0.0; 64];
    let mut aux = [0.0; 64];
    let mut expected = [0.0; 64];
    va_filter::filter_pair(&inputs, &mut pair_state, &mut main, &mut aux, &kx);
    va_filter::filter(&inputs, &mut legacy_state, &mut expected, &kx);
    assert_bits_eq(&main, &expected, "alternating selected path");
    assert_eq!(pair_state, legacy_state);
}

fn install_sample(store: &mut SampleStore, id: u32, channels: u8, frames: Vec<f32>) {
    let data = Arc::new(SampleData {
        rate: 48_000,
        channels,
        frames: frames.into_boxed_slice(),
    });
    assert!(store.install_arc(id, 1, data).is_ok());
}

fn sample_kx<'a>(
    store: &'a SampleStore,
    caps: &'a CapabilitySet,
    stats: &'a mut FxStats,
) -> Kx<'a> {
    Kx {
        sr: 48_000.0,
        gate: 64,
        bank: None,
        store,
        caps,
        stats,
        seed: 0,
    }
}

#[test]
fn play_stereo_preserves_mono_and_interpolates_both_resource_channels() {
    let mut store = SampleStore::default();
    let mono_data: Vec<f32> = (0..256).map(|frame| frame as f32 * 0.01).collect();
    install_sample(&mut store, 10, 1, mono_data.clone());
    let stereo_data: Vec<f32> = (0..256)
        .flat_map(|frame| {
            let value = frame as f32 * 0.01;
            [value, -0.25 * value]
        })
        .collect();
    install_sample(&mut store, 11, 2, stereo_data);
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = sample_kx(&store, &caps, &mut stats);
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    inputs[0] = Inp::Val(0.5);
    inputs[1] = Inp::Val(0.0);
    inputs[2] = Inp::Val(1.0);
    inputs[3] = Inp::Val(0.0);

    let mut mono_state = NodeState::default();
    let mut legacy_state = mono_state;
    let mut mono = [0.0; 64];
    let mut expected = [0.0; 64];
    let mut left = [0.0; 64];
    let mut right = [0.0; 64];
    sample::play_stereo(
        BankRef::new(10),
        &inputs,
        &mut mono_state,
        &mut mono,
        &mut left,
        &mut right,
        &kx,
    );
    sample::play(
        BankRef::new(10),
        &inputs,
        &mut legacy_state,
        &mut expected,
        &kx,
    );
    assert_bits_eq(&mono, &expected, "mono resource mono output");
    assert_bits_eq(&left, &mono, "mono resource left duplication");
    assert_bits_eq(&right, &mono, "mono resource right duplication");
    assert_eq!(mono_state, legacy_state);

    let mut stereo_state = NodeState::default();
    let mut legacy_stereo_state = stereo_state;
    let mut stereo_mono = [0.0; 64];
    let mut legacy_mono = [0.0; 64];
    let mut stereo_left = [0.0; 64];
    let mut stereo_right = [0.0; 64];
    sample::play_stereo(
        BankRef::new(11),
        &inputs,
        &mut stereo_state,
        &mut stereo_mono,
        &mut stereo_left,
        &mut stereo_right,
        &kx,
    );
    sample::play(
        BankRef::new(11),
        &inputs,
        &mut legacy_stereo_state,
        &mut legacy_mono,
        &kx,
    );
    assert_bits_eq(&stereo_mono, &legacy_mono, "stereo resource mono mix");
    assert_ne!(stereo_left, stereo_right);
    for frame in 0..64 {
        let position = frame as f32 * 0.5;
        let source = position.floor() as usize;
        let fraction = position.fract();
        let next = (source + 1).min(255);
        let expected_left = mono_data[source] + (mono_data[next] - mono_data[source]) * fraction;
        let expected_right = -0.25 * expected_left;
        assert_eq!(stereo_left[frame].to_bits(), expected_left.to_bits());
        assert_eq!(stereo_right[frame].to_bits(), expected_right.to_bits());
    }
    assert_eq!(stereo_state, legacy_stereo_state);
}

#[test]
fn play_stereo_early_returns_clear_every_output_and_finish_state() {
    let mut store = SampleStore::default();
    install_sample(&mut store, 98, 1, vec![0.25; 32]);
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = sample_kx(&store, &caps, &mut stats);
    let inputs = [Inp::Val(1.0); MAX_PORTS];
    for (bank, region_end) in [(BankRef::new(99), 1.0), (BankRef::new(98), 0.0)] {
        let mut inputs = inputs;
        inputs[2] = Inp::Val(region_end);
        let mut pair_state = NodeState::default();
        let mut legacy_state = pair_state;
        let mut mono = [3.0; 16];
        let mut left = [3.0; 16];
        let mut right = [3.0; 16];
        let mut expected = [3.0; 16];
        sample::play_stereo(
            bank,
            &inputs,
            &mut pair_state,
            &mut mono,
            &mut left,
            &mut right,
            &kx,
        );
        sample::play(bank, &inputs, &mut legacy_state, &mut expected, &kx);
        assert_eq!(mono, [0.0; 16]);
        assert_eq!(left, [0.0; 16]);
        assert_eq!(right, [0.0; 16]);
        assert_eq!(pair_state.done(), legacy_state.done());
        assert!(pair_state.done());
    }
}
