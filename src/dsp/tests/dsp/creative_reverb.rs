//! Measured DSP behavior of original creative reverbs and delays.
use super::caps;
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::effects::{self, FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::EffectKind;

const KINDS: [EffectKind; 7] = [
    EffectKind::EarlyReflections,
    EffectKind::SchroederReverb,
    EffectKind::SpringReverb,
    EffectKind::SpaceReverb,
    EffectKind::ShimmerReverb,
    EffectKind::TapeDelay,
    EffectKind::DiffusionDelay,
];

fn render(
    kind: EffectKind,
    named: &[(&str, f32)],
    sr: f32,
    seconds: f32,
    block: usize,
    mode: u8,
) -> (Vec<f32>, Vec<f32>) {
    let defs = effects::params(kind);
    let mut p: Vec<_> = defs.iter().map(|d| d.default).collect();
    for &(name, v) in named {
        p[defs.iter().position(|d| d.name == name).unwrap()] = v;
    }
    let cap = caps();
    let mut mem = vec![0.0; effects::mem_len(kind, sr, &cap)];
    let mut st = FxState::default();
    effects::init(kind, &mut st, &mut mem, sr, &cap);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let mut scratch = vec![0.0; 4096];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let n = (seconds * sr) as usize;
    let mut left = vec![0.0; n];
    let mut right = vec![0.0; n];
    if mode == 0 {
        left[0] = 1.0;
    } else {
        for (i, x) in left.iter_mut().enumerate().take((sr * 0.3) as usize) {
            *x = (i as f32 * std::f32::consts::TAU * 400.0 / sr).sin() * 0.2;
        }
    }
    for offset in (0..n).step_by(block) {
        if mode == 2 && offset > sr as usize {
            p[defs.iter().position(|d| d.name == "freeze").unwrap()] = 1.0;
        }
        if mode == 4 {
            for (i, d) in defs.iter().enumerate() {
                p[i] = if (offset / block) % 2 == 0 {
                    d.min
                } else {
                    d.max
                };
            }
        }
        let end = (offset + block).min(n);
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        let (_, allocs) = armed(|| {
            effects::process(
                kind,
                &p,
                &mut st,
                &mut mem,
                &mut left[offset..end],
                &mut right[offset..end],
                &mut ctx,
            )
        });
        assert_eq!(allocs, 0);
    }
    (left, right)
}
fn energy(x: &[f32]) -> f32 {
    x.iter().map(|v| v * v).sum()
}

#[test]
fn creative_defaults_are_audible_stereo_independent_partition_invariant_and_allocation_free() {
    for kind in KINDS {
        let a = render(kind, &[], 48000.0, 2.0, 127, 0);
        let b = render(kind, &[], 48000.0, 2.0, 256, 0);
        assert!(energy(&a.0) > 1e-7, "{kind:?}");
        assert_eq!(energy(&a.1), 0.0, "{kind:?} leaks");
        assert_eq!(a, b, "{kind:?} partition");
    }
}
#[test]
fn creative_sample_rates_and_valid_extremes_are_finite() {
    for sr in [44100.0, 48000.0, 96000.0] {
        for kind in KINDS {
            for high in [false, true] {
                let values: Vec<_> = effects::params(kind)
                    .iter()
                    .map(|d| (d.name, if high { d.max } else { d.min }))
                    .collect();
                let out = render(kind, &values, sr, 0.8, 257, 1);
                assert!(
                    out.0
                        .iter()
                        .chain(&out.1)
                        .all(|x| x.is_finite() && x.abs() < 20.0),
                    "{kind:?} {sr} {high}"
                );
            }
        }
    }
}
#[test]
fn early_reflections_follow_image_source_distance_and_wall_absorption() {
    let a = render(EffectKind::EarlyReflections, &[], 48000.0, 0.25, 128, 0).0;
    // Defaults: nearest floor/ceiling image is sqrt(2.8²+1²+3²) meters.
    let expected = ((2.8f32.powi(2) + 1.0 + 9.0).sqrt() / 343.0 * 48000.0).floor() as usize;
    let first = a.iter().position(|x| x.abs() > 1e-8).unwrap();
    assert!(
        first.abs_diff(expected) <= 2,
        "first={first} expected={expected}"
    );
    let absorbed = render(
        EffectKind::EarlyReflections,
        &[("wall-loss", 1.0)],
        48000.0,
        0.25,
        128,
        0,
    )
    .0;
    assert_eq!(energy(&absorbed), 0.0);
}
#[test]
fn spring_dispersion_changes_chirp_and_decay_control_extends_tail() {
    let dry = render(
        EffectKind::SpringReverb,
        &[("dispersion", 0.0)],
        48000.0,
        2.0,
        128,
        0,
    )
    .0;
    let dispersed = render(
        EffectKind::SpringReverb,
        &[("dispersion", 1.0)],
        48000.0,
        2.0,
        128,
        0,
    )
    .0;
    assert!(
        dry.iter()
            .zip(&dispersed)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            > 0.5
    );
    let short = render(
        EffectKind::SpringReverb,
        &[("decay", 0.2)],
        48000.0,
        2.0,
        128,
        0,
    )
    .0;
    assert!(energy(&dispersed[48000..]) > energy(&short[48000..]) * 100.0);
}
#[test]
fn schroeder_rt60_has_measurable_decay() {
    let short = render(
        EffectKind::SchroederReverb,
        &[("decay", 0.3), ("damping", 0.0)],
        48000.0,
        3.0,
        128,
        0,
    )
    .0;
    let long = render(
        EffectKind::SchroederReverb,
        &[("decay", 3.0), ("damping", 0.0)],
        48000.0,
        3.0,
        128,
        0,
    )
    .0;
    assert!(energy(&long[48000..]) > energy(&short[48000..]) * 1000.0);
    assert!(energy(&long[96000..]) < energy(&long[24000..72000]));
}
#[test]
fn negative_gravity_suppresses_onset_and_then_opens_swelling_tail() {
    let normal = render(
        EffectKind::SpaceReverb,
        &[("modulation", 0.0), ("gravity", 1.0)],
        48000.0,
        3.0,
        128,
        0,
    )
    .0;
    let swell = render(
        EffectKind::SpaceReverb,
        &[("modulation", 0.0), ("gravity", -1.0)],
        48000.0,
        3.0,
        128,
        0,
    )
    .0;
    let early = energy(&swell[..12000]) / energy(&normal[..12000]);
    let late = energy(&swell[96000..]) / energy(&normal[96000..]);
    assert!(early < 0.1, "{early}");
    assert!(late > early * 4.0, "{early} {late}");
}
#[test]
fn freeze_silence_stays_silent_and_existing_tail_retains_energy() {
    let empty = render(
        EffectKind::SpaceReverb,
        &[("freeze", 1.0)],
        48000.0,
        2.0,
        128,
        0,
    )
    .0;
    assert_eq!(energy(&empty), 0.0);
    let frozen = render(
        EffectKind::SpaceReverb,
        &[("modulation", 0.0)],
        48000.0,
        12.0,
        128,
        2,
    )
    .0;
    let a = energy(&frozen[96000..288000]);
    let b = energy(&frozen[384000..576000]);
    assert!(b / a > 0.7 && b / a < 1.3, "ratio {}", b / a);
}
#[test]
fn shimmer_resampler_measures_semitone_pitch_ratios() {
    use crate::dsp::effects::creative_reverb::{common::Bank, diffuse::shifted};
    let sr = 48000.0;
    for semitone in [-12.0f32, 0.0, 12.0] {
        let ratio = 2f32.powf(semitone / 12.0);
        let bank = Bank::new(0.1, sr);
        let mut mem = vec![0.0; bank.len(1)];
        let mut phase = 0.0;
        let mut output = vec![0.0; 48000];
        for (i, x) in output.iter_mut().enumerate() {
            *x = shifted(
                bank,
                &mut mem,
                0,
                (i as f32 * std::f32::consts::TAU * 400.0 / sr).sin(),
                phase,
                0.05 * sr,
            );
            phase = (phase + (1.0 - ratio) / (0.05 * sr)).rem_euclid(1.0);
        }
        let power = |hz: f32| {
            let mut re = 0.0;
            let mut im = 0.0;
            for (i, x) in output.iter().enumerate().skip(12000) {
                let phase = i as f32 * std::f32::consts::TAU * hz / sr;
                re += x * phase.cos();
                im += x * phase.sin();
            }
            re * re + im * im
        };
        assert!(
            power(400.0 * ratio) > power(400.0 * ratio + 100.0) * 100.0,
            "shift {semitone}"
        );
    }
}
#[test]
fn shimmer_changes_tail_only_when_pitch_is_fed_back() {
    let a = render(
        EffectKind::ShimmerReverb,
        &[("feedback", 0.0), ("shift", 12.0)],
        48000.0,
        3.0,
        128,
        1,
    )
    .0;
    let b = render(
        EffectKind::ShimmerReverb,
        &[("feedback", 0.0), ("shift", -12.0)],
        48000.0,
        3.0,
        128,
        1,
    )
    .0;
    assert_eq!(a, b);
    let c = render(
        EffectKind::ShimmerReverb,
        &[("feedback", 0.8), ("shift", 12.0)],
        48000.0,
        3.0,
        128,
        1,
    )
    .0;
    assert!(
        a[48000..]
            .iter()
            .zip(&c[48000..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            > 0.01
    );
}
#[test]
fn tape_heads_and_diffusion_have_distinct_echo_structure() {
    let tape = render(
        EffectKind::TapeDelay,
        &[("time", 0.1), ("feedback", 0.0), ("wow", 0.0)],
        48000.0,
        0.4,
        128,
        0,
    )
    .0;
    for (position, gain) in [(4800, 0.5), (9600, 0.3), (14400, 0.2)] {
        assert!((tape[position] - gain).abs() < 1e-5);
    }
    let plain = render(
        EffectKind::DiffusionDelay,
        &[("time", 0.1), ("diffusion", 0.0), ("feedback", 0.0)],
        48000.0,
        0.4,
        128,
        0,
    )
    .0;
    let diffuse = render(
        EffectKind::DiffusionDelay,
        &[("time", 0.1), ("diffusion", 0.8), ("feedback", 0.0)],
        48000.0,
        0.4,
        128,
        0,
    )
    .0;
    assert!(
        diffuse.iter().filter(|x| x.abs() > 1e-6).count()
            > plain.iter().filter(|x| x.abs() > 1e-6).count() * 10
    );
}

fn band_energy(samples: &[f32], sr: f32, center: f32) -> f32 {
    // Integrate nearby FDN modes instead of sampling a single sparse resonant bin.
    (0..21)
        .map(|k| {
            let hz = center - 10.0 + k as f32;
            let mut re = 0.0;
            let mut im = 0.0;
            for (i, x) in samples.iter().enumerate() {
                let phi = i as f32 * std::f32::consts::TAU * hz / sr;
                re += x * phi.cos();
                im += x * phi.sin();
            }
            re * re + im * im
        })
        .sum()
}
#[test]
fn shimmer_recirculation_moves_tail_energy_by_an_octave() {
    let settings = [("feedback", 0.75), ("damping", 0.0), ("decay", 15.0)];
    let mut up = settings.to_vec();
    up.push(("shift", 12.0));
    let mut down = settings.to_vec();
    down.push(("shift", -12.0));
    let a = render(EffectKind::ShimmerReverb, &up, 48000.0, 2.0, 128, 1).0;
    let b = render(EffectKind::ShimmerReverb, &down, 48000.0, 2.0, 128, 1).0;
    let up800 = band_energy(&a[24000..72000], 48000.0, 800.0);
    let down800 = band_energy(&b[24000..72000], 48000.0, 800.0);
    let up200 = band_energy(&a[24000..72000], 48000.0, 200.0);
    let down200 = band_energy(&b[24000..72000], 48000.0, 200.0);
    assert!(up800 > down800 * 4.0, "up800={up800} down800={down800}");
    assert!(down200 > up200 * 4.0, "down200={down200} up200={up200}");
}
#[test]
fn positive_gravity_controls_forward_decay() {
    let a = render(
        EffectKind::SpaceReverb,
        &[("gravity", 0.0), ("modulation", 0.0)],
        48000.0,
        3.0,
        128,
        0,
    )
    .0;
    let b = render(
        EffectKind::SpaceReverb,
        &[("gravity", 1.0), ("modulation", 0.0)],
        48000.0,
        3.0,
        128,
        0,
    )
    .0;
    assert!(energy(&b[48000..]) > energy(&a[48000..]) * 3.0);
}
#[test]
fn creative_controls_change_output_and_extreme_automation_remains_finite() {
    for kind in KINDS {
        let base = render(kind, &[], 48000.0, 2.0, 128, 0).0;
        for d in effects::params(kind).iter().filter(|d| d.name != "mix") {
            let out = render(
                kind,
                &[(d.name, if d.default == d.max { d.min } else { d.max })],
                48000.0,
                2.0,
                128,
                0,
            )
            .0;
            let difference = base
                .iter()
                .zip(out)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>();
            assert!(difference > 1e-5, "{kind:?} {} has no effect", d.name);
        }
        let automated = render(kind, &[], 48000.0, 2.0, 127, 4);
        assert!(
            automated
                .0
                .iter()
                .chain(&automated.1)
                .all(|x| x.is_finite() && x.abs() < 20.0),
            "{kind:?}"
        );
    }
}
