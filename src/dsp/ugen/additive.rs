//! The `additive` kernel: up to `partials_max` harmonic sine partials.
//!
//! Partial `k` (1-based) has amplitude `1 / k^tilt`, normalized so the
//! partials sum to 1; `count` selects how many sound. A `partials` list
//! argument is a realization-time constant (12.8.7) that the lowering maps
//! onto `count` and `tilt`; per-partial amplitude lists are not carried by
//! `UGenSpec::Additive`. Partials at or above Nyquist are skipped.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// `additive freq count tilt` (phases live in `mem`).
pub fn render(
    partials_max: u8,
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let max = usize::from(partials_max.max(1)).min(mem.len());
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = (ins[1].first().round().max(1.0) as usize).min(max);
    let tilt = ins[2].first().clamp(0.0, 4.0);
    let mut norm = 0.0;
    #[allow(clippy::cast_precision_loss)]
    for k in 1..=count {
        norm += 1.0 / (k as f32).powf(tilt);
    }
    let norm = if norm > 0.0 { 1.0 / norm } else { 0.0 };
    let phases = &mut mem[..max];
    for (i, y) in out.iter_mut().enumerate() {
        let f = ins[0].at(i);
        let f = if f.is_finite() { f.max(0.0) } else { 0.0 };
        let mut acc = 0.0;
        for (k0, ph) in phases.iter_mut().enumerate().take(count) {
            #[allow(clippy::cast_precision_loss)]
            let k = (k0 + 1) as f32;
            let dt = f * k / kx.sr;
            if dt >= 0.5 {
                break;
            }
            acc += (TAU * *ph).sin() / k.powf(tilt);
            *ph += dt;
            *ph -= ph.floor();
        }
        *y = acc * norm;
    }
}
