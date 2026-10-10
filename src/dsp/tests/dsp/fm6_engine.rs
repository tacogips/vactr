//! FM6 port, topology, rendering and compatibility tests.

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::fm::{
    self, algorithms, engine,
    patch::{self, Fm6Patch},
};
use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

#[test]
fn fm6_ports_contract() {
    use engine::{fm6_port, fm_mod_port};
    assert_eq!(
        engine::FM6_PORTS,
        [
            ("freq", 440.0),
            ("velocity", 1.0),
            ("algorithm", 0.0),
            ("ratio", 1.0),
            ("index", 1.0),
            ("fm6-feedback", 4.0)
        ]
    );
    assert_eq!(
        [
            fm6_port::FREQ,
            fm6_port::VELOCITY,
            fm6_port::ALGORITHM,
            fm6_port::RATIO,
            fm6_port::INDEX,
            fm6_port::FEEDBACK
        ],
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        [
            fm_mod_port::IN,
            fm_mod_port::MOD,
            fm_mod_port::INDEX,
            fm_mod_port::ALGORITHM,
            fm_mod_port::FREQ,
            fm_mod_port::RATIO,
            fm_mod_port::VELOCITY
        ],
        [0, 1, 2, 3, 4, 5, 6]
    );
    assert_eq!(engine::MACRO_FEEDBACK, 4);
    assert!(engine::STATE_FLOATS <= 160);
}

fn full_inputs(algorithm: f32, feedback: f32) -> [Inp<'static>; MAX_PORTS] {
    let mut inputs = std::array::from_fn(|index| {
        engine::FM6_PORTS
            .get(index)
            .map_or(Inp::Val(0.0), |(_, value)| Inp::Val(*value))
    });
    inputs[engine::fm6_port::FREQ] = Inp::Val(220.0);
    inputs[engine::fm6_port::VELOCITY] = Inp::Val(1.0);
    inputs[engine::fm6_port::ALGORITHM] = Inp::Val(algorithm);
    inputs[engine::fm6_port::RATIO] = Inp::Val(14.0);
    inputs[engine::fm6_port::INDEX] = Inp::Val(2.0);
    inputs[engine::fm6_port::FEEDBACK] = Inp::Val(feedback);
    inputs
}

fn fm_mod_inputs<'a>(
    algorithm: f32,
    in_buf: &'a [f32],
    mod_buf: &'a [f32],
) -> [Inp<'a>; MAX_PORTS] {
    let mut inputs = std::array::from_fn(|_| Inp::Val(0.0));
    inputs[engine::fm_mod_port::IN] = Inp::Buf(in_buf);
    inputs[engine::fm_mod_port::MOD] = Inp::Buf(mod_buf);
    inputs[engine::fm_mod_port::INDEX] = Inp::Val(0.7);
    inputs[engine::fm_mod_port::ALGORITHM] = Inp::Val(algorithm);
    inputs[engine::fm_mod_port::FREQ] = Inp::Val(220.0);
    inputs[engine::fm_mod_port::RATIO] = Inp::Val(14.0);
    inputs[engine::fm_mod_port::VELOCITY] = Inp::Val(1.0);
    inputs
}

fn render_macro(algorithm: u8, feedback: u8, frames: usize, block_size: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let inputs = full_inputs(f32::from(algorithm), f32::from(feedback));
    let mut state = NodeState::default();
    let mut memory = vec![0.0; engine::STATE_FLOATS];
    let mut audio = vec![0.0; frames];
    for block in audio.chunks_mut(block_size) {
        let kx = Kx {
            sr: 48_000.0,
            gate: block.len(),
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 41,
        };
        engine::fm6_render(&inputs, None, &mut state, &mut memory, block, &kx);
    }
    audio
}

fn differs(left: &[f32], right: &[f32]) -> bool {
    left.iter()
        .zip(right)
        .any(|(a, b)| a.to_bits() != b.to_bits())
}

fn reaches_carrier(algorithm: &algorithms::Algorithm, operator: usize, depth: usize) -> bool {
    if depth > 6 {
        return false;
    }
    if algorithm.carriers & (1 << operator) != 0 {
        return true;
    }
    (0..6).any(|target| {
        algorithm.modulators[target] & (1 << operator) != 0
            && reaches_carrier(algorithm, target, depth + 1)
    })
}

