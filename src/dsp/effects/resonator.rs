//! The resonator group (design-music 5 "resonator", design-implementation
//! 12.5, 12.8.8): `modal`, `horn`.
//!
//! `modal` is a bank of up to eight resonant bandpass biquads per channel
//! (`FxState::bq` holds exactly 16, so both channels fit without
//! aliasing), their `Q` derived from the requested decay time. `horn` is a
//! single-line waveguide-style feedback delay per channel, tuned to the
//! fundamental and shaped by a damping filter. Both are stable, bounded
//! primitives (a fixed biquad and a delay loop with gain below 1), so
//! output stays finite for any input.

use std::f32::consts::LN_10;

use super::prim::{clampf, DelayLine, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// The most resonators `modal` runs per channel (`bq` holds 16, split
/// evenly across the two channels).
const MAX_PARTIALS: usize = 8;
/// The lowest `horn`/waveguide fundamental, bounding its delay-line size.
const MIN_HORN_HZ: f32 = 20.0;

const MODAL_PARAMS: &[ParamDef] = &[
    ParamDef::unit("freq", 220.0, 20.0, 8000.0, "Hz"),
    ParamDef::new("count", 4.0, 1.0, MAX_PARTIALS as f32),
    ParamDef::new("spread", 0.05, 0.0, 1.0),
    ParamDef::unit("decay", 1.5, 0.05, 8.0, "s"),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

const HORN_PARAMS: &[ParamDef] = &[
    ParamDef::unit("freq", 110.0, MIN_HORN_HZ, 2000.0, "Hz"),
    ParamDef::unit("decay", 1.0, 0.05, 5.0, "s"),
    ParamDef::new("flare", 0.4, 0.0, 1.0),
    ParamDef::new("mix", 0.3, 0.0, 1.0),
];

/// The named parameters of a resonator-group kind.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Modal => MODAL_PARAMS,
        K::Horn => HORN_PARAMS,
        _ => &[],
    }
}

/// The preferred delay memory of a kind, in floats. `modal` uses only
/// biquads and needs none.
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    use EffectKind as K;
    let sr = sr.max(1.0);
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    match kind {
        K::Horn => 2 * ((sr / MIN_HORN_HZ) as usize + 4),
        _ => 0,
    }
}

/// Sets up a zeroed unit for a resonator-group kind.
#[allow(clippy::single_match)]
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, sr: f32) {
    use EffectKind as K;
    let sr = sr.max(1.0);
    match kind {
        K::Horn => {
            let mut cursor = 0usize;
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let want = (sr / MIN_HORN_HZ) as usize + 4;
            st.dl[0] = DelayLine::carve(&mut cursor, mem_len, want);
            // `want` here is clamped internally to whatever is left.
            st.dl[1] = DelayLine::carve(&mut cursor, mem_len, want);
        }
        _ => {}
    }
}

/// A bank of resonant bandpass biquads: partial `k` sits at
/// `freq * (k + 1) * (1 + spread * k)` (a simple inharmonic stretch), with
/// `Q` set from the -60 dB point of `decay` seconds
/// (`Q = freq / bandwidth`, `bandwidth = ln(1000) / (pi * decay)`), capped
/// so no single resonator can dominate the sum. Left uses `bq[0..8]`,
/// right `bq[8..16]`, so the channels never share filter state.
fn run_modal(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let base = p.first().copied().unwrap_or(220.0).max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = (p.get(1).copied().unwrap_or(4.0).round() as usize).clamp(1, MAX_PARTIALS);
    let spread = clampf(p.get(2).copied().unwrap_or(0.05), 0.0, 1.0);
    let decay = p.get(3).copied().unwrap_or(1.5).max(0.02);
    let bandwidth = (3.0 * LN_10) / (std::f32::consts::PI * decay);
    for k in 0..count {
        #[allow(clippy::cast_precision_loss)]
        let kf = k as f32;
        let f = clampf(base * (kf + 1.0) * (1.0 + spread * kf), 20.0, 0.49 * sr);
        let q = (f / bandwidth.max(1.0)).clamp(0.5, 40.0);
        st.bq[k].set(Shape::Bandpass, f, q, 0.0, sr);
        st.bq[MAX_PARTIALS + k].set(Shape::Bandpass, f, q, 0.0, sr);
    }
    #[allow(clippy::cast_precision_loss)]
    let gain = 1.0 / (count as f32).sqrt();
    let n = l.len().min(r.len());
    for i in 0..n {
        let mut outl = 0.0f32;
        let mut outr = 0.0f32;
        for k in 0..count {
            outl += st.bq[k].run(l[i]);
            outr += st.bq[MAX_PARTIALS + k].run(r[i]);
        }
        l[i] = outl * gain;
        r[i] = outr * gain;
    }
}

/// A single-line feedback loop per channel tuned to `freq`, approximating
/// a horn/tube resonance: the round-trip gain is set from `decay` (the
/// -60 dB point) and clamped well below the crate's stability bar, and
/// `flare` shapes a damping lowpass in the loop (brighter for a higher
/// value).
fn run_horn(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = p.first().copied().unwrap_or(110.0).max(MIN_HORN_HZ);
    let decay = p.get(1).copied().unwrap_or(1.0).max(0.05);
    let flare = clampf(p.get(2).copied().unwrap_or(0.4), 0.0, 1.0);
    let d = (sr / freq).max(2.0);
    let g = (-3.0 * LN_10 * (d / sr) / decay).exp().min(0.9);
    let cutoff = 400.0 + flare * 9000.0;
    let a = OnePole::coef(cutoff, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let yl = st.dl[0].read(mem, d);
        let yr = st.dl[1].read(mem, d);
        let fbl = st.op[0].lp(yl, a) * g;
        let fbr = st.op[1].lp(yr, a) * g;
        st.dl[0].write(mem, xl * 0.5 + fbl);
        st.dl[1].write(mem, xr * 0.5 + fbr);
        l[i] = yl;
        r[i] = yr;
    }
}

/// Processes one stereo block for a resonator-group kind, fully wet.
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
    use EffectKind as K;
    let sr = ctx.sr.max(1.0);
    match kind {
        K::Modal => run_modal(p, st, l, r, sr),
        K::Horn => run_horn(p, st, mem, l, r, sr),
        _ => {}
    }
}
