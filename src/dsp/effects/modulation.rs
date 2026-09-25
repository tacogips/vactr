//! Modulation effects (design 12.5, 12.8.8; catalog design-music.md
//! section 5): `chorus`, `flanger`, `phaser`, `tremolo`, `auto-pan`,
//! `auto-filter`, `pitch-shift`, `pitch-shift-hq`, `freq-shift`, `rotary`,
//! `wow-flutter`, `doppler`, `vibrato`.
//!
//! Delay-based kinds (`chorus`, `flanger`, `vibrato`, `wow-flutter`,
//! `doppler`, `rotary`, the two pitch shifters) modulate a fractional
//! delay tap; the LFO advances every sample (cheap: `Lfo::next` is one
//! `sin`) so the tap moves smoothly. Kinds that need biquad coefficients
//! (`phaser`, `auto-filter`) recompute them once per block from a single
//! LFO sample advanced by a whole block's worth of phase in one call
//! (`sr / frames` as the "advance rate" argument), per 12.8.8's
//! "coefficients once per block" rule; the audio itself still runs every
//! sample. `freq-shift`'s single-sideband path uses a fixed (unmodulated)
//! allpass Hilbert approximation set once at `init`, never recomputed.
//!
//! **Approximations (documented per 12.8.8 B4):**
//! - `pitch-shift`: two crossfaded delay taps ramping through a window,
//!   not a phase vocoder.
//! - `pitch-shift-hq`: four overlapping delay taps with raised-cosine
//!   windows (a coarse grain engine), not a phase vocoder.
//! - `freq-shift`'s single-sideband mode: a 2-stage allpass Hilbert
//!   approximation per channel, accurate only over a limited mid-band
//!   (no matched-delay direct branch); `ring` selects plain ring
//!   modulation instead (both sidebands, no Hilbert network).

use std::f32::consts::TAU;