#[test]
fn fm_mod_legacy_branch_is_bit_identical() {
    let input: [f32; 128] = std::array::from_fn(|index| ((index * 17 % 31) as f32 - 15.0) / 16.0);
    let modulation: [f32; 128] =
        std::array::from_fn(|index| ((index * 11 % 23) as f32 - 11.0) / 12.0);
    for algorithm in [0.0, -1.0, f32::NAN] {
        let inputs = fm_mod_inputs(algorithm, &input, &modulation);
        let mut expected = [0.0; 128];
        fm::modulate(&inputs, &mut expected);
        let mut actual = [0.0; 128];
        let mut state = NodeState::default();
        state.s[0] = 0.375;
        let before_state = state;
        let mut memory = [3.25; engine::STATE_FLOATS];
        let before_memory = memory;
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = CapabilitySet::browser();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: 48_000.0,
            gate: actual.len(),
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 0,
        };
        fm::modulate_with_algorithm(&inputs, &mut state, &mut memory, &mut actual, &kx);
        assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
        assert_eq!(state, before_state);
        assert_eq!(memory, before_memory);
    }
}

#[test]
fn fm_mod_algorithms_are_pairwise_distinct() {
    const FRAMES: usize = 12_000;
    let modulation = [0.0_f32; FRAMES];
    let input = [0.0_f32; FRAMES];
    let mut renders = Vec::with_capacity(32);
    for algorithm in 1..=32 {
        let inputs = fm_mod_inputs(algorithm as f32, &input, &modulation);
        let mut state = NodeState::default();
        let mut memory = vec![0.0; engine::STATE_FLOATS];
        let mut audio = vec![0.0; FRAMES];
        let store = SampleStore::new(StoreKind::NativeArc);
        let caps = CapabilitySet::browser();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: 48_000.0,
            gate: FRAMES,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 0,
        };
        fm::modulate_with_algorithm(&inputs, &mut state, &mut memory, &mut audio, &kx);
        renders.push(audio);
    }
    for left in 0..renders.len() {
        for right in left + 1..renders.len() {
            assert!(
                differs(&renders[left], &renders[right]),
                "algorithms {} and {}",
                left + 1,
                right + 1
            );
        }
    }
}

#[test]
fn fm6_routing_is_observable() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    for algorithm_number in 1..=32 {
        let inputs = full_inputs(algorithm_number as f32, 0.0);
        let mut baseline_state = NodeState::default();
        let mut baseline_memory = vec![0.0; engine::STATE_FLOATS];
        let mut baseline = [0.0; 128];
        let setup_kx = Kx {
            sr: 48_000.0,
            gate: 128,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 1,
        };
        engine::fm6_render(
            &inputs,
            None,
            &mut baseline_state,
            &mut baseline_memory,
            &mut baseline,
            &setup_kx,
        );
        for operator in 0..6 {
            let mut state = baseline_state;
            let mut memory = baseline_memory.clone();
            engine::silence_op(&mut memory, operator + 1);
            let mut muted = [0.0; 128];
            let kx = Kx {
                sr: 48_000.0,
                gate: 128,
                bank: None,
                store: &store,
                caps: &caps,
                stats: &mut stats,
                seed: 1,
            };
            engine::fm6_render(&inputs, None, &mut state, &mut memory, &mut muted, &kx);
            let expected = reaches_carrier(algorithms::algorithm(algorithm_number), operator, 0);
            assert_eq!(
                differs(&baseline, &muted),
                expected,
                "algorithm {algorithm_number}, operator {}",
                operator + 1
            );
        }
    }
}

#[test]
fn fm6_feedback_uses_source_history_only_and_covers_cross_operator_edges() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    for (algorithm_number, source) in [(32, 6), (4, 4), (6, 5)] {
        let mut pair = Vec::new();
        for feedback in [0.0, 7.0] {
            let inputs = full_inputs(algorithm_number as f32, feedback);
            let mut state = NodeState::default();
            let mut memory = vec![0.0; engine::STATE_FLOATS];
            let mut warm = [0.0; 128];
            let warm_kx = Kx {
                sr: 48_000.0,
                gate: 128,
                bank: None,
                store: &store,
                caps: &caps,
                stats: &mut stats,
                seed: 2,
            };
            engine::fm6_render(&inputs, None, &mut state, &mut memory, &mut warm, &warm_kx);
            if algorithm_number == 32 {
                for operator in 1..=6 {
                    if operator != source {
                        engine::silence_op(&mut memory, operator);
                    }
                }
            }
            let mut output = [0.0; 128];
            let kx = Kx {
                sr: 48_000.0,
                gate: 128,
                bank: None,
                store: &store,
                caps: &caps,
                stats: &mut stats,
                seed: 2,
            };
            engine::fm6_render(&inputs, None, &mut state, &mut memory, &mut output, &kx);
            pair.push(output);
        }
        assert!(
            differs(&pair[0], &pair[1]),
            "feedback edge in algorithm {algorithm_number}, source op {source}"
        );
    }
}

