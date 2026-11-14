//! Plaits position 21 analog and synthetic bass-drum signal stages.
//!
//! MIT source translation with analytic sine and host-rate timing. The
//! original copyright and permission notice are in THIRD_PARTY_NOTICES.md.

mod analog;
mod synthetic;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Fixed preallocated state per output node (48 floats for the voice pair).
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
fn hz(v: f32, sr: f32) -> f32 {
    if v.is_finite() {
        v.clamp(20.0, sr * 0.4)
    } else {
        80.0
    }
}

/// Equivalent one-pole coefficient when moving a source 48 kHz process to
/// a different host rate.
#[inline]
fn rate_pole(source_coefficient: f32, sr: f32) -> f32 {
    1.0 - (1.0 - source_coefficient).powf(48_000.0 / sr)
}

/// stmlib's bounded soft-clip transfer, translated under its MIT notice.
#[inline]
fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let square = x * x;
    x * (27.0 + square) / (27.0 + 9.0 * square)
}

/// Both nodes are allocated at install. Voice::start zeroes their state for
/// every event; the first sample then starts the trigger pulses/envelopes.
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
        let freq = hz(ins[0].at(i), sr);
        let harmonics = unit(ins[1].at(i), 0.5);
        let timbre = unit(ins[2].at(i), 0.5);
        let morph = unit(ins[3].at(i), 0.5);
        let accent = unit(ins[4].at(i), 1.0);
        let sustain = unit(ins[6].at(i), 0.0) >= 0.5;
        let gate_open = i < kx.gate;
        let raw = if ins[5].at(i) < 0.5 {
            #[allow(clippy::cast_precision_loss)]
            let remaining = (block_len - i) as f32;
            analog::tick(
                mem, st, freq, harmonics, timbre, morph, accent, sustain, gate_open, sr, remaining,
            )
        } else {
            synthetic::tick(
                mem, st, freq, harmonics, timbre, morph, accent, sustain, gate_open, sr, kx.seed,
            )
        };
        *sample = if raw.is_finite() {
            raw.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        st.s[1] += 1.0 / sr;
        if (!sustain && st.s[1] > 4.0) || (sustain && !gate_open && st.s[1] > 0.1) {
            st.finish();
        }
    }
}