use super::prim::{DelayLine, Lfo, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

const CHORUS_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 0.5, 0.05, 10.0, "Hz"),
    ParamDef::new("depth", 0.5, 0.0, 1.0),
    ParamDef::unit("delay", 15.0, 1.0, 30.0, "ms"),
    ParamDef::new("feedback", 0.0, -0.95, 0.95),
    ParamDef::new("mix", 0.5, 0.0, 1.0),
];
const FLANGER_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 0.2, 0.02, 10.0, "Hz"),
    ParamDef::new("depth", 0.7, 0.0, 1.0),
    ParamDef::new("feedback", 0.5, -0.95, 0.95),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const PHASER_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 0.3, 0.02, 5.0, "Hz"),
    ParamDef::new("depth", 0.7, 0.0, 1.0),
    ParamDef::new("stages", 4.0, 2.0, 8.0),
    ParamDef::new("feedback", 0.3, -0.95, 0.95),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const TREMOLO_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 5.0, 0.1, 20.0, "Hz"),
    ParamDef::new("depth", 0.5, 0.0, 1.0),
];
const AUTO_PAN_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 0.5, 0.02, 10.0, "Hz"),
    ParamDef::new("depth", 0.7, 0.0, 1.0),
];
const AUTO_FILTER_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 0.3, 0.02, 10.0, "Hz"),
    ParamDef::new("depth", 0.6, 0.0, 1.0),
    ParamDef::unit("freq", 800.0, 60.0, 12_000.0, "Hz"),
    ParamDef::new("res", 0.7, 0.1, 10.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const PITCH_SHIFT_PARAMS: &[ParamDef] = &[
    ParamDef::new("semitones", 0.0, -24.0, 24.0),
    ParamDef::unit("window", 50.0, 10.0, 100.0, "ms"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const FREQ_SHIFT_PARAMS: &[ParamDef] = &[
    ParamDef::unit("shift", 0.0, -2000.0, 2000.0, "Hz"),
    ParamDef::new("ring", 0.0, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const ROTARY_PARAMS: &[ParamDef] = &[
    ParamDef::unit("speed", 0.8, 0.02, 10.0, "Hz"),
    ParamDef::new("depth", 0.6, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const WOW_FLUTTER_PARAMS: &[ParamDef] = &[
    ParamDef::new("wow", 0.5, 0.0, 1.0),
    ParamDef::new("flutter", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const DOPPLER_PARAMS: &[ParamDef] = &[
    ParamDef::unit("speed", 0.5, 0.02, 5.0, "Hz"),
    ParamDef::new("distance", 0.5, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];
const VIBRATO_PARAMS: &[ParamDef] = &[
    ParamDef::unit("rate", 5.0, 0.1, 12.0, "Hz"),
    ParamDef::new("depth", 0.3, 0.0, 1.0),
];

/// The named parameters of a modulation kind, in catalog order.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind::{
        AutoFilter, AutoPan, Chorus, Doppler, Flanger, FreqShift, Phaser, PitchShift, PitchShiftHq,
        Rotary, Tremolo, Vibrato, WowFlutter,
    };
    match kind {
        Chorus => CHORUS_PARAMS,
        Flanger => FLANGER_PARAMS,
        Phaser => PHASER_PARAMS,
        Tremolo => TREMOLO_PARAMS,
        AutoPan => AUTO_PAN_PARAMS,
        AutoFilter => AUTO_FILTER_PARAMS,
        PitchShift | PitchShiftHq => PITCH_SHIFT_PARAMS,
        FreqShift => FREQ_SHIFT_PARAMS,
        Rotary => ROTARY_PARAMS,
        WowFlutter => WOW_FLUTTER_PARAMS,
        Doppler => DOPPLER_PARAMS,
        Vibrato => VIBRATO_PARAMS,
        _ => &[],
    }
}

/// The preferred stereo delay-line span of a kind, in milliseconds per
/// channel (`0.0` for kinds with no delay memory).
fn delay_ms_for(kind: EffectKind) -> f32 {
    use EffectKind::{
        Chorus, Doppler, Flanger, PitchShift, PitchShiftHq, Rotary, Vibrato, WowFlutter,
    };
    match kind {
        Chorus => 40.0,
        Flanger => 10.0,
        Vibrato => 12.0,
        WowFlutter => 15.0,
        Doppler => 45.0,
        Rotary => 10.0,
        PitchShift | PitchShiftHq => 100.0,
        _ => 0.0,
    }
}

/// `ms` converted to samples at `sr` (never negative).
fn ms_to_samples(ms: f32, sr: f32) -> f32 {
    ms.max(0.0) * sr.max(1.0) / 1000.0
}

/// Carves two equal delay lines (left, right) out of `mem_len` floats,
/// each wanting `ms` milliseconds of capacity.
fn carve_stereo(mem_len: usize, ms: f32, sr: f32) -> (DelayLine, DelayLine) {
    let want = ms_to_samples(ms, sr).ceil().max(1.0) as usize;
    let mut cursor = 0usize;
    let a = DelayLine::carve(&mut cursor, mem_len, want);
    let b = DelayLine::carve(&mut cursor, mem_len, want);
    (a, b)
}

/// Parameter `i`, or `default` if `p` is shorter than expected.
fn pv(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i).copied().unwrap_or(default)
}

/// A block-rate LFO step: advances `lfo` by one block's worth of phase
/// (`frames` samples at `rate` Hz) in a single call, for kinds that
/// recompute biquad coefficients once per block instead of per sample.
fn block_lfo(lfo: &mut Lfo, rate: f32, sr: f32, frames: usize) -> f32 {
    lfo.next(rate, sr / (frames.max(1) as f32))
}

/// The preferred delay memory of a kind, in floats (the caller clamps it
/// to what the unit's region holds).
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    let ms = delay_ms_for(kind);
    if ms <= 0.0 {
        0
    } else {
        2 * (ms_to_samples(ms, sr).ceil() as usize)
    }
}

/// Sets up a zeroed modulation unit.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, sr: f32) {
    use EffectKind::{
        Chorus, Doppler, Flanger, FreqShift, PitchShift, PitchShiftHq, Rotary, Vibrato, WowFlutter,
    };
    let ms = delay_ms_for(kind);
    if ms > 0.0 {
        let (a, b) = carve_stereo(mem_len, ms, sr);
        st.dl[0] = a;
        st.dl[1] = b;
    }
    match kind {
        Chorus | Flanger | Vibrato | WowFlutter | Doppler | Rotary => {
            st.lfo[0] = Lfo::default();
            st.lfo[1] = Lfo { phase: 0.25 };
        }
        PitchShift => {
            st.s[0] = 0.0;
            st.s[1] = 0.5;
        }
        PitchShiftHq => {
            st.s[0] = 0.0;
            st.s[1] = 0.25;
            st.s[2] = 0.5;
            st.s[3] = 0.75;
        }
        FreqShift => {
            // A fixed (never re-swept) 2-stage-per-channel allpass network
            // approximating a quadrature (Hilbert) split over a limited
            // mid-band; see the module doc comment.
            st.bq[0].set(Shape::Allpass, 300.0, 0.6, 0.0, sr);
            st.bq[1].set(Shape::Allpass, 3000.0, 0.6, 0.0, sr);
            st.bq[2].set(Shape::Allpass, 300.0, 0.6, 0.0, sr);
            st.bq[3].set(Shape::Allpass, 3000.0, 0.6, 0.0, sr);
        }
        _ => {}
    }
}

/// Processes one stereo block, fully wet.
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
    use EffectKind::{
        AutoFilter, AutoPan, Chorus, Doppler, Flanger, FreqShift, Phaser, PitchShift, PitchShiftHq,
        Rotary, Tremolo, Vibrato, WowFlutter,
    };
    let sr = ctx.sr;
    match kind {
        Chorus => chorus(p, st, mem, l, r, sr),
        Flanger => flanger(p, st, mem, l, r, sr),
        Phaser => phaser(p, st, l, r, sr),
        Tremolo => tremolo(p, st, l, r, sr),
        AutoPan => auto_pan(p, st, l, r, sr),
        AutoFilter => auto_filter(p, st, l, r, sr),
        PitchShift => pitch_shift(p, st, mem, l, r, sr, false),
        PitchShiftHq => pitch_shift(p, st, mem, l, r, sr, true),
        FreqShift => freq_shift(p, st, l, r, sr),
        Rotary => rotary(p, st, mem, l, r, sr),
        WowFlutter => wow_flutter(p, st, mem, l, r, sr),
        Doppler => doppler(p, st, mem, l, r, sr),
        Vibrato => vibrato(p, st, mem, l, r, sr),
        _ => {}
    }
}

/// One step of a modulated stereo delay tap shared by chorus, flanger,
/// vibrato, wow-flutter and doppler: each channel reads its own delay
/// line at the given distance (in samples), then writes the input (plus
/// `feedback` of the tap it just read) at the head. Not a closure so it
/// can be called directly from each kind's per-sample loop.
#[inline]
#[allow(clippy::too_many_arguments)]
fn mod_delay_step(
    st: &mut FxState,
    mem: &mut [f32],
    xl: f32,
    xr: f32,
    dl_s: f32,
    dr_s: f32,
    feedback: f32,
) -> (f32, f32) {
    let wl = st.dl[0].read(mem, dl_s.max(1.0));
    st.dl[0].write(mem, xl + feedback * wl);
    let wr = st.dl[1].read(mem, dr_s.max(1.0));
    st.dl[1].write(mem, xr + feedback * wr);
    (wl, wr)
}

fn chorus(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 0.5).max(0.0);
    let depth = pv(p, 1, 0.5).clamp(0.0, 1.0);
    let base_ms = pv(p, 2, 15.0).clamp(1.0, 30.0);
    let feedback = pv(p, 3, 0.0).clamp(-0.95, 0.95);
    let mod_ms = depth * 10.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        let ml = st.lfo[0].next(rate, sr);
        let mr = st.lfo[1].next(rate, sr);
        let dl_s = ms_to_samples(base_ms + mod_ms * ml, sr);
        let dr_s = ms_to_samples(base_ms + mod_ms * mr, sr);
        let (wl, wr) = mod_delay_step(st, mem, l[i], r[i], dl_s, dr_s, feedback);
        l[i] = wl;
        r[i] = wr;
    }
}

fn flanger(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 0.2).max(0.0);
    let depth = pv(p, 1, 0.7).clamp(0.0, 1.0);
    let feedback = pv(p, 2, 0.5).clamp(-0.95, 0.95);
    let base_ms = 1.0;
    let mod_ms = depth * 6.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        let ml = st.lfo[0].next(rate, sr);
        let mr = st.lfo[1].next(rate, sr);
        let dl_s = ms_to_samples(base_ms + mod_ms * (0.5 + 0.5 * ml), sr);
        let dr_s = ms_to_samples(base_ms + mod_ms * (0.5 + 0.5 * mr), sr);
        let (wl, wr) = mod_delay_step(st, mem, l[i], r[i], dl_s, dr_s, feedback);
        l[i] = wl;
        r[i] = wr;
    }
}

