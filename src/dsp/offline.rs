//! Analysis over sample slices (design 14.5.9): `rms`, `peak`, `scope` and
//! `spectrum` of a captured or rendered buffer, and of a live tap snapshot.
//!
//! These run the same `fft.rs` transform the analyzer units use (Hann
//! window, `2 / n` magnitude scale), over a slice instead of a delay line,
//! so the live and offline spectra agree. Evaluator side only: they
//! allocate their results.

use crate::dsp::fft::Fft;

/// The sample rate of an offline render, Hz.
pub const OFFLINE_RATE: u32 = 48_000;

/// The root mean square of `frames` (every sample, so both channels of
/// interleaved stereo); 0 when empty.
#[must_use]
pub fn rms(frames: &[f32]) -> f32 {
    if frames.is_empty() {
        return 0.0;
    }
    let sum: f64 = frames.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let r = (sum / frames.len() as f64).sqrt() as f32;
    r
}

/// The largest absolute sample of `frames`; 0 when empty.
#[must_use]
pub fn peak(frames: &[f32]) -> f32 {
    frames.iter().fold(0.0, |m, &x| m.max(x.abs()))
}

/// The first `n` samples of `frames` (all of them when shorter).
#[must_use]
pub fn scope(frames: &[f32], n: usize) -> Vec<f32> {
    frames[..n.min(frames.len())].to_vec()
}

/// `bins` linear-bin magnitudes of mono `frames`: one FFT of `2 * bins`
/// samples per consecutive window, averaged over the windows. A slice
/// shorter than one window is zero-padded (one window).
#[must_use]
pub fn spectrum(frames: &[f32], bins: usize) -> Vec<f32> {
    let n = (2 * bins.max(1)).next_power_of_two();
    let fft = Fft::new(n);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    let mut mags = vec![0.0; n / 2];
    let mut acc = vec![0.0; n / 2];
    let windows = (frames.len() / n).max(1);
    for w in 0..windows {
        let start = (w * n).min(frames.len());
        let end = (start + n).min(frames.len());
        fft.magnitudes(&frames[start..end], &mut re, &mut im, &mut mags);
        for (a, m) in acc.iter_mut().zip(&mags) {
            *a += m;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let scale = 1.0 / windows as f32;
    acc.iter_mut().for_each(|a| *a *= scale);
    acc.truncate(bins);
    acc
}
