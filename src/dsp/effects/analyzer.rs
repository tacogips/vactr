//! Analyzers (design 12.5, design-music section 5): audio-transparent taps
//! that write preallocated `f32` analysis cells and never alter the signal.
//!
//! Every analyzer has an `id` parameter: the first analysis cell it owns.
//! The cell layout per kind is fixed (`cells`); ring-buffered kinds
//! (spectrogram, note-spectrogram, oscilloscope) keep their frames in their
//! cells with the next write index in the last cell. A layout that does not
//! fit the table is written as far as it fits and counted in
//! `FxStats::clamped` (the audio continues untouched).

use std::f32::consts::TAU;

use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::{AnalyzerKind, EffectKind};

use super::prim::{DelayLine, OnePole};
use super::{FxCtx, FxState, ParamDef};

/// Log-spaced bands of `spectrum` and `spectrogram`.
pub const BANDS: usize = 32;
/// Frames kept by `spectrogram`.
pub const SPECTROGRAM_FRAMES: usize = 16;
/// Frames kept by `note-spectrogram` (12 pitch classes each).
pub const CHROMA_FRAMES: usize = 8;
/// Samples kept by `oscilloscope`.
pub const SCOPE_LEN: usize = 256;

const ID: ParamDef = ParamDef::new("id", 0.0, 0.0, 65_535.0);
const P_ID: &[ParamDef] = &[ID];
const P_LEVEL: &[ParamDef] = &[ID, ParamDef::unit("window", 0.3, 0.01, 5.0, "s")];
const P_SCOPE: &[ParamDef] = &[ID, ParamDef::new("decimate", 1.0, 1.0, 64.0)];

/// The parameters of an analyzer.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    match kind {
        EffectKind::Analyzer(AnalyzerKind::Level) => P_LEVEL,
        EffectKind::Analyzer(AnalyzerKind::Oscilloscope) => P_SCOPE,
        EffectKind::Analyzer(_) => P_ID,
        _ => &[],
    }
}

/// The number of analysis cells an analyzer writes, from its `id` on.
#[must_use]
pub fn cells(kind: AnalyzerKind) -> usize {
    match kind {
        AnalyzerKind::Level => 2,
        AnalyzerKind::Spectrum => BANDS,
        AnalyzerKind::Spectrogram => SPECTROGRAM_FRAMES * BANDS + 1,
        AnalyzerKind::NoteSpectrogram => CHROMA_FRAMES * 12 + 1,
        AnalyzerKind::Oscilloscope => SCOPE_LEN + 1,
        AnalyzerKind::PitchMeter => 2,
        AnalyzerKind::StereoMeter => 3,
    }
}

fn spectral(kind: EffectKind) -> bool {
    matches!(
        kind,
        EffectKind::Analyzer(
            AnalyzerKind::Spectrum
                | AnalyzerKind::Spectrogram
                | AnalyzerKind::NoteSpectrogram
                | AnalyzerKind::PitchMeter
        )
    )
}

/// Spectral kinds keep the last `FFT_SIZE` mono samples.
pub(super) fn mem_len(kind: EffectKind, _sr: f32) -> usize {
    if spectral(kind) {
        FFT_SIZE
    } else {
        0
    }
}

pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, _sr: f32) {
    let mut cursor = 0;
    if spectral(kind) {
        st.dl[0] = DelayLine::carve(&mut cursor, mem_len, FFT_SIZE);
    }
}