fn vibrato(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 5.0).max(0.0);
    let depth = pv(p, 1, 0.3).clamp(0.0, 1.0);
    let base_ms = 5.0;
    let mod_ms = depth * 4.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        let ml = st.lfo[0].next(rate, sr);
        let mr = st.lfo[1].next(rate, sr);
        let dl_s = ms_to_samples(base_ms + mod_ms * ml, sr);
        let dr_s = ms_to_samples(base_ms + mod_ms * mr, sr);
        let (wl, wr) = mod_delay_step(st, mem, l[i], r[i], dl_s, dr_s, 0.0);
        l[i] = wl;
        r[i] = wr;
    }
}

fn wow_flutter(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
) {
    let wow = pv(p, 0, 0.5).clamp(0.0, 1.0);
    let flutter = pv(p, 1, 0.3).clamp(0.0, 1.0);
    let base_ms = 8.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        // Mono wobble: both channels track the same wow/flutter LFOs.
        let w = st.lfo[0].next(0.7, sr);
        let f = st.lfo[1].next(9.0, sr);
        let d_s = ms_to_samples(base_ms + wow * 3.0 * w + flutter * f, sr);
        let (wl, wr) = mod_delay_step(st, mem, l[i], r[i], d_s, d_s, 0.0);
        l[i] = wl;
        r[i] = wr;
    }
}

