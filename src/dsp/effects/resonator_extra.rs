//! Independently parameterized modal rotations and nonlinear body feedback.
//! Rotation state is energy stable under tuning changes; decay is RT60.
use super::prim::{DelayLine, OnePole};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;
use std::f32::consts::{LN_10, TAU};

const PARAMETRIC: &[ParamDef] = &[
    ParamDef::unit("freq-1", 110.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("decay-1", 1.5, 0.01, 30.0, "s"),
    ParamDef::unit("gain-1", 0.0, -60.0, 12.0, "dB"),
    ParamDef::unit("freq-2", 237.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("decay-2", 1.1, 0.01, 30.0, "s"),
    ParamDef::unit("gain-2", -6.0, -60.0, 12.0, "dB"),
    ParamDef::unit("freq-3", 431.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("decay-3", 0.8, 0.01, 30.0, "s"),
    ParamDef::unit("gain-3", -9.0, -60.0, 12.0, "dB"),
    ParamDef::unit("freq-4", 791.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("decay-4", 0.5, 0.01, 30.0, "s"),
    ParamDef::unit("gain-4", -12.0, -60.0, 12.0, "dB"),
    ParamDef::new("mix", 0.5, 0.0, 1.0),
];
const FEEDBACK: &[ParamDef] = &[
    ParamDef::unit("freq", 220.0, 20.0, 8000.0, "Hz"),
    ParamDef::new("body-q", 4.0, 0.5, 30.0),
    ParamDef::new("feedback", 0.7, 0.0, 0.98),
    ParamDef::unit("damping", 5000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::unit("distance", 0.01, 0.0001, 0.24, "s"),
    ParamDef::new("drive", 2.0, 0.1, 20.0),
    ParamDef::new("mix", 0.5, 0.0, 1.0),
];
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    match kind {
        EffectKind::ParametricResonator => PARAMETRIC,
        EffectKind::FeedbackResonator => FEEDBACK,
        _ => &[],
    }
}
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    if kind == EffectKind::FeedbackResonator {
        2 * ((sr.max(1.0) * 0.24).ceil() as usize + 4)
    } else {
        0
    }
}
pub(super) fn init(kind: EffectKind, st: &mut FxState, len: usize, _sr: f32) {
    if kind == EffectKind::FeedbackResonator {
        let mut cursor = 0;
        st.dl[0] = DelayLine::carve(&mut cursor, len, len / 2);
        st.dl[1] = DelayLine::carve(&mut cursor, len, len - len / 2);
    }
}
fn value(p: &[f32], i: usize, default: f32, lo: f32, hi: f32) -> f32 {
    let x = p.get(i).copied().unwrap_or(default);
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        default.clamp(lo, hi)
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let sr = ctx.sr.max(100.0);
    match kind {
        EffectKind::ParametricResonator => {
            let mut modes = [[0.0; 4]; 4];
            for (m, c) in modes.iter_mut().enumerate() {
                let freq = value(p, 3 * m, PARAMETRIC[3 * m].default, 1.0, 0.45 * sr);
                let decay = value(p, 3 * m + 1, PARAMETRIC[3 * m + 1].default, 0.01, 30.0);
                let gain = 10.0f32
                    .powf(value(p, 3 * m + 2, PARAMETRIC[3 * m + 2].default, -60.0, 12.0) / 20.0);
                let radius = (-3.0 * LN_10 / (sr * decay)).exp().min(1.0 - f32::EPSILON);
                let (sin, cos) = (TAU * freq / sr).sin_cos();
                // L1-bounded excitation gives a resonant transfer gain <= gain.
                *c = [radius * cos, radius * sin, 1.0 - radius, gain];
            }
            for (a, b) in l.iter_mut().zip(r) {
                let mut wet = [0.0; 2];
                for (ch, input) in [(0, *a), (1, *b)] {
                    for (m, c) in modes.iter().enumerate() {
                        let s = &mut st.s[8 * ch + 2 * m..8 * ch + 2 * m + 2];
                        let real = c[0] * s[0] - c[1] * s[1] + c[2] * input;
                        let imag = c[1] * s[0] + c[0] * s[1];
                        s[0] = real;
                        s[1] = imag;
                        wet[ch] += real * c[3];
                    }
                }
                *a = wet[0] * 0.5;
                *b = wet[1] * 0.5;
            }
        }
        EffectKind::FeedbackResonator => {
            let freq = value(p, 0, 220.0, 1.0, 0.45 * sr);
            let k = 1.0 / value(p, 1, 4.0, 0.5, 30.0);
            let g = (std::f32::consts::PI * freq / sr).tan();
            let feedback = value(p, 2, 0.7, 0.0, 0.98);
            let a = OnePole::coef(value(p, 3, 5000.0, 1.0, 0.45 * sr), sr);
            let delay = value(p, 4, 0.01, 0.0001, 0.24) * sr;
            let drive = value(p, 5, 2.0, 0.1, 20.0);
            for (x, y) in l.iter_mut().zip(r) {
                let mut output = [0.0; 2];
                for (ch, input) in [(0, *x), (1, *y)] {
                    let loop_signal = st.op[ch].lp(st.dl[ch].read(mem, delay), a);
                    let excitation = (drive * (input + feedback * loop_signal)).tanh() / drive;
                    let s = &mut st.s[2 * ch..2 * ch + 2];
                    let v1 = (s[0] + g * (excitation - s[1])) / (1.0 + g * (g + k));
                    let v2 = s[1] + g * v1;
                    s[0] = 2.0 * v1 - s[0];
                    s[1] = 2.0 * v2 - s[1];
                    // Constant-peak bandpass body; bounded nonlinear amplifier
                    // and a lossy physical distance loop, unlike horn's pitch delay.
                    output[ch] = k * v1;
                    st.dl[ch].write(mem, output[ch].tanh());
                }
                *x = output[0];
                *y = output[1];
            }
        }
        _ => {}
    }
}
