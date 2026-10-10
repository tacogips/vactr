//! Pinned kalimba port contract test owned by FM1V-00.

use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::kalimba;
use crate::dsp::ugen::{Inp, Kx, NodeState, MAX_PORTS};

#[test]
fn kalimba_ports_are_unique_and_within_max_ports() {
    const _: () = assert!(kalimba::PORT_COUNT <= crate::dsp::ugen::MAX_PORTS);
    let expected: [(&str, usize, f32); 8] = [
        ("freq", kalimba::port::FREQ, 440.0),
        ("kalimba-beat", kalimba::port::KALIMBA_BEAT, 1.5),
        ("kalimba-hardness", kalimba::port::KALIMBA_HARDNESS, 0.5),
        ("kalimba-decay", kalimba::port::KALIMBA_DECAY, 2.5),
        ("kalimba-damping", kalimba::port::KALIMBA_DAMPING, 0.5),
        ("kalimba-body", kalimba::port::KALIMBA_BODY, 0.3),
        ("kalimba-buzz", kalimba::port::KALIMBA_BUZZ, 0.0),
        ("velocity", kalimba::port::VELOCITY, 1.0),
    ];
    assert_eq!(kalimba::PORT_COUNT, expected.len());
    for (index, (name, port, default)) in expected.iter().enumerate() {
        assert_eq!(*port, index, "port index for {name}");
        assert_eq!(kalimba::PORTS[index].0, *name, "port name at {index}");
        assert_eq!(kalimba::PORTS[index].1, *default, "port default at {index}");
        for (other_index, (other_name, _, _)) in expected.iter().enumerate() {
            if index != other_index {
                assert_ne!(name, other_name, "duplicate port name {name}");
            }
        }
    }
}

fn inputs(overrides: &[(usize, f32)]) -> [Inp<'static>; MAX_PORTS] {
    let mut result = std::array::from_fn(|index| {
        kalimba::PORTS
            .get(index)
            .map_or(Inp::Val(0.0), |(_, value)| Inp::Val(*value))
    });
    for &(port, value) in overrides {
        result[port] = Inp::Val(value);
    }
    result
}

fn render_at(
    overrides: &[(usize, f32)],
    frames: usize,
    seed: u32,
    sr: f32,
    block_size: usize,
) -> (Vec<f32>, NodeState) {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: 0,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed,
    };
    let port_inputs = inputs(overrides);
    let mut state = NodeState::default();
    let mut memory = vec![0.0; kalimba::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for block in output.chunks_mut(block_size) {
        kalimba::render(&port_inputs, &mut state, &mut memory, block, &kx);
    }
    (output, state)
}

fn render(overrides: &[(usize, f32)], frames: usize, seed: u32) -> (Vec<f32>, NodeState) {
    render_at(overrides, frames, seed, 48_000.0, 256)
}

fn spectrum_band_energy(samples: &[f32], low_hz: f32, high_hz: f32) -> f32 {
    let bins = samples.len().next_power_of_two() / 2;
    let magnitudes = crate::dsp::offline::spectrum(samples, bins);
    let hz_per_bin = 48_000.0 / (2 * bins) as f32;
    magnitudes
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            let hz = *index as f32 * hz_per_bin;
            hz >= low_hz && hz <= high_hz
        })
        .map(|(_, magnitude)| magnitude * magnitude)
        .sum()
}

fn spectral_centroid(samples: &[f32]) -> f32 {
    let bins = samples.len() / 2;
    let magnitudes = crate::dsp::offline::spectrum(samples, bins);
    let mut weighted = 0.0;
    let mut total = 0.0;
    for (index, magnitude) in magnitudes.iter().enumerate().skip(1) {
        weighted += index as f32 * magnitude;
        total += magnitude;
    }
    if total == 0.0 {
        0.0
    } else {
        weighted / total * 48_000.0 / (2 * bins) as f32
    }
}

fn peak_near(magnitudes: &[f32], frequency: f32, sample_rate: f32) -> f32 {
    let bin_hz = sample_rate / (2 * magnitudes.len()) as f32;
    let center = (frequency / bin_hz).round() as usize;
    let radius = ((frequency * 0.01 / bin_hz).ceil() as usize).max(2);
    let start = center.saturating_sub(radius);
    let end = (center + radius + 1).min(magnitudes.len());
    let index = (start..end)
        .max_by(|&left, &right| magnitudes[left].total_cmp(&magnitudes[right]))
        .unwrap_or(center);
    index as f32 * bin_hz
}

