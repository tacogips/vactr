//! Measured response, decay and automation contracts for original filters.
use super::{caps, rms};
use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::effects::{self, FxCtx, FxState, FxStats};
use crate::dsp::fft::Fft;
use crate::dsp::graph::EffectKind;

fn render(
    kind: EffectKind,
    overrides: &[(&str, f32)],
    sr: f32,
    input: &[f32],
    partition: usize,
) -> (Vec<f32>, Vec<f32>) {
    let defs = effects::params(kind);
    let mut p: Vec<_> = defs.iter().map(|d| d.default).collect();
    for &(name, v) in overrides {
        p[defs.iter().position(|d| d.name == name).unwrap()] = v;
    }
    let cap = caps();
    let mut st = FxState::default();
    let mut mem = vec![0.0; effects::mem_len(kind, sr, &cap)];
    effects::init(kind, &mut st, &mut mem, sr, &cap);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(1024);
    let mut scratch = vec![0.0; 8192];
    let mut analysis = vec![0.0; 1024];
    let mut stats = FxStats::default();
    let mut l = input.to_vec();
    let mut r = vec![0.0; input.len()];
    for (left, right) in l.chunks_mut(partition).zip(r.chunks_mut(partition)) {
        let mut ctx = FxCtx {
            sr,
            store: &store,
            fft: &fft,
            caps: &cap,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
            effects::process(kind, &p, &mut st, &mut mem, left, right, &mut ctx);
        });
        assert_eq!(allocations, 0);
    }
    (l, r)
}
fn response(kind: EffectKind, p: &[(&str, f32)], sr: f32, freq: f32) -> f32 {
    let input: Vec<_> = (0..32768)
        .map(|i| (std::f32::consts::TAU * freq * i as f32 / sr).sin())
        .collect();
    let (out, right) = render(kind, p, sr, &input, 128);
    assert!(right.iter().all(|&x| x == 0.0));
    rms(&out[16384..]) / rms(&input[16384..])
}
#[test]
fn butterworth_orders_match_bilinear_prototype_and_highpass() {
    for sr in [44100.0, 48000.0, 96000.0] {
        for order in [2.0, 4.0, 6.0, 8.0] {
            for high in [0.0, 1.0] {
                for f in [500.0, 1000.0, 2000.0] {
                    let measured = response(
                        EffectKind::ButterworthFilter,
                        &[("cutoff", 1000.0), ("order", order), ("mode", high)],
                        sr,
                        f,
                    );
                    let mut ratio = (std::f32::consts::PI * f / sr).tan()
                        / (std::f32::consts::PI * 1000.0 / sr).tan();
                    if high > 0.0 {
                        ratio = 1.0 / ratio;
                    }
                    let expected = 1.0 / (1.0 + ratio.powf(2.0 * order)).sqrt();
                    assert!(
                        (measured - expected).abs() < 0.004,
                        "sr {sr} order {order} high {high}: {measured} != {expected}"
                    );
                }
            }
        }
    }
}
#[test]
fn chebyshev_passband_has_declared_equiripple_not_butterworth_q() {
    let sr = 48000.0;
    for order in [2.0, 4.0, 6.0, 8.0] {
        for ripple in [0.2, 1.0, 4.0] {
            for high in [0.0, 1.0] {
                for normalized in [0.15f32, 0.4, 0.7, 1.0, 1.5] {
                    let warp = (std::f32::consts::PI * 1000.0 / sr).tan();
                    let f = sr / std::f32::consts::PI
                        * (if high > 0.0 {
                            warp / normalized
                        } else {
                            warp * normalized
                        })
                        .atan();
                    let measured = response(
                        EffectKind::ChebyshevFilter,
                        &[
                            ("cutoff", 1000.0),
                            ("order", order),
                            ("ripple", ripple),
                            ("mode", high),
                        ],
                        sr,
                        f,
                    );
                    let t = if normalized <= 1.0 {
                        (order * normalized.acos()).cos()
                    } else {
                        (order * normalized.acosh()).cosh()
                    };
                    let expected = 1.0 / (1.0 + (10.0f32.powf(ripple / 10.0) - 1.0) * t * t).sqrt();
                    assert!(
                        (measured - expected).abs() < 0.015,
                        "order {order} ripple {ripple} high {high}: {measured} vs {expected}"
                    );
                }
            }
        }
    }
}
#[test]
fn svf_modes_and_allpass_have_their_advertised_response() {
    for sr in [44100.0, 48000.0, 96000.0] {
        let band = response(
            EffectKind::SvfFilter,
            &[("q", 4.0), ("mode", 2.0)],
            sr,
            1000.0,
        );
        let notch = response(
            EffectKind::SvfFilter,
            &[("q", 4.0), ("mode", 3.0)],
            sr,
            1000.0,
        );
        assert!((band - 1.0).abs() < 0.005 && notch < 0.001);
        for f in [80.0, 1000.0, 11000.0] {
            let a = response(EffectKind::AllpassFilter, &[("q", 4.0)], sr, f);
            assert!((a - 1.0).abs() < 0.002);
        }
    }
}
#[test]
fn independent_modal_frequencies_decay_rt60_and_gain() {
    let kind = EffectKind::ParametricResonator;
    for sr in [44100.0, 48000.0, 96000.0] {
        let mut impulse = vec![0.0; (sr * 1.6) as usize];
        impulse[0] = 1.0;
        let p = [
            ("freq-1", 750.0),
            ("decay-1", 0.8),
            ("gain-2", -60.0),
            ("decay-2", 0.01),
            ("decay-3", 0.01),
            ("decay-4", 0.01),
            ("gain-3", -60.0),
            ("gain-4", -60.0),
        ];
        let (out, right) = render(kind, &p, sr, &impulse, 127);
        assert!(right.iter().all(|&x| x == 0.0));
        let width = (sr * 0.04) as usize;
        let start = (sr * 0.1) as usize;
        let later = start + (sr * 0.8) as usize;
        let ratio = rms(&out[later..later + width]) / rms(&out[start..start + width]);
        assert!(
            (ratio.log10() * 20.0 + 60.0).abs() < 0.8,
            "RT60 ratio {ratio} sr {sr}"
        );
        let tuned = response(kind, &p, sr, 750.0);
        let distant = response(kind, &p, sr, 1200.0);
        assert!(tuned > distant * 20.0);
        let quieter = response(
            kind,
            &[
                ("freq-1", 750.0),
                ("gain-1", -6.0),
                ("decay-1", 0.8),
                ("gain-2", -60.0),
                ("decay-2", 0.01),
                ("decay-3", 0.01),
                ("decay-4", 0.01),
                ("gain-3", -60.0),
                ("gain-4", -60.0),
            ],
            sr,
            750.0,
        );
        assert!((quieter / tuned - 0.5012).abs() < 0.004);
    }
}
#[test]
fn comb_modes_have_exact_echo_signatures_and_default_regression() {
    let mut impulse = vec![0.0; 128];
    impulse[0] = 1.0;
    for mode in [0.0, 1.0, 2.0] {
        let (out, _) = render(
            EffectKind::Comb,
            &[("time", 0.001), ("feedback", 0.5), ("mode", mode)],
            48000.0,
            &impulse,
            17,
        );
        let expected = match mode as u32 {
            0 => [1.0, 0.5, 0.25],
            1 => [1.0, 0.5, 0.0],
            _ => [-0.5, 0.75, 0.375],
        };
        for (&value, index) in expected.iter().zip([0, 48, 96]) {
            assert!(
                (out[index] - value).abs() < 1e-5,
                "mode {mode} index {index}: {} vs {value}",
                out[index]
            );
        }
    }
    let defaults: Vec<_> = effects::params(EffectKind::Comb)
        .iter()
        .take(3)
        .map(|d| (d.name, d.default))
        .collect();
    let (a, _) = render(EffectKind::Comb, &defaults, 48000.0, &impulse, 128);
    let (b, _) = render(EffectKind::Comb, &[], 48000.0, &impulse, 128);
    assert_eq!(a, b);
    for f in [100.0, 700.0, 5700.0] {
        assert!(
            (response(
                EffectKind::Comb,
                &[("mode", 2.0), ("feedback", 0.8)],
                48000.0,
                f
            ) - 1.0)
                .abs()
                < 0.002
        );
    }
}
#[test]
fn new_filters_are_partition_independent_stereo_isolated_and_finite_at_limits() {
    let kinds = [
        EffectKind::SvfFilter,
        EffectKind::ButterworthFilter,
        EffectKind::ChebyshevFilter,
        EffectKind::LadderFilter,
        EffectKind::AllpassFilter,
        EffectKind::ParametricResonator,
        EffectKind::FeedbackResonator,
    ];
    let input: Vec<_> = (0..8192)
        .map(|i| if i % 223 == 0 { 0.7 } else { 0.0 })
        .collect();
    for sr in [44100.0, 48000.0, 96000.0] {
        for kind in kinds {
            let (base, right) = render(kind, &[], sr, &input, 8192);
            let (other, _) = render(kind, &[], sr, &input, 31);
            assert_eq!(base, other, "{kind:?} partition");
            assert!(right.iter().all(|&x| x == 0.0));
            assert!(rms(&base) > 1e-7, "{kind:?} silent");
            for upper in [false, true] {
                let p: Vec<_> = effects::params(kind)
                    .iter()
                    .map(|d| (d.name, if upper { d.max } else { d.min }))
                    .collect();
                let (out, right) = render(kind, &p, sr, &input, 63);
                assert!(
                    out.iter()
                        .chain(&right)
                        .all(|x| x.is_finite() && x.abs() < 100.0),
                    "{kind:?} limit"
                );
            }
        }
    }
}

