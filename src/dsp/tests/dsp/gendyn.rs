//! GENDYN port and behavioral contracts owned by FM1V-17.

use super::caps;
use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::{gendyn, Inp, Kx, NodeState, MAX_PORTS};

fn inputs(values: [f32; 6]) -> [Inp<'static>; MAX_PORTS] {
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    for (input, value) in ins.iter_mut().zip(values) {
        *input = Inp::Val(value);
    }
    ins
}

fn render(values: [f32; 6], sr: f32, seed: u32, frames: usize, block: usize) -> Vec<f32> {
    let store = SampleStore::new(StoreKind::NativeArc);
    let capability = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: frames,
        bank: None,
        store: &store,
        caps: &capability,
        stats: &mut stats,
        seed,
    };
    let ins = inputs(values);
    let mut state = NodeState::default();
    let mut memory = vec![0.0; gendyn::STATE_FLOATS];
    let mut output = vec![0.0; frames];
    for chunk in output.chunks_mut(block) {
        gendyn::render(&ins, &mut state, &mut memory, chunk, &kx);
    }
    output
}

#[test]
fn gendyn_ports_are_unique_and_within_max_ports() {
    const _: () = assert!(gendyn::PORT_COUNT <= crate::dsp::ugen::MAX_PORTS);
    let expected: [(&str, usize, f32); 6] = [
        ("freq", gendyn::port::FREQ, 440.0),
        ("gendyn-points", gendyn::port::GENDYN_POINTS, 12.0),
        ("gendyn-amp-step", gendyn::port::GENDYN_AMP_STEP, 0.2),
        ("gendyn-dur-step", gendyn::port::GENDYN_DUR_STEP, 0.1),
        ("gendyn-spread", gendyn::port::GENDYN_SPREAD, 0.1),
        ("gendyn-dist", gendyn::port::GENDYN_DIST, 1.0),
    ];
    assert_eq!(gendyn::PORT_COUNT, expected.len());
    for (index, (name, port, default)) in expected.iter().enumerate() {
        assert_eq!(*port, index, "port index for {name}");
        assert_eq!(gendyn::PORTS[index].0, *name, "port name at {index}");
        assert_eq!(gendyn::PORTS[index].1, *default, "port default at {index}");
        for (other_index, (other_name, _, _)) in expected.iter().enumerate() {
            if index != other_index {
                assert_ne!(name, other_name, "duplicate port name {name}");
            }
        }
    }
}

#[test]
fn gendyn_deterministic_per_seed() {
    let values = [220.0, 12.0, 0.3, 0.2, 0.7, 2.0];
    let first = render(values, 48_000.0, 31, 16_384, 256);
    assert_eq!(first, render(values, 48_000.0, 31, 16_384, 256));
    assert_ne!(first, render(values, 48_000.0, 32, 16_384, 256));
}