fn doppler(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let speed = pv(p, 0, 0.5).max(0.0);
    let distance = pv(p, 1, 0.5).clamp(0.0, 1.0);
    let base_ms = 5.0 + distance * 20.0;
    let mod_ms = distance * 15.0 + 1.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        let m = st.lfo[0].next(speed, sr);
        let d_s = ms_to_samples(base_ms + mod_ms * m, sr);
        let (wl, wr) = mod_delay_step(st, mem, l[i], r[i], d_s, d_s, 0.0);
        l[i] = wl;
        r[i] = wr;
    }
}

fn rotary(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let speed = pv(p, 0, 0.8).max(0.0);
    let depth = pv(p, 1, 0.6).clamp(0.0, 1.0);
    let base_ms = 3.0;
    let mod_ms = depth * 4.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        let ml = st.lfo[0].next(speed, sr);
        let mr = st.lfo[1].next(speed, sr);
        let dl_s = ms_to_samples(base_ms + mod_ms * (0.5 + 0.5 * ml), sr).max(1.0);
        let dr_s = ms_to_samples(base_ms + mod_ms * (0.5 + 0.5 * mr), sr).max(1.0);
        let xl = l[i];
        let xr = r[i];
        let wl = st.dl[0].read(mem, dl_s);
        st.dl[0].write(mem, xl);
        let wr = st.dl[1].read(mem, dr_s);
        st.dl[1].write(mem, xr);
        l[i] = wl * (0.85 + 0.15 * ml);
        r[i] = wr * (0.85 + 0.15 * mr);
    }
}

fn tremolo(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 5.0).max(0.0);
    let depth = pv(p, 1, 0.5).clamp(0.0, 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let m = st.lfo[0].next(rate, sr);
        let g = 1.0 - depth * (0.5 - 0.5 * m);
        l[i] *= g;
        r[i] *= g;
    }
}

fn auto_pan(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 0.5).max(0.0);
    let depth = pv(p, 1, 0.7).clamp(0.0, 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let m = st.lfo[0].next(rate, sr);
        let pan = (0.5 + 0.5 * depth * m).clamp(0.0, 1.0);
        let (gl, gr) = super::prim::pan_gains(pan);
        l[i] *= gl;
        r[i] *= gr;
    }
}

fn auto_filter(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 0.3).max(0.0);
    let depth = pv(p, 1, 0.6).clamp(0.0, 1.0);
    let freq = pv(p, 2, 800.0).clamp(60.0, 0.45 * sr.max(200.0));
    let res = pv(p, 3, 0.7).clamp(0.05, 10.0);
    let n = l.len().min(r.len());
    let m = block_lfo(&mut st.lfo[0], rate, sr, n);
    let f = (freq * (1.0 + depth * 2.0 * (0.5 + 0.5 * m))).clamp(60.0, 0.49 * sr.max(200.0));
    st.bq[0].set(Shape::Lowpass, f, res, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, f, res, 0.0, sr);
    for i in 0..n {
        l[i] = st.bq[0].run(l[i]);
        r[i] = st.bq[1].run(r[i]);
    }
}