#[test]
fn kalimba_mode_peaks_follow_beam_ratios() {
    let (samples, _) = render(
        &[
            (kalimba::port::FREQ, 440.0),
            (kalimba::port::KALIMBA_BEAT, 0.0),
            (kalimba::port::KALIMBA_HARDNESS, 1.0),
            (kalimba::port::KALIMBA_DECAY, 4.0),
            (kalimba::port::KALIMBA_DAMPING, 0.0),
            (kalimba::port::KALIMBA_BODY, 0.0),
        ],
        48_000,
        31,
    );
    let magnitudes = crate::dsp::offline::spectrum(&samples[..32_768], 16_384);
    for frequency in [440.0, 2_757.4, 7_720.9, 15_130.0] {
        let peak = peak_near(&magnitudes, frequency, 48_000.0);
        assert!(
            (peak - frequency).abs() <= frequency * 0.01,
            "peak {peak} Hz vs {frequency} Hz"
        );
    }
}

#[test]
fn kalimba_default_c4_peak_is_normalized() {
    let (samples, _) = render(&[(kalimba::port::FREQ, 261.625_6)], 4_096, 33);
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max);
    assert!((0.4..=0.9).contains(&peak), "default C4 peak was {peak}");
}

#[test]
fn kalimba_beat_period_matches_control() {
    let (samples, _) = render(
        &[
            (kalimba::port::KALIMBA_BEAT, 2.0),
            (kalimba::port::KALIMBA_DECAY, 4.0),
            (kalimba::port::KALIMBA_DAMPING, 1.0),
            (kalimba::port::KALIMBA_BODY, 0.0),
        ],
        144_000,
        37,
    );
    let envelope: Vec<f32> = samples
        .chunks_exact(480)
        .enumerate()
        .map(|(index, window)| {
            let time = index as f32 * 0.01;
            let decay_compensation = (3.0 * std::f32::consts::LN_10 * time / 4.0).exp();
            (window.iter().map(|x| x * x).sum::<f32>() / 480.0).sqrt() * decay_compensation
        })
        .collect();
    let start = 50;
    let end = 300.min(envelope.len());
    let mean = envelope[start..end].iter().sum::<f32>() / (end - start) as f32;
    let mut best_lag = 0;
    let mut best_corr = f32::NEG_INFINITY;
    for lag in 40..=60 {
        let mut corr = 0.0;
        for index in start..end - lag {
            corr += (envelope[index] - mean) * (envelope[index + lag] - mean);
        }
        if corr > best_corr {
            best_corr = corr;
            best_lag = lag;
        }
    }
    assert!(
        (best_lag as f32 * 0.01 - 0.5).abs() <= 0.025,
        "beat lag {best_lag} buckets"
    );
}

#[test]
fn kalimba_damping_shortens_upper_modes() {
    let mut energies = [0.0; 2];
    for (slot, damping) in [0.0, 1.0].into_iter().enumerate() {
        let (samples, _) = render(
            &[
                (kalimba::port::FREQ, 440.0),
                (kalimba::port::KALIMBA_BEAT, 0.0),
                (kalimba::port::KALIMBA_HARDNESS, 1.0),
                (kalimba::port::KALIMBA_DECAY, 4.0),
                (kalimba::port::KALIMBA_DAMPING, damping),
                (kalimba::port::KALIMBA_BODY, 0.0),
            ],
            20_000,
            41,
        );
        let early = spectrum_band_energy(&samples[..960], 2_500.0, 3_000.0);
        let late = spectrum_band_energy(&samples[14_400..15_360], 2_500.0, 3_000.0);
        energies[slot] = late / early.max(1.0e-20);
    }
    assert!(energies[1] < energies[0], "damping ratios: {energies:?}");
}

#[test]
fn kalimba_hardness_brightens() {
    let mut centroids = [0.0; 2];
    for (slot, hardness) in [0.0, 1.0].into_iter().enumerate() {
        let (samples, _) = render(
            &[
                (kalimba::port::KALIMBA_HARDNESS, hardness),
                (kalimba::port::KALIMBA_BODY, 0.0),
            ],
            2_400,
            43,
        );
        centroids[slot] = spectral_centroid(&samples[..2_048]);
    }
    assert!(centroids[1] > centroids[0], "centroids: {centroids:?}");
}

