//! Two grainlets on main and one Z oscillator on auxiliary.
//!
//! Source-stage translation of Emilie Gillet's MIT Plaits position 11
//! oscillators and stmlib one-pole high-pass. Analytic sine replaces the
//! generated sine table. Per-sample Vactr controls replace source block
//! interpolation, so the output is not numerically identical.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::{PI, TAU};

/// Independent node instance: 2 grainlets use 3 floats each and one HP
/// state; the auxiliary Z oscillator uses 4 floats and one HP state.
pub const STATE_FLOATS: usize = 8;
const HP_STATE: usize = 6;

#[inline]
fn sine(phase: f32) -> f32 {
    (TAU * phase).sin()
}

#[inline]
fn carrier(mut phase: f32, shape: f32) -> f32 {
    let position = shape.clamp(0.0, 1.0) * 3.0;
    let segment = position.floor();
    let mut t = 1.0 - position.fract();
    if segment < 1.0 {
        phase = (phase * (1.0 + t.powi(3) * 15.0)).min(1.0) + 0.75;
    } else if segment < 2.0 {
        let breakpoint = 0.001 + 0.499 * t.powi(3);
        phase = if phase < breakpoint {
            phase * 0.5 / breakpoint
        } else {
            0.5 + (phase - breakpoint) * 0.5 / (1.0 - breakpoint)
        } + 0.75;
    } else {
        t = 1.0 - t;
        phase = (0.25 + phase * (0.5 + t.powi(3) * 14.5)).min(0.75);
    }
    (sine(phase) + 1.0) * 0.25
}

#[inline]
fn grain_wave(c: f32, f: f32, shape: f32, bleed: f32) -> f32 {
    carrier(c, shape) * (sine(f) + bleed) / (1.0 + bleed)
}

#[inline]
fn blep(t: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0);
    (0.5 * t * t, -0.5 * (1.0 - t).powi(2))
}

#[inline]
fn grainlet(mem: &mut [f32], offset: usize, f0: f32, f1: f32, shape: f32, bleed: f32) -> f32 {
    let mut current = mem[offset + 2];
    let mut pending = 0.0;
    let mut carrier_phase = mem[offset] + f0;
    let mut formant_phase = mem[offset + 1];
    if carrier_phase >= 1.0 {
        carrier_phase -= 1.0;
        let crossing = carrier_phase / f0;
        let before = grain_wave(1.0, formant_phase + (1.0 - crossing) * f1, shape, bleed);
        let after = grain_wave(0.0, 0.0, shape, bleed);
        let jump = after - before;
        let (this_blep, next_blep) = blep(crossing);
        current += jump * this_blep;
        pending += jump * next_blep;
        formant_phase = crossing * f1;
    } else {
        formant_phase = (formant_phase + f1).fract();
    }
    pending += grain_wave(carrier_phase, formant_phase, shape, bleed);
    mem[offset] = carrier_phase;
    mem[offset + 1] = formant_phase;
    mem[offset + 2] = pending;
    current
}

#[inline]
fn z_wave(c: f32, d: f32, f: f32, mut shape: f32, mode: f32) -> f32 {
    let mut ramp = 0.5 * (1.0 + sine(0.5 * d + 0.25));
    let (offset, phase_shift) = if mode < 0.333 {
        (1.0, 0.25 + mode * 1.5)
    } else if mode < 0.666 {
        let shift = 0.7495 - (mode - 0.33) * 0.75;
        (-sine(shift), shift)
    } else {
        (0.001, 0.7495 - (mode - 0.33) * 0.75)
    };
    let contour = if shape < 0.5 {
        shape *= 2.0;
        if c >= 0.5 {
            ramp *= shape;
        }
        1.0 + (sine(c + 0.25) - 1.0) * shape
    } else {
        sine(c + shape * 0.5)
    };
    (ramp * (offset + sine(f + phase_shift)) - offset) * contour
}

#[inline]
fn z_osc(mem: &mut [f32], f0: f32, f1: f32, shape: f32, mode: f32) -> f32 {
    let mut current = mem[3];
    let mut pending = 0.0;
    let mut carrier_phase = mem[0] + f0;
    let mut discontinuity_phase = mem[1] + 2.0 * f0;
    let mut formant_phase = mem[2];
    if discontinuity_phase >= 1.0 {
        discontinuity_phase -= 1.0;
        let crossing = discontinuity_phase / (2.0 * f0);
        let upper_half = carrier_phase >= 1.0;
        let before = z_wave(
            if upper_half { 1.0 } else { 0.5 },
            1.0,
            formant_phase + (1.0 - crossing) * f1,
            shape,
            mode,
        );
        let after = z_wave(if upper_half { 0.0 } else { 0.5 }, 0.0, 0.0, shape, mode);
        let (this_blep, next_blep) = blep(crossing);
        let jump = after - before;
        current += jump * this_blep;
        pending += jump * next_blep;
        formant_phase = crossing * f1;
        if carrier_phase > 1.0 {
            carrier_phase = discontinuity_phase * 0.5;
        }
    } else {
        formant_phase = (formant_phase + f1).fract();
    }
    if carrier_phase >= 1.0 {
        carrier_phase -= 1.0;
    }
    pending += z_wave(
        carrier_phase,
        discontinuity_phase,
        formant_phase,
        shape,
        mode,
    );
    mem[0] = carrier_phase;
    mem[1] = discontinuity_phase;
    mem[2] = formant_phase;
    mem[3] = pending;
    current
}