/// Reads the block (never writes `l`/`r`) and updates the cells.
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
    let EffectKind::Analyzer(a) = kind else {
        return;
    };
    let n = l.len().min(r.len());
    let (l, r) = (&l[..n], &r[..n]);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let base = p.first().copied().unwrap_or(0.0).max(0.0) as usize;
    let want = cells(a);
    let FxCtx {
        sr,
        fft,
        scratch,
        analysis,
        stats,
        ..
    } = ctx;
    let sr = *sr;
    let table = analysis.len();
    let end = base.saturating_add(want).min(table);
    if end < base.saturating_add(want) {
        stats.clamped += 1;
    }
    let out = if base < end {
        &mut analysis[base..end]
    } else {
        &mut []
    };
    match a {
        AnalyzerKind::Level => level(p, st, l, r, out, sr),
        AnalyzerKind::StereoMeter => stereo(st, l, r, out, sr),
        AnalyzerKind::Oscilloscope => scope(p, st, l, r, out),
        AnalyzerKind::Spectrum
        | AnalyzerKind::Spectrogram
        | AnalyzerKind::NoteSpectrogram
        | AnalyzerKind::PitchMeter => {
            for i in 0..n {
                st.dl[0].write(mem, 0.5 * (l[i] + r[i]));
            }
            let half_len = scratch.len() / 2;
            let (re, rest) = scratch.split_at_mut(half_len);
            let quarter = rest.len() / 2;
            let (im, mags) = rest.split_at_mut(quarter);
            if re.len() < FFT_SIZE || im.len() < FFT_SIZE || mags.len() < FFT_SIZE / 2 {
                stats.clamped += 1;
                return;
            }
            spectrum_of(st.dl[0], mem, re, im, mags, fft);
            let half = FFT_SIZE / 2;
            match a {
                AnalyzerKind::Spectrum => bands(&mags[..half], out),
                AnalyzerKind::Spectrogram => {
                    let frame = ring_slot(st, out, SPECTROGRAM_FRAMES, BANDS);
                    bands(&mags[..half], frame);
                }
                AnalyzerKind::NoteSpectrogram => {
                    let frame = ring_slot(st, out, CHROMA_FRAMES, 12);
                    chroma(&mags[..half], frame, sr);
                }
                _ => pitch(&mags[..half], out, sr),
            }
        }
    }
}

/// Fills `mags` with the magnitude spectrum of the last `FFT_SIZE` samples.
fn spectrum_of(
    dl: DelayLine,
    mem: &[f32],
    re: &mut [f32],
    im: &mut [f32],
    mags: &mut [f32],
    fft: &Fft,
) {
    // Oldest first: tap FFT_SIZE is the oldest sample kept.
    #[allow(clippy::cast_precision_loss, clippy::needless_range_loop)]
    for i in 0..FFT_SIZE {
        let w = 0.5 - 0.5 * (TAU * i as f32 / FFT_SIZE as f32).cos();
        re[i] = dl.tap(mem, FFT_SIZE - i) * w;
    }
    im[..FFT_SIZE].fill(0.0);
    let (re, im) = (&mut re[..FFT_SIZE], &mut im[..FFT_SIZE]);
    fft.forward(re, im);
    #[allow(clippy::cast_precision_loss)]
    let scale = 2.0 / FFT_SIZE as f32;
    let half = (FFT_SIZE / 2).min(mags.len()).min(im.len());
    for k in 0..half {
        mags[k] = (re[k] * re[k] + im[k] * im[k]).sqrt() * scale;
    }
}

/// The next frame of a ring of `frames` x `width` cells; the last cell of
/// `out` holds the index of the frame written most recently.
fn ring_slot<'o>(
    st: &mut FxState,
    out: &'o mut [f32],
    frames: usize,
    width: usize,
) -> &'o mut [f32] {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let next = (st.s[0] as usize) % frames;
    st.s[0] = ((next + 1) % frames) as f32;
    let idx = frames * width;
    if let Some(c) = out.get_mut(idx) {
        *c = next as f32;
    }
    let start = (next * width).min(out.len());
    let end = (start + width).min(out.len());
    &mut out[start..end]
}

/// Log-spaced band maxima of `mags` (bins `1..`), linear magnitude.
fn bands(mags: &[f32], out: &mut [f32]) {
    let half = mags.len().max(2);
    for (b, cell) in out.iter_mut().enumerate().take(BANDS) {
        #[allow(clippy::cast_precision_loss)]
        let lo_f = (half as f32).powf(b as f32 / BANDS as f32);
        #[allow(clippy::cast_precision_loss)]
        let hi_f = (half as f32).powf((b + 1) as f32 / BANDS as f32);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (lo, hi) = (lo_f as usize, (hi_f as usize).max(lo_f as usize + 1));
        let mut m = 0.0f32;
        for &x in mags.iter().take(hi.min(mags.len())).skip(lo.max(1)) {
            m = m.max(x);
        }
        *cell = m;
    }
}

/// Pitch-class energies of `mags` (A = 440 Hz, class 0 = C).
fn chroma(mags: &[f32], out: &mut [f32], sr: f32) {
    let mut acc = [0.0f32; 12];
    #[allow(clippy::cast_precision_loss)]
    let bin_hz = sr / FFT_SIZE as f32;
    for (k, &m) in mags.iter().enumerate().skip(1) {
        #[allow(clippy::cast_precision_loss)]
        let f = k as f32 * bin_hz;
        if !(27.5..=5000.0).contains(&f) {
            continue;
        }
        let midi = 69.0 + 12.0 * (f / 440.0).log2();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pc = (midi.round() as i32).rem_euclid(12) as usize;
        acc[pc] += m * m;
    }
    for (cell, a) in out.iter_mut().zip(acc) {
        *cell = a.sqrt();
    }
}