#[test]
fn kalimba_buzz_zero_is_exact_bypass_and_buzz_adds_highs() {
    let common = [
        (kalimba::port::KALIMBA_BEAT, 0.0),
        (kalimba::port::KALIMBA_BODY, 0.0),
        (kalimba::port::KALIMBA_DECAY, 2.5),
    ];
    let (zero_a, _) = render(&common, 9_600, 47);
    let (zero_b, _) = render(&common, 9_600, 47);
    let with_buzz = [
        common[0],
        common[1],
        common[2],
        (kalimba::port::KALIMBA_BUZZ, 1.0),
    ];
    let (buzzed, _) = render(&with_buzz, 9_600, 47);
    assert_eq!(zero_a, zero_b);
    assert_eq!(
        zero_a,
        render(
            &[
                common[0],
                common[1],
                common[2],
                (kalimba::port::KALIMBA_BUZZ, 0.0)
            ],
            9_600,
            47
        )
        .0
    );
    assert!(
        spectrum_band_energy(&buzzed[..9_600], 3_000.0, 6_000.0)
            > spectrum_band_energy(&zero_a[..9_600], 3_000.0, 6_000.0),
        "buzz did not raise high-band energy"
    );
}

#[test]
fn kalimba_finishes_and_is_deterministic() {
    let (a, state_a) = render(&[(kalimba::port::KALIMBA_DECAY, 1.0)], 144_000, 53);
    let (b, state_b) = render(&[(kalimba::port::KALIMBA_DECAY, 1.0)], 144_000, 53);
    assert_eq!(a, b);
    assert_eq!(state_a, state_b);
    assert!(
        state_a.done(),
        "voice did not finish within three decay seconds"
    );
}

#[test]
fn kalimba_finish_is_below_minus_80_db() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let caps = CapabilitySet::browser();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 0,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 33,
    };
    let port_inputs = inputs(&[(kalimba::port::FREQ, 261.625_6)]);
    let mut state = NodeState::default();
    let mut memory = [0.0; kalimba::STATE_FLOATS];
    let mut output = [0.0; 256];
    let mut final_samples = [0.0; 480];
    let mut rendered_frames = 0;

    for _ in 0..(12 * 48_000 / output.len()) {
        kalimba::render(&port_inputs, &mut state, &mut memory, &mut output, &kx);
        for sample in output {
            final_samples[rendered_frames % final_samples.len()] = sample;
            rendered_frames += 1;
        }
        if state.done() {
            break;
        }
    }

    assert!(state.done(), "voice did not finish within 12 seconds");
    assert!(rendered_frames as f32 / 48_000.0 >= 2.0);
    let tail_peak = final_samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max);
    assert!(tail_peak < 1.0e-4, "final 480-sample peak was {tail_peak}");
}

#[test]
fn kalimba_block_size_independent() {
    let overrides = [
        (kalimba::port::KALIMBA_DECAY, 0.1),
        (kalimba::port::KALIMBA_BODY, 0.0),
    ];
    let (small, small_state) = render_at(&overrides, 48_000, 59, 48_000.0, 64);
    let (large, large_state) = render_at(&overrides, 48_000, 59, 48_000.0, 256);
    assert_eq!(small, large);
    assert_eq!(small_state.done(), large_state.done());
    assert!(
        small_state.done(),
        "short-decay voice should finish within one second"
    );
}

#[test]
fn kalimba_extreme_grid_is_finite_and_bounded() {
    for freq in [20.0, 440.0, 8_000.0, 20_000.0] {
        for beat in [0.0, 8.0] {
            for hardness in [0.0, 1.0] {
                for decay in [0.1, 10.0] {
                    for damping in [0.0, 1.0] {
                        for body in [0.0, 1.0] {
                            for buzz in [0.0, 1.0] {
                                for velocity in [0.0, 1.0] {
                                    let (samples, _) = render_at(
                                        &[
                                            (kalimba::port::FREQ, freq),
                                            (kalimba::port::KALIMBA_BEAT, beat),
                                            (kalimba::port::KALIMBA_HARDNESS, hardness),
                                            (kalimba::port::KALIMBA_DECAY, decay),
                                            (kalimba::port::KALIMBA_DAMPING, damping),
                                            (kalimba::port::KALIMBA_BODY, body),
                                            (kalimba::port::KALIMBA_BUZZ, buzz),
                                            (kalimba::port::VELOCITY, velocity),
                                        ],
                                        512,
                                        61,
                                        48_000.0,
                                        256,
                                    );
                                    assert!(samples
                                        .iter()
                                        .all(|sample| sample.is_finite() && sample.abs() <= 1.0));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn kalimba_render_does_not_allocate() {
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
        seed: 67,
    };
    let port_inputs = inputs(&[(kalimba::port::KALIMBA_BUZZ, 1.0)]);
    let mut state = NodeState::default();
    let mut memory = [0.0; kalimba::STATE_FLOATS];
    let mut output = [0.0; 256];
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        kalimba::render(&port_inputs, &mut state, &mut memory, &mut output, &kx);
    });
    assert_eq!(allocations, 0);
}