#[inline]
fn high_pass(input: f32, state: &mut f32, normalized_hz: f32) -> f32 {
    // stmlib OnePole<FREQUENCY_DIRTY>: trapezoidal one-pole with a cubic
    // approximation to tan(pi*f). f is far below the approximation limit.
    let f = normalized_hz.clamp(0.0, 0.16);
    let g = f * (PI + 0.3736 * PI.powi(3) * f * f);
    let lp = (g * input + *state) / (1.0 + g);
    *state = g * (input - lp) + lp;
    input - lp
}

fn bounded(input: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if input.is_finite() {
        input.clamp(lo, hi)
    } else {
        fallback
    }
}

/// Mode 0 sums two grainlets; mode 1 renders the independently filtered Z path.
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
    let auxiliary = ins[4].first() >= 0.5;
    for (frame, sample) in out.iter_mut().enumerate() {
        let freq = bounded(ins[0].at(frame), 20.0, sr * 0.125, 220.0);
        let harmonics = bounded(ins[1].at(frame), 0.0, 1.0, 0.4);
        let timbre = bounded(ins[2].at(frame), 0.0, 1.0, 0.5);
        let morph = bounded(ins[3].at(frame), 0.0, 1.0, 0.5);
        let f0 = (freq / sr).min(0.125);
        let f1 = (32.703_197 * 2.0_f32.powf(7.0 * timbre) / sr).min(0.25);
        let ratio = 2.0_f32.powf(4.0 * harmonics - 2.0);
        let bleed = (1.0 - 2.0 * harmonics).max(0.0);
        let bleed = bleed * (2.0 - bleed);
        let shape = 0.33 + (morph - 0.33) * (1.0 - 24.0 * f0).max(0.0);
        let cutoff = (freq * 2.0_f32.powf(8.0 * timbre) / sr).min(0.25);
        let raw = if auxiliary {
            z_osc(mem, f0, cutoff, morph, harmonics)
        } else {
            grainlet(mem, 0, f0, f1, shape, bleed)
                + grainlet(mem, 3, f0, (f1 * ratio).min(0.25), shape, bleed)
        };
        *sample = high_pass(raw, &mut mem[HP_STATE], 0.3 * f0);
    }
}

#[cfg(test)]
mod tests {
    use super::{blep, grain_wave, grainlet, high_pass, z_osc, z_wave};

    #[test]
    fn quadratic_blep_spans_current_and_next_sample() {
        assert_eq!(blep(0.0), (0.0, -0.5));
        assert_eq!(blep(1.0), (0.5, 0.0));
        assert_eq!(blep(0.5), (0.125, -0.125));
    }

    #[test]
    fn dirty_one_pole_high_pass_rejects_dc() {
        let mut state = 0.0;
        let out: Vec<_> = (0..4096)
            .map(|_| high_pass(1.0, &mut state, 0.01))
            .collect();
        assert!(out[0] > 0.9);
        assert!(out[4095].abs() < 1.0e-5);
        assert!(out.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn grainlet_reset_corrects_current_and_pending_samples() {
        let mut state = [0.95, 0.2, 0.0];
        let (this, next) = blep(0.5);
        let jump = grain_wave(0.0, 0.0, 0.4, 0.2) - grain_wave(1.0, 0.2 + 0.5 * 0.08, 0.4, 0.2);
        let output = grainlet(&mut state, 0, 0.1, 0.08, 0.4, 0.2);
        assert!((output - jump * this).abs() < 1.0e-6);
        assert!((state[2] - (grain_wave(0.05, 0.04, 0.4, 0.2) + jump * next)).abs() < 1.0e-6);
        assert!((state[0] - 0.05).abs() < 1.0e-6);
    }

    #[test]
    fn z_half_cycle_reset_corrects_current_and_pending_samples() {
        let mut state = [0.45, 0.9, 0.2, 0.0];
        let (this, next) = blep(0.5);
        let jump = z_wave(0.5, 0.0, 0.0, 0.4, 0.3) - z_wave(0.5, 1.0, 0.2 + 0.5 * 0.08, 0.4, 0.3);
        let output = z_osc(&mut state, 0.1, 0.08, 0.4, 0.3);
        assert!((output - jump * this).abs() < 1.0e-6);
        assert!((state[3] - (z_wave(0.55, 0.1, 0.04, 0.4, 0.3) + jump * next)).abs() < 1.0e-6);
        assert!((state[1] - 0.1).abs() < 1.0e-6);
    }
}