fn audible_patch() -> Fm6Patch {
    let mut patch = Fm6Patch::EMPTY;
    patch.params[patch::global::ALGORITHM] = 0;
    patch.params[patch::global::FEEDBACK] = 0;
    let base = patch::op_base(1);
    patch.params[base + patch::op::OUTPUT_LEVEL] = 99;
    patch.params[base + patch::op::OSC_MODE] = 0;
    patch.params[base + patch::op::COARSE] = 1;
    patch.params[base + patch::op::FINE] = 0;
    patch.params[base + patch::op::DETUNE] = 7;
    for (offset, value) in [99, 99, 70, 50].into_iter().enumerate() {
        patch.params[base + patch::op::R1 + offset] = value;
    }
    for (offset, value) in [99, 99, 50, 0].into_iter().enumerate() {
        patch.params[base + patch::op::L1 + offset] = value;
    }
    patch
}

#[test]
fn fm6_patch_mode_eg_shapes_output_and_releases() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let inputs = full_inputs(1.0, 0.0);
    let patch = audible_patch();
    let mut state = NodeState::default();
    let mut memory = vec![0.0; engine::STATE_FLOATS];
    let mut audio = vec![0.0; 48_000];
    for block in audio.chunks_mut(128) {
        let kx = Kx {
            sr: 48_000.0,
            gate: block.len(),
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 3,
        };
        engine::fm6_render(&inputs, Some(&patch), &mut state, &mut memory, block, &kx);
    }
    let rms = |samples: &[f32]| {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    };
    assert!(
        rms(&audio[24_000..43_200]) < rms(&audio[..960]),
        "late EG should decay below attack RMS"
    );
    while !state.done() && audio.len() < 240_000 {
        let mut release = [0.0; 128];
        let kx = Kx {
            sr: 48_000.0,
            gate: 0,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 3,
        };
        engine::fm6_render(
            &inputs,
            Some(&patch),
            &mut state,
            &mut memory,
            &mut release,
            &kx,
        );
        audio.extend_from_slice(&release);
    }
    assert!(state.done(), "release should finish within five seconds");
}

#[test]
fn fm6_patch_key_off_waits_for_next_eg_chunk() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let inputs = full_inputs(1.0, 0.0);
    let patch = audible_patch();
    let mut edge_state = NodeState::default();
    let mut edge_memory = vec![0.0; engine::STATE_FLOATS];
    let mut initial = [0.0; 128];
    let held = Kx {
        sr: 48_000.0,
        gate: 128,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 7,
    };
    engine::fm6_render(
        &inputs,
        Some(&patch),
        &mut edge_state,
        &mut edge_memory,
        &mut initial,
        &held,
    );
    let mut held_state = edge_state;
    let mut held_memory = edge_memory.clone();

    let mut before_boundary_edge = [0.0; 64];
    let mut before_boundary_held = [0.0; 64];
    {
        let edge_gate = Kx {
            sr: 48_000.0,
            gate: 17,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 7,
        };
        engine::fm6_render(
            &inputs,
            Some(&patch),
            &mut edge_state,
            &mut edge_memory,
            &mut before_boundary_edge,
            &edge_gate,
        );
    }
    {
        let held_gate = Kx {
            sr: 48_000.0,
            gate: 64,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 7,
        };
        engine::fm6_render(
            &inputs,
            Some(&patch),
            &mut held_state,
            &mut held_memory,
            &mut before_boundary_held,
            &held_gate,
        );
    }
    assert_eq!(before_boundary_edge, before_boundary_held);

    let mut at_boundary_edge = [0.0; 64];
    let mut at_boundary_held = [0.0; 64];
    {
        let edge_gate = Kx {
            sr: 48_000.0,
            gate: 64,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 7,
        };
        engine::fm6_render(
            &inputs,
            Some(&patch),
            &mut edge_state,
            &mut edge_memory,
            &mut at_boundary_edge,
            &edge_gate,
        );
    }
    {
        let held_gate = Kx {
            sr: 48_000.0,
            gate: 64,
            bank: None,
            store: &store,
            caps: &caps,
            stats: &mut stats,
            seed: 7,
        };
        engine::fm6_render(
            &inputs,
            Some(&patch),
            &mut held_state,
            &mut held_memory,
            &mut at_boundary_held,
            &held_gate,
        );
    }
    assert!(differs(&at_boundary_edge, &at_boundary_held));
}