#[test]
fn automated_extreme_controls_preserve_finite_state_without_allocation() {
    for kind in [
        EffectKind::SvfFilter,
        EffectKind::ButterworthFilter,
        EffectKind::ChebyshevFilter,
        EffectKind::LadderFilter,
        EffectKind::AllpassFilter,
        EffectKind::ParametricResonator,
        EffectKind::FeedbackResonator,
        EffectKind::Comb,
    ] {
        let cap = caps();
        let sr = 48000.0;
        let defs = effects::params(kind);
        let mut p: Vec<_> = defs.iter().map(|d| d.default).collect();
        let mut mem = vec![0.0; effects::mem_len(kind, sr, &cap)];
        let mut st = FxState::default();
        effects::init(kind, &mut st, &mut mem, sr, &cap);
        let store = SampleStore::new(StoreKind::NativeArc);
        let fft = Fft::new(1024);
        let mut scratch = vec![0.0; 8192];
        let mut analysis = vec![0.0; 1024];
        let mut stats = FxStats::default();
        for block in 0..300 {
            for (v, d) in p.iter_mut().zip(defs) {
                *v = if block % 2 == 0 { d.max } else { d.min };
            }
            let mut l = [0.0; 64];
            let mut r = [0.0; 64];
            l[0] = 0.5;
            r[11] = -0.25;
            let mut ctx = FxCtx {
                sr,
                store: &store,
                fft: &fft,
                caps: &cap,
                scratch: &mut scratch,
                analysis: &mut analysis,
                stats: &mut stats,
            };
            let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
                effects::process(kind, &p, &mut st, &mut mem, &mut l, &mut r, &mut ctx)
            });
            assert_eq!(allocations, 0);
            assert!(
                l.iter().chain(&r).all(|x| x.is_finite() && x.abs() < 100.0),
                "{kind:?} automation block {block}"
            );
        }
    }
}

