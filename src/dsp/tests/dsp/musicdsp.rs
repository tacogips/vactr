//! Independent MusicDSP algorithms: nonlinear curves, complex history and FIR selection.
use super::{caps, noise, rms};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::cells::AtomicCells;
use crate::dsp::effects::{
    self,
    musicdsp::{clip_sample, fold_sample, FIR_TAPS},
    FxCtx, FxStats, FxUnit,
};
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::EffectKind;
use crate::host::wire::Ctl;

const KINDS: [EffectKind; 4] = [
    EffectKind::Foldback,
    EffectKind::VariableClip,
    EffectKind::AlienWah,
    EffectKind::DynamicConvolution,
];

fn render(
    kind: EffectKind,
    controls: &[(&str, f32)],
    left: &[f32],
    right: &[f32],
    sr: f32,
    block: usize,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let capabilities = caps();
    let cells = AtomicCells::new(4);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let mut memory = vec![0.0; effects::mem_len(kind, sr, &capabilities)];
    let mut unit = FxUnit::empty();
    let mut given: Vec<_> = controls
        .iter()
        .map(|(name, value)| (effects::param_ctl(kind, name).unwrap(), Ctl::Const(*value)))
        .collect();
    if !controls.iter().any(|(name, _)| *name == "mix") {
        given.push((effects::param_ctl(kind, "mix").unwrap(), Ctl::Const(1.0)));
    }
    unit.configure(kind, &given, &cells, &mut memory, sr, &capabilities);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; 1024];
    let mut dry = vec![0.0; 2 * block];
    let mut stats = FxStats::default();
    let (mut output_l, mut output_r) = (left.to_vec(), right.to_vec());
    for (l, r) in output_l.chunks_mut(block).zip(output_r.chunks_mut(block)) {
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &capabilities,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        let (_, allocations) = armed(|| unit.run(&mut memory, l, r, &mut dry, &mut ctx));
        assert_eq!(allocations, 0, "{} callback allocation", kind.name());
    }
    (output_l, output_r, memory)
}
#[test]
fn reflection_is_periodic_odd_bounded_and_identity_inside_threshold() {
    let threshold = 0.5;
    for (x, expected) in [
        (0.0, 0.0),
        (0.25, 0.25),
        (0.5, 0.5),
        (0.75, 0.25),
        (1.0, 0.0),
        (1.5, -0.5),
        (2.25, 0.25),
    ] {
        assert!((fold_sample(x, threshold) - expected).abs() < 1e-6);
        assert!((fold_sample(-x, threshold) + expected).abs() < 1e-6);
    }
    for i in -1000..1000 {
        let x = i as f32 * 0.01;
        assert!(fold_sample(x, threshold).abs() <= threshold);
        assert!(
            (fold_sample(x, threshold) - fold_sample(x + 4.0 * threshold, threshold)).abs() < 2e-6
        );
    }
}
#[test]
fn clip_hardness_continuously_changes_transfer_without_overflow() {
    assert!((clip_sample(0.5, 0.0) - 1.0 / 3.0).abs() < 1e-6);
    assert!((clip_sample(0.5, 1.0) - 0.5).abs() < 1e-6);
    let mut previous = 0.0;
    for i in 0..101 {
        let hard = i as f32 / 100.0;
        let sample = clip_sample(0.8, hard);
        assert!(sample >= previous);
        previous = sample;
        for input in [0.0, 1e-6, 0.5, 2.0, 128.0, f32::MAX] {
            let y = clip_sample(input, hard);
            assert!(y.is_finite() && y <= 1.0);
            assert_eq!(clip_sample(-input, hard), -y);
        }
    }
}
#[test]
fn alien_wah_stores_imaginary_history_and_has_independent_stereo_lanes() {
    let left = noise(4096, 0.5, 27);
    let silence = vec![0.0; left.len()];
    let (wet, right, memory) = render(EffectKind::AlienWah, &[], &left, &silence, 48000.0, 128);
    assert!(rms(&wet) > 1e-3);
    assert!(right.iter().all(|x| *x == 0.0));
    let lane_length = memory.len() / 4;
    assert!(
        memory[lane_length..2 * lane_length]
            .iter()
            .any(|x| x.abs() > 1e-4),
        "complex imaginary state participates"
    );
    let (plain, _, _) = render(
        EffectKind::AlienWah,
        &[("feedback", 0.0)],
        &left,
        &silence,
        48000.0,
        128,
    );
    assert_eq!(plain, left);
}
#[test]
fn dynamic_convolution_retains_each_inputs_amplitude_region_in_history() {
    let kind = EffectKind::DynamicConvolution;
    let controls = [("drive", 0.0), ("sweep", 5.0)];
    let mut low = vec![0.0; 512];
    let mut high = low.clone();
    let silence = low.clone();
    low[0] = 0.1;
    high[20] = 0.9;
    let combined: Vec<_> = low.iter().zip(&high).map(|(l, h)| l + h).collect();
    let (a, _, _) = render(kind, &controls, &low, &silence, 48000.0, 128);
    let (b, _, _) = render(kind, &controls, &high, &silence, 48000.0, 128);
    let (sum, _, _) = render(kind, &controls, &combined, &silence, 48000.0, 128);
    for i in 0..sum.len() {
        assert!(
            (sum[i] - a[i] - b[i]).abs() < 1e-7,
            "historical selection at {i}"
        );
    }
    // The louder input's normalized kernel is different from the quiet kernel.
    let difference: f32 = (0..FIR_TAPS)
        .map(|i| (a[i] / 0.1 - b[i + 20] / 0.9).abs())
        .sum();
    assert!(difference > 0.3, "bank selected by amplitude: {difference}");
    assert!(
        a[FIR_TAPS..].iter().all(|x| *x == 0.0),
        "true finite FIR history"
    );
}
#[test]
fn all_controls_change_audio_and_extremes_are_finite_at_supported_rates() {
    let input = noise(4096, 0.75, 49);
    for sr in [44100.0, 48000.0, 96000.0] {
        for kind in KINDS {
            let (default, _, _) = render(kind, &[], &input, &input, sr, 256);
            assert!(
                rms(&default) > 1e-5,
                "{}/{sr} audible defaults",
                kind.name()
            );
            for param in effects::params(kind) {
                let (low, low_r, _) =
                    render(kind, &[(param.name, param.min)], &input, &input, sr, 256);
                let (high, high_r, _) =
                    render(kind, &[(param.name, param.max)], &input, &input, sr, 256);
                assert!(
                    low.iter()
                        .chain(&high)
                        .chain(&low_r)
                        .chain(&high_r)
                        .all(|x| x.is_finite() && x.abs() <= 32.0),
                    "{}/{}/{sr}",
                    kind.name(),
                    param.name
                );
                let diff: f32 = low
                    .iter()
                    .zip(&high)
                    .chain(low_r.iter().zip(&high_r))
                    .map(|(a, b)| (a - b).abs())
                    .sum();
                assert!(
                    diff > 0.001,
                    "{}/{} changes audio: {diff}",
                    kind.name(),
                    param.name
                );
            }
        }
    }
}
#[test]
fn fixed_controls_are_partition_invariant_and_mix_zero_is_transparent() {
    let input = noise(4096, 0.6, 20);
    let right = noise(4096, 0.3, 21);
    for kind in KINDS {
        let (expected_l, expected_r, _) = render(kind, &[], &input, &right, 48000.0, 1024);
        for block in [17, 64, 128, 256] {
            let (l, r, _) = render(kind, &[], &input, &right, 48000.0, block);
            assert_eq!(l, expected_l, "{}/{block}", kind.name());
            assert_eq!(r, expected_r, "{}/{block}", kind.name());
        }
        let (l, r, _) = render(kind, &[("mix", 0.0)], &input, &right, 48000.0, 128);
        assert_eq!(l, input);
        assert_eq!(r, right);
    }
}