#[test]
fn gendyn_spread_zero_locks_period() {
    let output = render([200.0, 12.0, 0.0, 0.0, 0.0, 0.0], 48_000.0, 5, 48_000, 256);
    let settled = &output[9_600..];
    let period_error: f32 = settled
        .iter()
        .zip(&settled[240..])
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / (settled.len() - 240) as f32;
    assert!(
        period_error < 1.0e-6,
        "240-sample period error {period_error}"
    );
    for lag in (1..=241).filter(|lag| *lag != 240) {
        let error: f32 = settled
            .iter()
            .zip(&settled[lag..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / (settled.len() - lag) as f32;
        assert!(error > period_error + 1.0e-5, "lag {lag}: {error}");
    }
}

#[test]
fn gendyn_spread_increases_period_variance() {
    fn variance(spread: f32) -> f32 {
        let store = SampleStore::new(StoreKind::NativeArc);
        let capability = caps();
        let mut stats = FxStats::default();
        let kx = Kx {
            sr: 48_000.0,
            gate: 48_000,
            bank: None,
            store: &store,
            caps: &capability,
            stats: &mut stats,
            seed: 77,
        };
        let ins = inputs([200.0, 12.0, 0.0, 1.0, spread, 0.0]);
        let mut state = NodeState::default();
        let mut memory = vec![0.0; gendyn::STATE_FLOATS];
        let mut output = [0.0; 1];
        let mut count = 0;
        let mut mean = 0.0f64;
        let mut sum_squared_delta = 0.0f64;
        let mut previous_segment = 0;
        for _ in 0..30_000 {
            gendyn::render(&ins, &mut state, &mut memory, &mut output, &kx);
            let segment = gendyn::test_segment_index(&memory);
            if previous_segment > 0 && segment == 0 {
                count += 1;
                let duration = f64::from(gendyn::test_period_duration(&memory, 12));
                let delta = duration - mean;
                mean += delta / count as f64;
                sum_squared_delta += delta * (duration - mean);
            }
            previous_segment = segment;
        }
        assert!(count >= 100);
        (sum_squared_delta / count as f64) as f32
    }
    assert!(variance(0.0) < 1.0e-12);
    assert!(variance(0.8) > 1.0e-8);
}

#[test]
fn gendyn_every_distribution_is_finite() {
    for distribution in 0..=3 {
        let output = render(
            [220.0, 16.0, 1.0, 1.0, 0.8, distribution as f32],
            48_000.0,
            100 + distribution,
            96_000,
            97,
        );
        assert!(output.iter().all(|sample| sample.is_finite()));
        assert!(output.iter().all(|sample| sample.abs() <= 1.0));
    }
}

#[test]
fn gendyn_amp_step_zero_freezes_waveform_after_first_period() {
    let output = render([200.0, 12.0, 0.0, 0.0, 0.0, 1.0], 48_000.0, 8, 48_000, 128);
    let settled = &output[24_000..];
    let error = settled
        .iter()
        .zip(&settled[240..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(error < 1.0e-6, "periodic waveform max error {error}");
}

#[test]
fn gendyn_block_size_independent() {
    let values = [173.0, 19.0, 0.75, 0.5, 0.65, 3.0];
    let reference = render(values, 48_000.0, 49, 8_191, 8_191);
    for block in [64, 97, 128, 256] {
        assert_eq!(
            reference,
            render(values, 48_000.0, 49, 8_191, block),
            "block {block}"
        );
    }
}

#[test]
fn gendyn_extreme_grid_is_finite_and_bounded() {
    for frequency in [1.0, 23_520.0] {
        for points in [3.0, 32.0] {
            for amp_step in [0.0, 1.0] {
                for dur_step in [0.0, 1.0] {
                    for spread in [0.0, 1.0] {
                        for distribution in 0..=3 {
                            let output = render(
                                [
                                    frequency,
                                    points,
                                    amp_step,
                                    dur_step,
                                    spread,
                                    distribution as f32,
                                ],
                                48_000.0,
                                991,
                                2_048,
                                128,
                            );
                            assert!(output.iter().all(|sample| sample.is_finite()));
                            assert!(output.iter().all(|sample| sample.abs() <= 1.0));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn gendyn_render_does_not_allocate() {
    let store = SampleStore::new(StoreKind::NativeArc);
    let capability = caps();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: 48_000.0,
        gate: 256,
        bank: None,
        store: &store,
        caps: &capability,
        stats: &mut stats,
        seed: 13,
    };
    let ins = inputs([220.0, 12.0, 0.4, 0.3, 0.7, 2.0]);
    let mut state = NodeState::default();
    let mut memory = vec![0.0; gendyn::STATE_FLOATS];
    let mut output = [0.0; 256];
    gendyn::render(&ins, &mut state, &mut memory, &mut output, &kx);
    let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
        for _ in 0..8 {
            gendyn::render(&ins, &mut state, &mut memory, &mut output, &kx);
        }
    });
    assert_eq!(allocations, 0);
}
