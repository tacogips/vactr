//! Plaits position-12 additive amplitude stages translated from Emilie Gillet's
//! MIT-licensed engine. Analytic sine replaces the generated oscillator table.
//! Host-rate smoothing replaces the source's 12-sample, 48 kHz block update.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const MAIN_PARTIALS: usize = 24;
const ORGAN_PARTIALS: usize = 8;
const STATE_FLOATS: usize = MAIN_PARTIALS * 2;
// One-based form of the source's zero-based, MIT-covered organ map.
const ORGAN_STOPS: [u8; ORGAN_PARTIALS] = [1, 2, 3, 4, 6, 8, 10, 12];

fn bounded(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

/// Source control mapping: `harmonics` couples cubic slope and bump depth.
fn shape_controls(morph: f32, harmonics: f32) -> (f32, f32) {
    let raw_slope = (1.0 - 0.6 * harmonics) * morph;
    (
        0.01 + 1.99 * raw_slope.powi(3),
        16.0 * harmonics * harmonics,
    )
}

/// Source centroid, margin, rectified squared slope and fourth-power bump.
#[allow(clippy::cast_precision_loss)]
fn spectral_gain(index: usize, count: usize, centroid: f32, slope: f32, bumps: f32) -> f32 {
    let n = (count - 1) as f32;
    let margin = (1.0 / slope - 1.0) / (1.0 + bumps);
    let center = centroid * (n + margin) - 0.5 * margin;
    let order = ((index as f32) - center).abs() * slope;
    let rectified = (1.0 - order).max(0.0);
    let squared = 4.0 * rectified * rectified;
    let bump = 1.0 + (TAU * (0.25 + order * bumps)).sin();
    (squared * bump).powi(4)
}

/// The source one-pole coefficient is 0.001 once per 12 samples at 48 kHz.
/// Continuous-time conversion keeps the envelope time constant stable when
/// the host callback block or sample rate differs.
fn smooth_coefficient(sr: f32) -> f32 {
    1.0 - 0.999_f32.powf(48_000.0 / (12.0 * sr))
}

fn harmonic(index: usize, organ: bool) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    if organ {
        f32::from(ORGAN_STOPS[index])
    } else {
        (index + 1) as f32
    }
}

fn nyquist_taper(pitch: f32, order: f32, sr: f32) -> f32 {
    1.0 - (pitch * order / sr).min(0.5) * 2.0
}

/// Mode zero emits 24 integer partials; mode one emits eight organ stops.
/// Each mode reserves 48 floats for phase and smoothed raw amplitude state.
/// The main/aux graph uses two such nodes and has a 96-float install budget.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    let (phases, amplitudes) = mem[..STATE_FLOATS].split_at_mut(MAIN_PARTIALS);
    let smooth = smooth_coefficient(sr);
    for (i, y) in out.iter_mut().enumerate() {
        let pitch = bounded(ins[0].at(i), 20.0, sr * 0.4, 220.0);
        let centroid = bounded(ins[1].at(i), 0.0, 1.0, 0.5);
        let morph = bounded(ins[2].at(i), 0.0, 1.0, 0.5);
        let harmonics = bounded(ins[3].at(i), 0.0, 1.0, 0.5);
        let organ = ins[4].at(i) >= 0.5;
        let count = if organ { ORGAN_PARTIALS } else { MAIN_PARTIALS };
        let (slope, bumps) = shape_controls(morph, harmonics);
        let mut sum = 0.001;
        for (j, amplitude) in amplitudes.iter_mut().take(count).enumerate() {
            let gain = spectral_gain(j, count, centroid, slope, bumps);
            *amplitude += smooth * (gain - *amplitude);
            sum += *amplitude;
        }
        let inv_sum = 1.0 / sum;
        let mut output = 0.0;
        for j in 0..count {
            let order = harmonic(j, organ);
            let taper = nyquist_taper(pitch, order, sr);
            phases[j] = (phases[j] + pitch * order / sr).fract();
            output += amplitudes[j] * inv_sum * taper * (TAU * phases[j]).sin();
        }
        *y = output.clamp(-1.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_stage_control_law_and_harmonic_map() {
        let (slope, bumps) = shape_controls(1.0, 0.0);
        assert!((slope - 2.0).abs() < 1.0e-6);
        assert_eq!(bumps, 0.0);
        let (slope, bumps) = shape_controls(1.0, 1.0);
        assert!((slope - 0.137_36).abs() < 1.0e-5);
        assert_eq!(bumps, 16.0);
        assert_eq!(harmonic(7, true), 12.0);
        assert_eq!(harmonic(23, false), 24.0);
        let peak = spectral_gain(0, 24, 0.0, 2.0, 0.0);
        assert!((peak - 16.0).abs() < 1.0e-3);
        assert_eq!(spectral_gain(23, 24, 0.0, 2.0, 0.0), 0.0);
        assert!((nyquist_taper(1_000.0, 12.0, 48_000.0) - 0.5).abs() < 1.0e-6);
        assert_eq!(nyquist_taper(1_000.0, 24.0, 48_000.0), 0.0);
    }

    #[test]
    fn host_rate_smoothing_matches_source_time_constant() {
        let a48 = smooth_coefficient(48_000.0);
        let a96 = smooth_coefficient(96_000.0);
        let residual48 = (1.0 - a48).powi(48_000);
        let residual96 = (1.0 - a96).powi(96_000);
        assert!((residual48 - residual96).abs() < 0.001);
        assert!((residual48 - 0.999_f32.powi(4000)).abs() < 0.001);
    }
}
