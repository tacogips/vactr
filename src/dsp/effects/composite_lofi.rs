//! Original composite production lo-fi: bandwidth, normalized saturation,
//! stereo tape timing, fractional sample-and-hold, quantization and surface noise.
//! Research references are in design-lofi.md; no external snippets are copied.
use super::prim::{DelayLine, OnePole};
use super::{FxCtx, FxState, ParamDef};

pub(super) const PARAMS: &[ParamDef] = &[
    ParamDef::unit("tone", 6500.0, 200.0, 20000.0, "Hz"),
    ParamDef::new("drive", 1.3, 0.1, 12.0),
    ParamDef::new("wow", 0.08, 0.0, 1.0),
    ParamDef::new("flutter", 0.03, 0.0, 1.0),
    ParamDef::new("bits", 16.0, 2.0, 24.0),
    ParamDef::unit("rate", 32000.0, 500.0, 96000.0, "Hz"),
    ParamDef::new("hiss", 0.0, 0.0, 1.0),
    ParamDef::new("crackle", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
/// Six milliseconds per channel covers all valid wow/flutter settings.
pub(super) fn mem_len(sr: f32) -> usize {
    2 * ((sr.max(1.0) * 0.006).ceil() as usize + 4)
}
pub(super) fn init(st: &mut FxState, len: usize) {
    let mut cursor = 0;
    st.dl[0] = DelayLine::carve(&mut cursor, len, len / 2);
    st.dl[1] = DelayLine::carve(&mut cursor, len, len - len / 2);
}
fn value(p: &[f32], i: usize) -> f32 {
    PARAMS[i].clamp(p.get(i).copied().unwrap_or(PARAMS[i].default))
}
fn finite(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

/// Processes fully wet; FxUnit supplies dry-transparent mix in bus and voice.
pub(super) fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let sr = ctx.sr.max(100.0);
    let tone = value(p, 0).min(0.45 * sr);
    let drive = value(p, 1);
    let wow = value(p, 2);
    let flutter = value(p, 3);
    let quantum = 2.0f32.powf(value(p, 4).round() - 1.0);
    let rate = value(p, 5).min(sr);
    let hiss = value(p, 6);
    let crackle = value(p, 7);
    let a = OnePole::coef(tone, sr);
    let click_decay = (-1.0 / (0.002 * sr)).exp();
    let click_probability = (40.0 * crackle / sr).min(1.0);
    let wow_depth = 0.002 * wow;
    let flutter_depth = 0.00015 * flutter;
    let base = wow_depth + flutter_depth;
    for (left, right) in l.iter_mut().zip(r) {
        // A shared transport avoids stereo wandering; audio histories stay separate.
        let delay =
            (base + wow_depth * st.lfo[0].next(0.55, sr) + flutter_depth * st.lfo[1].next(8.1, sr))
                * sr;
        let input = [finite(*left), finite(*right)];
        let mut tape = [0.0; 2];
        for ch in 0..2 {
            let first = st.op[2 * ch].lp(input[ch], a);
            let filtered = st.op[2 * ch + 1].lp(first, a);
            let saturated = (drive * filtered).tanh() / drive;
            st.dl[ch].write(mem, saturated);
            tape[ch] = if base == 0.0 {
                saturated
            } else {
                // Fractional delay below one sample interpolates to the current
                // input rather than imposing a minimum one-sample latency.
                if delay < 1.0 {
                    saturated + (st.dl[ch].read(mem, 2.0) - saturated) * delay
                } else {
                    st.dl[ch].read(mem, delay + 1.0)
                }
            };
        }
        st.s[0] += rate / sr;
        if st.s[5] == 0.0 || st.s[0] >= 1.0 {
            st.s[1] = tape[0];
            st.s[2] = tape[1];
            st.s[0] = st.s[0].fract();
            st.s[5] = 1.0;
        }
        let mut wet = [0.0; 2];
        for (ch, y) in wet.iter_mut().enumerate() {
            // Consume the same deterministic RNG stream even when noise is off.
            let noise = st.rng.bipolar();
            let chance = st.rng.unit();
            let impulse = st.rng.bipolar();
            st.s[3 + ch] *= click_decay;
            if crackle == 0.0 {
                st.s[3 + ch] = 0.0;
            } else if chance < click_probability {
                st.s[3 + ch] += impulse * 0.12 * crackle * crackle;
            }
            *y = (st.s[1 + ch] * quantum).round() / quantum
                + noise * 0.015 * hiss * hiss
                + st.s[3 + ch];
        }
        *left = wet[0];
        *right = wet[1];
    }
}