/// The strongest bin's frequency (parabolic interpolation) and a 0..1
/// confidence (peak over total).
fn pitch(mags: &[f32], out: &mut [f32], sr: f32) {
    let mut best = 1;
    let mut total = 0.0f32;
    for (k, &m) in mags.iter().enumerate().skip(1) {
        total += m;
        if m > mags[best] {
            best = k;
        }
    }
    let peak = mags.get(best).copied().unwrap_or(0.0);
    let (a, b, c) = (
        mags.get(best - 1).copied().unwrap_or(0.0),
        peak,
        mags.get(best + 1).copied().unwrap_or(0.0),
    );
    let denom = a - 2.0 * b + c;
    let shift = if denom.abs() > 1.0e-12 {
        (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    #[allow(clippy::cast_precision_loss)]
    let freq = (best as f32 + shift) * sr / FFT_SIZE as f32;
    let conf = if total > 1.0e-9 { peak / total } else { 0.0 };
    if let Some(cell) = out.get_mut(0) {
        *cell = if peak > 1.0e-6 { freq } else { 0.0 };
    }
    if let Some(cell) = out.get_mut(1) {
        *cell = conf.clamp(0.0, 1.0);
    }
}

/// `[rms, peak]` of the mono sum, smoothed over `window` seconds.
fn level(p: &[f32], st: &mut FxState, l: &[f32], r: &[f32], out: &mut [f32], sr: f32) {
    let window = p.get(1).copied().unwrap_or(0.3);
    let a = OnePole::time_coef(window, sr);
    let mut peak = st.s[1] * (1.0 - a).powi(i32::try_from(l.len()).unwrap_or(i32::MAX));
    for (&x, &y) in l.iter().zip(r) {
        let m = 0.5 * (x + y);
        st.op[0].lp(m * m, a);
        peak = peak.max(m.abs());
    }
    st.s[1] = peak;
    if let Some(c) = out.get_mut(0) {
        *c = st.op[0].z.max(0.0).sqrt();
    }
    if let Some(c) = out.get_mut(1) {
        *c = peak;
    }
}

/// `[correlation, width, balance]` over a ~300 ms window.
fn stereo(st: &mut FxState, l: &[f32], r: &[f32], out: &mut [f32], sr: f32) {
    let a = OnePole::time_coef(0.3, sr);
    for (&x, &y) in l.iter().zip(r) {
        st.op[0].lp(x * y, a);
        st.op[1].lp(x * x, a);
        st.op[2].lp(y * y, a);
        let (m, s) = (0.5 * (x + y), 0.5 * (x - y));
        st.op[3].lp(m * m, a);
        st.op[4].lp(s * s, a);
    }
    let (xy, xx, yy) = (st.op[0].z, st.op[1].z.max(0.0), st.op[2].z.max(0.0));
    let norm = (xx * yy).sqrt();
    let corr = if norm > 1.0e-12 { xy / norm } else { 0.0 };
    let (mm, ss) = (st.op[3].z.max(0.0), st.op[4].z.max(0.0));
    let width = if mm + ss > 1.0e-12 {
        ss / (mm + ss)
    } else {
        0.0
    };
    let bal = if xx + yy > 1.0e-12 {
        (yy - xx) / (xx + yy)
    } else {
        0.0
    };
    for (cell, v) in out.iter_mut().zip([corr.clamp(-1.0, 1.0), width, bal]) {
        *cell = v;
    }
}

/// Writes every `decimate`-th mono sample into the scope ring.
fn scope(p: &[f32], st: &mut FxState, l: &[f32], r: &[f32], out: &mut [f32]) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let every = p.get(1).copied().unwrap_or(1.0).max(1.0) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut pos = st.s[0] as usize % SCOPE_LEN;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut count = st.s[1] as u32;
    for (&x, &y) in l.iter().zip(r) {
        count += 1;
        if count >= every {
            count = 0;
            if let Some(c) = out.get_mut(pos) {
                *c = 0.5 * (x + y);
            }
            pos = (pos + 1) % SCOPE_LEN;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    {
        st.s[0] = pos as f32;
        st.s[1] = count as f32;
        if let Some(c) = out.get_mut(SCOPE_LEN) {
            *c = pos as f32;
        }
    }
}