#[test]
fn fm6_patch_fixed_frequency_operator_ignores_note_pitch() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let mut patch = Fm6Patch::EMPTY;
    patch.params[patch::global::ALGORITHM] = 31;
    let base = patch::op_base(1);
    for (field, value) in [
        (patch::op::OSC_MODE, 1),
        (patch::op::COARSE, 1),
        (patch::op::FINE, 0),
        (patch::op::DETUNE, 7),
        (patch::op::OUTPUT_LEVEL, 99),
        (patch::op::R1, 99),
        (patch::op::R2, 99),
        (patch::op::R3, 99),
        (patch::op::R4, 99),
        (patch::op::L1, 99),
        (patch::op::L2, 99),
        (patch::op::L3, 99),
        (patch::op::L4, 0),
    ] {
        patch.params[base + field] = value;
    }
    let mut estimate = |freq: f32| {
        let mut inputs = full_inputs(32.0, 0.0);
        inputs[engine::fm6_port::FREQ] = Inp::Val(freq);
        let mut state = NodeState::default();
        let mut memory = vec![0.0; engine::STATE_FLOATS];
        let mut audio = vec![0.0; 48_000];
        for block in audio.chunks_mut(128) {
            let kx = Kx {
                sr: 48_000.0,
                gate: block.len(),
                bank: None,
                store: &store,
                caps: &caps,
                stats: &mut stats,
                seed: 4,
            };
            engine::fm6_render(&inputs, Some(&patch), &mut state, &mut memory, block, &kx);
        }
        let samples = &audio[2_400..];
        let crossing_frames: Vec<usize> = samples
            .windows(2)
            .enumerate()
            .filter_map(|(frame, pair)| (pair[0] <= 0.0 && pair[1] > 0.0).then_some(frame))
            .collect();
        let span = crossing_frames.last().copied().unwrap_or(0)
            - crossing_frames.first().copied().unwrap_or(0);
        (crossing_frames.len().saturating_sub(1)) as f32 * 48_000.0 / span.max(1) as f32
    };
    for freq in [220.0, 440.0] {
        let hz = estimate(freq);
        assert!(
            (9.8..=10.2).contains(&hz),
            "fixed frequency at note {freq} Hz was {hz}"
        );
    }
}

#[test]
fn fm6_block_size_independent_and_deterministic() {
    for alg in [1, 4, 6, 17, 32] {
        let a = render_macro(alg, 5, 12_288, 64);
        let b = render_macro(alg, 5, 12_288, 128);
        let c = render_macro(alg, 5, 12_288, 256);
        assert_eq!(a, b, "algorithm {alg}, 64 vs 128");
        assert_eq!(a, c, "algorithm {alg}, 64 vs 256");
        assert_eq!(
            a,
            render_macro(alg, 5, 12_288, 128),
            "algorithm {alg}, repeat"
        );
    }
}

#[test]
fn fm6_stability_grid_is_finite_and_bounded() {
    for algorithm in 1..=32 {
        for freq in [20.0, 8_000.0] {
            let mut inputs = full_inputs(algorithm as f32, 7.0);
            inputs[engine::fm6_port::FREQ] = Inp::Val(freq);
            let store = SampleStore::new(StoreKind::NativeArc);
            let caps = CapabilitySet::browser();
            let mut stats = FxStats::default();
            let kx = Kx {
                sr: 48_000.0,
                gate: 256,
                bank: None,
                store: &store,
                caps: &caps,
                stats: &mut stats,
                seed: 5,
            };
            let mut state = NodeState::default();
            let mut memory = vec![0.0; engine::STATE_FLOATS];
            let mut output = [0.0; 256];
            engine::fm6_render(&inputs, None, &mut state, &mut memory, &mut output, &kx);
            assert!(
                output
                    .iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 1.0),
                "algorithm {algorithm}, freq {freq}"
            );
        }
    }
}

#[test]
fn fm6_render_does_not_allocate() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let mut state = NodeState::default();
    let mut memory = [0.0; engine::STATE_FLOATS];
    let mut output = [0.0; 128];
    let inputs = full_inputs(13.0, 4.0);
    let kx = Kx {
        sr: 48_000.0,
        gate: 128,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 6,
    };
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        engine::fm6_render(&inputs, None, &mut state, &mut memory, &mut output, &kx);
        engine::fm6_render(&inputs, None, &mut state, &mut memory, &mut output, &kx);
    });
    assert_eq!(allocations, 0);

    static INPUT: [f32; 128] = [0.2; 128];
    static MOD: [f32; 128] = [0.1; 128];
    let inputs = fm_mod_inputs(5.0, &INPUT, &MOD);
    let mut state = NodeState::default();
    let mut memory = [0.0; engine::STATE_FLOATS];
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        fm::modulate_with_algorithm(&inputs, &mut state, &mut memory, &mut output, &kx);
        fm::modulate_with_algorithm(&inputs, &mut state, &mut memory, &mut output, &kx);
    });
    assert_eq!(allocations, 0);
}
