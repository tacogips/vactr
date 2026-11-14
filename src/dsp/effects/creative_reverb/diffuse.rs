//! Independent stereo eight-line FDNs, input diffusion, and feedback pitch shifting.
use super::common::{damping, rt60, value, Bank};
use super::{EffectKind, FxState, ParamDef};
use std::f32::consts::TAU;

const SPACE: &[ParamDef] = &[
    ParamDef::new("size", 0.65, 0.0, 1.0),
    ParamDef::unit("decay", 8.0, 0.2, 40.0, "s"),
    ParamDef::new("damping", 0.35, 0.0, 0.98),
    ParamDef::new("modulation", 0.3, 0.0, 1.0),
    ParamDef::new("gravity", 0.5, -1.0, 1.0),
    ParamDef::new("freeze", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
const SHIMMER: &[ParamDef] = &[
    ParamDef::new("size", 0.55, 0.0, 1.0),
    ParamDef::unit("decay", 5.0, 0.2, 30.0, "s"),
    ParamDef::new("damping", 0.3, 0.0, 0.98),
    ParamDef::unit("shift", 12.0, -24.0, 24.0, "semitones"),
    ParamDef::new("feedback", 0.5, 0.0, 0.9),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    match kind {
        EffectKind::SpaceReverb => SPACE,
        EffectKind::ShimmerReverb => SHIMMER,
        _ => &[],
    }
}
const BASE: [f32; 8] = [
    0.0713, 0.0797, 0.0899, 0.1013, 0.1137, 0.1271, 0.1399, 0.1493,
];
fn layout(sr: f32) -> ([Bank; 22], usize) {
    let mut rings = [Bank::new(0.01, sr); 22];
    let mut cursor = super::common::HEADER;
    for (id, ring) in rings.iter_mut().enumerate() {
        let seconds = if id < 16 {
            BASE[id % 8] * 2.0 + 0.002
        } else if id < 20 {
            0.021
        } else {
            0.052
        };
        *ring = Bank::region(seconds, sr, cursor, id);
        cursor += ring.stride;
    }
    (rings, cursor)
}
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    let (rings, total) = layout(sr);
    if kind == EffectKind::ShimmerReverb {
        total
    } else {
        total - rings[20].stride - rings[21].stride
    }
}
/// Two complementary triangular grains. Their weights sum to one, so the
/// resampler cannot amplify a bounded signal. Delay slope 1-ratio produces
/// playback at ratio=2^(semitones/12). Used INSIDE the FDN recirculation.
pub(crate) fn shifted(
    bank: Bank,
    mem: &mut [f32],
    line: usize,
    input: f32,
    phase: f32,
    window: f32,
) -> f32 {
    bank.write(mem, line, input);
    let other = (phase + 0.5).fract();
    let triangle = |x: f32| 1.0 - (2.0 * x - 1.0).abs();
    bank.read(mem, line, 2.0 + phase * window) * triangle(phase)
        + bank.read(mem, line, 2.0 + other * window) * triangle(other)
}
#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let (rings, _) = layout(sr);
    let shimmer = kind == EffectKind::ShimmerReverb;
    let scale = 0.5 + 1.5 * value(p, 0, 0.6);
    let decay = value(p, 1, if shimmer { 5.0 } else { 8.0 });
    let loss = value(p, 2, 0.35);
    let modulation = if shimmer { 0.15 } else { value(p, 3, 0.3) };
    let gravity = if shimmer { 0.5 } else { value(p, 4, 0.5) };
    let freeze = !shimmer && value(p, 5, 0.0) >= 0.5;
    let ratio = 2f32.powf(value(p, 3, 12.0) / 12.0);
    let shift_feedback = if shimmer { value(p, 4, 0.5) } else { 0.0 };
    let window = 0.05 * sr;
    let base = BASE;
    // Fixed delay in freeze preserves energy; fractional modulation is stopped.
    for (left, right) in l.iter_mut().zip(r) {
        let inputs = [*left, *right];
        let mut out = [0.0; 2];
        for channel in 0..2 {
            let mut input = if freeze { 0.0 } else { inputs[channel] * 0.35 };
            input = rings[16 + channel * 2].allpass(mem, 0, input, 0.0113 * sr, 0.68);
            input = rings[17 + channel * 2].allpass(mem, 0, input, 0.0197 * sr, 0.62);
            let mut y = [0.0; 8];
            let mut gains = [0.0; 8];
            for k in 0..8 {
                let id = channel * 8 + k;
                let delay = base[k] * scale + 0.00043 * channel as f32;
                let wobble = if freeze {
                    0.0
                } else {
                    modulation * 0.0007 * sr * (TAU * st.s[0] + k as f32 * 1.7).sin()
                };
                y[k] = rings[id].read(mem, 0, (delay * sr).round() + wobble);
                gains[k] = if freeze {
                    1.0
                } else {
                    rt60(
                        delay,
                        decay
                            * if !shimmer && gravity >= 0.0 {
                                0.25 + 0.75 * gravity
                            } else {
                                1.0
                            },
                    )
                };
            }
            let sum = y.iter().sum::<f32>() * 0.25;
            let wet = (y[0] - y[1] + y[2] - y[3] + y[4] - y[5] + y[6] - y[7]) * 0.353_553_38;
            let pitch = if shimmer {
                shifted(rings[20 + channel], mem, 0, wet, st.s[1], window)
            } else {
                0.0
            };
            for k in 0..8 {
                let id = channel * 8 + k;
                let rotated = y[k] - sum; // Orthogonal Householder matrix.
                let filtered = if freeze {
                    rotated
                } else {
                    damping(mem, 32 + id, rotated, loss)
                };
                // Blend, rather than add, pitch feedback to retain bounded loop gain.
                let recirculated =
                    filtered * (1.0 - shift_feedback) + pitch * shift_feedback * 0.353_553_38;
                rings[id].write(mem, 0, input * 0.353_553_38 + gains[k] * recirculated);
            }
            // Negative gravity closes a wet gate at excitation then opens it slowly:
            // a causal swelling tail, without claiming future-signal reversal.
            let gate = if gravity < 0.0 && !freeze {
                let slot = 24 + channel;
                let trigger = inputs[channel].abs();
                if trigger > 0.001 && trigger > st.s[4 + channel] * 1.5 {
                    mem[slot] = 0.0;
                }
                st.s[4 + channel] = trigger.max(st.s[4 + channel] * (-1.0 / (0.03 * sr)).exp());
                mem[slot] +=
                    (1.0 - mem[slot]) * (1.0 - (-1.0 / ((0.1 + gravity.abs() * 1.5) * sr)).exp());
                mem[slot]
            } else {
                1.0
            };
            out[channel] = wet * gate;
        }
        if !freeze {
            st.s[0] = (st.s[0] + 0.17 / sr).fract();
        }
        st.s[1] = (st.s[1] + (1.0 - ratio) / window).rem_euclid(1.0);
        *left = out[0];
        *right = out[1];
    }
}
