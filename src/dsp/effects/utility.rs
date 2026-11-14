//! Utility effects (design-music.md section 5, design 12.5/12.8.8):
//! `gain`, `mute`, `polarity`, `dc-offset`, `dry-wet`, `section`,
//! `channel-divider`, `fir-crossover`.
//!
//! These are small, allocation-free bus/chain utilities: plain gain and
//! routing except `dc-offset` (a DC-blocking highpass) and
//! `fir-crossover` (a documented biquad approximation of a linear-phase
//! band split, design 12.8.8).

use super::prim::{db_to_gain, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// Reads parameter `i`, defaulting when `p` is shorter than
/// `params(kind)` (defensive: production callers always match lengths).
#[inline]
fn pv(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i).copied().unwrap_or(default)
}

const GAIN_PARAMS: [ParamDef; 1] = [ParamDef::unit("gain", 0.0, -60.0, 24.0, "dB")];
const MUTE_PARAMS: [ParamDef; 1] = [ParamDef::new("on", 1.0, 0.0, 1.0)];
const POLARITY_PARAMS: [ParamDef; 2] = [
    ParamDef::new("flip-l", 1.0, 0.0, 1.0),
    ParamDef::new("flip-r", 1.0, 0.0, 1.0),
];
/// `dc-offset` is a DC-blocking one-pole highpass, not a literal offset
/// adder: `freq` is the corner below which DC bias is removed.
const DC_OFFSET_PARAMS: [ParamDef; 1] = [ParamDef::unit("freq", 10.0, 1.0, 200.0, "Hz")];
const DRY_WET_PARAMS: [ParamDef; 2] = [
    ParamDef::new("mix", 1.0, 0.0, 1.0),
    ParamDef::unit("gain", 0.0, -24.0, 24.0, "dB"),
];
const SECTION_PARAMS: [ParamDef; 2] = [
    ParamDef::new("on", 1.0, 0.0, 1.0),
    ParamDef::new("count", 64.0, 0.0, 256.0),
];
const CHANNEL_DIVIDER_PARAMS: [ParamDef; 1] = [ParamDef::new("mode", 0.0, 0.0, 3.0)];
const FIR_CROSSOVER_PARAMS: [ParamDef; 2] = [
    ParamDef::unit("freq", 1000.0, 20.0, 20_000.0, "Hz"),
    ParamDef::new("band", 0.0, 0.0, 1.0),
];

/// The named parameters of a utility kind, in catalog order.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Gain => &GAIN_PARAMS,
        K::Mute => &MUTE_PARAMS,
        K::Polarity => &POLARITY_PARAMS,
        K::DcOffset => &DC_OFFSET_PARAMS,
        K::DryWet => &DRY_WET_PARAMS,
        K::Section => &SECTION_PARAMS,
        K::ChannelDivider => &CHANNEL_DIVIDER_PARAMS,
        K::FirCrossover => &FIR_CROSSOVER_PARAMS,
        _ => &[],
    }
}

/// None of the utility kinds need delay memory.
pub(super) fn mem_len(_kind: EffectKind, _sr: f32) -> usize {
    0
}

/// Utility kinds carry no state that needs setup beyond the zeroed
/// default: gains and one-pole coefficients are recomputed every block.
pub(super) fn init(_kind: EffectKind, _st: &mut FxState, _mem_len: usize, _sr: f32) {}

/// `dc-offset`: a one-pole DC-blocking highpass at `p[0]` Hz, run
/// independently on each channel via `st.op[0]`/`st.op[1]`.
fn dc_offset_process(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 10.0).max(0.01);
    let a = OnePole::coef(freq, sr);
    let n = l.len().min(r.len());
    for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
        *lx = st.op[0].hp(*lx, a);
        *rx = st.op[1].hp(*rx, a);
    }
}

/// `fir-crossover`: approximation — a 4th-order Linkwitz-Riley-style
/// cascade of two matched biquads selects the low or high band; no FIR
/// filter or lookahead is used (design 12.8.8).
fn fir_crossover_process(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 1000.0);
    let band = pv(p, 1, 0.0);
    let shape = if band >= 0.5 {
        Shape::Highpass
    } else {
        Shape::Lowpass
    };
    st.bq[0].set(shape, freq, 0.707, 0.0, sr);
    st.bq[1].set(shape, freq, 0.707, 0.0, sr);
    st.bq[2].set(shape, freq, 0.707, 0.0, sr);
    st.bq[3].set(shape, freq, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
        let x = st.bq[0].run(*lx);
        *lx = st.bq[2].run(x);
        let y = st.bq[1].run(*rx);
        *rx = st.bq[3].run(y);
    }
}

/// Processes one stereo block of a utility kind in place, fully wet.
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
    use EffectKind as K;
    let sr = ctx.sr;
    match kind {
        K::Gain => {
            let g = db_to_gain(pv(p, 0, 0.0));
            let n = l.len().min(r.len());
            for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                *lx *= g;
                *rx *= g;
            }
        }
        K::Mute => {
            if pv(p, 0, 1.0) >= 0.5 {
                let n = l.len().min(r.len());
                l[..n].fill(0.0);
                r[..n].fill(0.0);
            }
        }
        K::Polarity => {
            let flip_l = pv(p, 0, 1.0) >= 0.5;
            let flip_r = pv(p, 1, 1.0) >= 0.5;
            let n = l.len().min(r.len());
            if flip_l {
                for x in &mut l[..n] {
                    *x = -*x;
                }
            }
            if flip_r {
                for x in &mut r[..n] {
                    *x = -*x;
                }
            }
        }
        K::DcOffset => dc_offset_process(p, st, l, r, sr),
        K::DryWet => {
            let g = db_to_gain(pv(p, 1, 0.0));
            let n = l.len().min(r.len());
            for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                *lx *= g;
                *rx *= g;
            }
        }
        // The bus chain runner reads `on`/`count` to bypass following
        // units; the unit itself is an identity pass-through.
        K::Section => {}
        K::ChannelDivider => {
            let mode = pv(p, 0, 0.0).round();
            let n = l.len().min(r.len());
            if mode <= 0.5 {
                // stereo pass: no change
            } else if mode <= 1.5 {
                r[..n].copy_from_slice(&l[..n]);
            } else if mode <= 2.5 {
                l[..n].copy_from_slice(&r[..n]);
            } else {
                for (lx, rx) in l[..n].iter_mut().zip(r[..n].iter_mut()) {
                    std::mem::swap(lx, rx);
                }
            }
        }
        K::FirCrossover => fir_crossover_process(p, st, l, r, sr),
        _ => {}
    }
}
