//! Original state-variable, prototype-pole and nonlinear ladder filters.
//! Butterworth and type-I Chebyshev poles are mapped by the bilinear transform;
//! no MusicDSP snippets are copied. Orders are even, from two to eight.
use std::f32::consts::PI;

use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

const SVF: &[ParamDef] = &[
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 0.707, 0.1, 30.0),
    ParamDef::new("mode", 0.0, 0.0, 4.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const BUTTER: &[ParamDef] = &[
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("order", 4.0, 2.0, 8.0),
    ParamDef::new("mode", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const CHEBY: &[ParamDef] = &[
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("order", 4.0, 2.0, 8.0),
    ParamDef::unit("ripple", 1.0, 0.05, 6.0, "dB"),
    ParamDef::new("mode", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const LADDER: &[ParamDef] = &[
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("res", 0.3, 0.0, 1.0),
    ParamDef::new("drive", 1.0, 0.1, 12.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const ALLPASS: &[ParamDef] = &[
    ParamDef::unit("cutoff", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("q", 0.707, 0.1, 30.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    match kind {
        EffectKind::SvfFilter => SVF,
        EffectKind::ButterworthFilter => BUTTER,
        EffectKind::ChebyshevFilter => CHEBY,
        EffectKind::LadderFilter => LADDER,
        EffectKind::AllpassFilter => ALLPASS,
        _ => &[],
    }
}
pub(super) fn mem_len(_kind: EffectKind, _sr: f32) -> usize {
    0
}
pub(super) fn init(_kind: EffectKind, _st: &mut FxState, _len: usize, _sr: f32) {}

fn value(p: &[f32], i: usize, fallback: f32, lo: f32, hi: f32) -> f32 {
    let x = p.get(i).copied().unwrap_or(fallback);
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        fallback.clamp(lo, hi)
    }
}

// A topology-preserving integrator pair: rotations remain stable when tuned.
fn svf(x: f32, s: &mut [f32], g: f32, k: f32, mode: u32) -> f32 {
    let a = 1.0 / (1.0 + g * (g + k));
    let v1 = a * (s[0] + g * (x - s[1]));
    let v2 = s[1] + g * v1;
    s[0] = 2.0 * v1 - s[0];
    s[1] = 2.0 * v2 - s[1];
    let high = x - k * v1 - v2;
    match mode {
        1 => high,
        2 => k * v1,
        3 => high + v2,
        4 => v2 - high,
        5 => x - 2.0 * k * v1,
        _ => v2,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    _mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let sr = ctx.sr.max(100.0);
    let cutoff = value(p, 0, 1000.0, 1.0, 0.45 * sr);
    let g = (PI * cutoff / sr).tan();
    match kind {
        EffectKind::SvfFilter => {
            let k = 1.0 / value(p, 1, 0.707, 0.1, 30.0);
            let mode = value(p, 2, 0.0, 0.0, 4.0).round() as u32;
            for (a, b) in l.iter_mut().zip(r) {
                *a = svf(*a, &mut st.s[..2], g, k, mode);
                *b = svf(*b, &mut st.s[2..4], g, k, mode);
            }
        }
        EffectKind::ButterworthFilter | EffectKind::ChebyshevFilter => {
            let count = (value(p, 1, 4.0, 2.0, 8.0) / 2.0).round() as usize;
            let order = count * 2;
            let cheby = kind == EffectKind::ChebyshevFilter;
            let ripple = f64::from(value(p, 2, 1.0, 0.05, 6.0));
            let epsilon = (10.0f64.powf(ripple / 10.0) - 1.0).sqrt();
            let mu = (1.0 / epsilon).asinh() / order as f64;
            let high = value(p, if cheby { 3 } else { 2 }, 0.0, 0.0, 1.0) >= 0.5;
            let mut coefficients = [[0.0; 2]; 4];
            for (j, c) in coefficients[..count].iter_mut().enumerate() {
                let theta = std::f64::consts::PI * (2 * j + 1) as f64 / (2 * order) as f64;
                let (re, im) = if cheby {
                    (-mu.sinh() * theta.sin(), mu.cosh() * theta.cos())
                } else {
                    (-theta.sin(), theta.cos())
                };
                let norm = (re * re + im * im).sqrt();
                *c = [
                    (if high {
                        f64::from(g) / norm
                    } else {
                        f64::from(g) * norm
                    }) as f32,
                    (-2.0 * re / norm) as f32,
                ];
            }
            // Even-order type I has -ripple dB at DC (or Nyquist for HP).
            let gain = if cheby {
                10.0f32.powf(-(ripple as f32) / 20.0)
            } else {
                1.0
            };
            for (a, b) in l.iter_mut().zip(r) {
                let (mut x, mut y) = (*a, *b);
                for (j, c) in coefficients[..count].iter().enumerate() {
                    x = svf(x, &mut st.s[2 * j..2 * j + 2], c[0], c[1], u32::from(high));
                    y = svf(
                        y,
                        &mut st.s[8 + 2 * j..10 + 2 * j],
                        c[0],
                        c[1],
                        u32::from(high),
                    );
                }
                *a = x * gain;
                *b = y * gain;
            }
        }
        EffectKind::LadderFilter => {
            let feedback = 3.8 * value(p, 1, 0.3, 0.0, 1.0);
            let drive = value(p, 2, 1.0, 0.1, 12.0);
            // Four substeps retain bounded nonlinear feedback at high cutoff.
            let a = 1.0 - (-2.0 * PI * cutoff / (4.0 * sr)).exp();
            for (x, y) in l.iter_mut().zip(r) {
                for (ch, input) in [(0, *x), (1, *y)] {
                    let s = &mut st.s[4 * ch..4 * ch + 4];
                    for _ in 0..4 {
                        let mut v = (input * drive - feedback * s[3]).tanh();
                        for state in s.iter_mut() {
                            *state += a * (v - state.tanh());
                            v = state.tanh();
                        }
                    }
                }
                *x = st.s[3].tanh() / drive;
                *y = st.s[7].tanh() / drive;
            }
        }
        EffectKind::AllpassFilter => {
            let q = value(p, 1, 0.707, 0.1, 30.0);
            for (a, b) in l.iter_mut().zip(r) {
                *a = svf(*a, &mut st.s[..2], g, 1.0 / q, 5);
                *b = svf(*b, &mut st.s[2..4], g, 1.0 / q, 5);
            }
        }
        _ => {}
    }
}
