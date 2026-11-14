//! Plaits position 22 analog and synthetic snare-drum signal stages.
//!
//! MIT source-stage translation with host-rate timing and per-voice RNG.
//! Copyright and numerical differences are recorded in THIRD_PARTY_NOTICES.md.

mod analog;
mod synthetic;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Fixed preallocated state per output node.
pub const STATE_FLOATS: usize = 24;

#[inline]
fn unit(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

#[inline]
fn rate_pole(source_coefficient: f32, sr: f32) -> f32 {
    1.0 - (1.0 - source_coefficient).powf(48_000.0 / sr)
}

#[inline]
fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Source stmlib SVF state update with exact tangent in place of its
/// fast polynomial coefficient; returns bandpass and lowpass outputs.
#[inline]
fn svf(input: f32, f: f32, q: f32, bp: &mut f32, lp: &mut f32) -> (f32, f32) {
    let g = (std::f32::consts::PI * f.min(0.497)).tan();
    let r = 1.0 / q.max(0.01);
    let h = 1.0 / (1.0 + r * g + g * g);
    let hp = (input - (r + g) * *bp - *lp) * h;
    let next_bp = g * hp + *bp;
    *bp = g * hp + next_bp;
    let next_lp = g * next_bp + *lp;
    *lp = g * next_bp + next_lp;
    (next_bp, next_lp)
}

/// Analog main (mode 0) or synthetic auxiliary (mode 1). Both states are
/// reset by `Voice::start` and allocated before entering the callback.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    let block_len = out.len();
    for (i, sample) in out.iter_mut().enumerate() {
        let raw_freq = ins[0].at(i);
        let freq = if raw_freq.is_finite() {
            raw_freq.clamp(20.0, sr * 0.4)
        } else {
            180.0
        };
        let harmonics = unit(ins[1].at(i), 0.5);
        let timbre = unit(ins[2].at(i), 0.5);
        let morph = unit(ins[3].at(i), 0.5);
        let accent = unit(ins[4].at(i), 1.0);
        let sustain = unit(ins[6].at(i), 0.0) >= 0.5;
        #[allow(clippy::cast_precision_loss)]
        let remaining = (block_len - i) as f32;
        let gate = i < kx.gate;
        let value = if ins[5].at(i) < 0.5 {
            analog::tick(
                mem, st, freq, harmonics, timbre, morph, accent, sustain, gate, sr, remaining,
                kx.seed,
            )
        } else {
            synthetic::tick(
                mem, st, freq, harmonics, timbre, morph, accent, sustain, gate, sr, remaining,
                kx.seed,
            )
        };
        *sample = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        st.s[1] += 1.0 / sr;
        if (!sustain && st.s[1] > 4.0) || (sustain && !gate && st.s[1] > 0.1) {
            st.finish();
        }
    }
}