#[test]
fn musicdsp_extreme_control_automation_is_bounded_without_callback_allocation() {
    let capabilities = caps();
    let cells = AtomicCells::new(4);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; 1024];
    let mut dry = vec![0.0; 256];
    let mut stats = FxStats::default();
    let input = noise(128, 0.6, 90);
    for kind in KINDS {
        let mut memory = vec![0.0; effects::mem_len(kind, 48000.0, &capabilities)];
        let mut unit = FxUnit::empty();
        unit.configure(kind, &[], &cells, &mut memory, 48000.0, &capabilities);
        for step in 0..24 {
            for (index, param) in effects::params(kind).iter().enumerate() {
                unit.set(index, if step % 2 == 0 { param.min } else { param.max });
            }
            let (mut l, mut r) = (input.clone(), input.clone());
            let mut ctx = FxCtx {
                sr: 48000.0,
                store: &store,
                fft: &fft,
                caps: &capabilities,
                scratch: &mut scratch,
                analysis: &mut analysis,
                stats: &mut stats,
            };
            let (_, allocations) =
                armed(|| unit.run(&mut memory, &mut l, &mut r, &mut dry, &mut ctx));
            assert_eq!(allocations, 0);
            assert!(
                l.iter().chain(&r).all(|x| x.is_finite() && x.abs() <= 32.0),
                "{} automation",
                kind.name()
            );
        }
    }
}
