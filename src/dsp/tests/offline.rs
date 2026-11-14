//! SS-ANALYSIS: analysis over sample slices (design 14.5.9) against
//! closed-form signals, and the native tier's offline-render flag.

use std::f32::consts::{FRAC_1_SQRT_2, TAU};

use crate::dsp::caps::{Cap, CapabilitySet};
use crate::dsp::offline::{peak, rms, scope, spectrum, OFFLINE_RATE};

/// `n` samples of a sine of amplitude `a` at `bin` cycles per `period`.
fn sine(n: usize, a: f32, bin: usize, period: usize) -> Vec<f32> {
    #[allow(clippy::cast_precision_loss)]
    (0..n)
        .map(|i| a * (TAU * (bin * i) as f32 / period as f32).sin())
        .collect()
}

#[test]
fn rms_and_peak_of_closed_form_signals() {
    // A whole number of periods of a unit sine: rms 1/sqrt(2), peak 1.
    let s = sine(4096, 1.0, 32, 4096);
    assert!((rms(&s) - FRAC_1_SQRT_2).abs() < 1e-4, "{}", rms(&s));
    assert!((peak(&s) - 1.0).abs() < 1e-4);
    // DC: rms and peak are the level; a negative level's peak is absolute.
    assert!((rms(&[0.25; 100]) - 0.25).abs() < 1e-6);
    assert!((peak(&[0.1, -0.75, 0.5]) - 0.75).abs() < 1e-6);
    assert_eq!((rms(&[]), peak(&[])), (0.0, 0.0));
}

#[test]
fn scope_takes_the_first_frames() {
    let s = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(scope(&s, 2), vec![1.0, 2.0]);
    assert_eq!(scope(&s, 10), s.to_vec());
}

#[test]
fn spectrum_peaks_at_the_signal_bin_and_averages_windows() {
    // Bin 5 of a 128-point FFT (64 bins), four whole windows.
    let s = sine(512, 0.5, 5, 128);
    let m = spectrum(&s, 64);
    assert_eq!(m.len(), 64);
    let top = m
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, _)| k);
    assert_eq!(top, Some(5));
    // The Hann window halves a bin-centred sine's magnitude: 0.5 * a.
    assert!((m[5] - 0.25).abs() < 1e-3, "{}", m[5]);
    // One window of the same signal gives the same average.
    let one = spectrum(&s[..128], 64);
    assert!((one[5] - m[5]).abs() < 1e-4);
    // A short slice is zero-padded to one window.
    assert_eq!(spectrum(&[0.0; 10], 8), vec![0.0; 8]);
}

#[test]
fn the_native_preset_has_offline_render_and_the_browser_does_not() {
    assert!(CapabilitySet::native().offline_render);
    assert!(CapabilitySet::native()
        .require(Cap::OfflineRender, None)
        .is_ok());
    assert!(!CapabilitySet::browser().offline_render);
    assert_eq!(OFFLINE_RATE, 48_000);
}