fn phaser(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 0.3).max(0.0);
    let depth = pv(p, 1, 0.7).clamp(0.0, 1.0);
    let stages = (pv(p, 2, 4.0).round().clamp(2.0, 8.0) as usize).min(8);
    let feedback = pv(p, 3, 0.3).clamp(-0.95, 0.95);
    let n = l.len().min(r.len());
    let m = block_lfo(&mut st.lfo[0], rate, sr, n);
    let center = 300.0 + depth * 1500.0 * (0.5 + 0.5 * m);
    for s in 0..stages {
        let f = center * (1.0 + 0.6 * s as f32);
        st.bq[s].set(Shape::Allpass, f, 0.5, 0.0, sr);
        st.bq[8 + s].set(Shape::Allpass, f, 0.5, 0.0, sr);
    }
    let mut fbl = st.s[0];
    let mut fbr = st.s[1];
    for i in 0..n {
        let mut yl = l[i] + feedback * fbl;
        for s in 0..stages {
            yl = st.bq[s].run(yl);
        }
        let mut yr = r[i] + feedback * fbr;
        for s in 0..stages {
            yr = st.bq[8 + s].run(yr);
        }
        fbl = yl;
        fbr = yr;
        l[i] = yl;
        r[i] = yr;
    }
    st.s[0] = fbl;
    st.s[1] = fbr;
}

/// A raised-triangle window (two-tap crossfade), argument in `[0, 1)`.
fn win_tri(frac: f32) -> f32 {
    (1.0 - (2.0 * frac - 1.0).abs()).max(0.0)
}

/// A raised-cosine window (four-tap crossfade), argument in `[0, 1)`.
fn win_cos(frac: f32) -> f32 {
    (0.5 - 0.5 * (TAU * frac).cos()).max(0.0)
}

/// `pitch-shift` (`hq = false`, two crossfaded taps) and `pitch-shift-hq`
/// (`hq = true`, four crossfaded taps; see the module doc comment).
#[allow(clippy::too_many_arguments)]
fn pitch_shift(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    sr: f32,
    hq: bool,
) {
    let semitones = pv(p, 0, 0.0).clamp(-24.0, 24.0);
    let window_ms = pv(p, 1, 50.0).clamp(10.0, 100.0);
    let ratio = 2f32.powf(semitones / 12.0);
    let window = ms_to_samples(window_ms, sr).max(8.0);
    let step_frac = (1.0 - ratio) / window;
    let taps = if hq { 4 } else { 2 };
    let norm = if hq { 0.5 } else { 1.0 };
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for k in 0..taps {
            let mut frac = st.s[k] + step_frac;
            frac -= frac.floor();
            st.s[k] = frac;
            let d = (frac * window).max(1.0);
            let w = if hq { win_cos(frac) } else { win_tri(frac) };
            out_l += st.dl[0].read(mem, d) * w;
            out_r += st.dl[1].read(mem, d) * w;
        }
        st.dl[0].write(mem, xl);
        st.dl[1].write(mem, xr);
        l[i] = out_l * norm;
        r[i] = out_r * norm;
    }
}

fn freq_shift(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let shift = pv(p, 0, 0.0).clamp(-2000.0, 2000.0);
    let ring = pv(p, 1, 0.0) >= 0.5;
    let inc = shift / sr.max(1.0);
    let mut phase = st.s[8];
    let n = l.len().min(r.len());
    for i in 0..n {
        let xl = l[i];
        let xr = r[i];
        phase += inc;
        phase -= phase.floor();
        let (sn, cs) = (TAU * phase).sin_cos();
        if ring {
            l[i] = xl * cs;
            r[i] = xr * cs;
        } else {
            let stage_l = st.bq[0].run(xl);
            let ql = st.bq[1].run(stage_l);
            let stage_r = st.bq[2].run(xr);
            let qr = st.bq[3].run(stage_r);
            l[i] = xl * cs - ql * sn;
            r[i] = xr * cs - qr * sn;
        }
    }
    st.s[8] = phase;
}