#[test]
fn voice_comb_modes_and_damping_match_stereo_effect_math() {
    use crate::dsp::ugen::{filter, Inp, Kx, NodeState, MAX_PORTS};
    let mut input = [0.0; 192];
    input[0] = 1.0;
    for mode in [0.0, 1.0, 2.0] {
        for damping in [0.0, 0.7] {
            let mut ins = [Inp::Val(0.0); MAX_PORTS];
            ins[0] = Inp::Buf(&input);
            ins[1] = Inp::Val(0.001);
            ins[2] = Inp::Val(0.5);
            ins[3] = Inp::Val(mode);
            ins[4] = Inp::Val(damping);
            let cap = caps();
            let store = SampleStore::new(StoreKind::NativeArc);
            let mut stats = FxStats::default();
            let kx = Kx {
                sr: 48000.0,
                gate: 192,
                bank: None,
                store: &store,
                caps: &cap,
                stats: &mut stats,
                seed: 1,
            };
            let mut state = NodeState::default();
            let mut memory = [0.0; 4800];
            let mut out = [0.0; 192];
            let (_, allocations) = crate::dsp::alloc_probe::armed(|| {
                filter::comb(&ins, &mut state, &mut memory, &mut out, &kx)
            });
            assert_eq!(allocations, 0);
            let (bus, _) = render(
                EffectKind::Comb,
                &[
                    ("time", 0.001),
                    ("feedback", 0.5),
                    ("mode", mode),
                    ("damping", damping),
                ],
                48000.0,
                &input,
                192,
            );
            assert_eq!(
                out.as_slice(),
                bus.as_slice(),
                "mode {mode} damping {damping}"
            );
        }
    }
}
