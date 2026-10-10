//! GENDYN dynamic stochastic synthesis, using fixed-capacity second-order
//! mirrored walks for breakpoint amplitudes and durations.

use std::f32::consts::PI;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Number of ports in the pinned contract.
pub const PORT_COUNT: usize = 6;
/// Stable port names and defaults; order is part of the template contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 440.0),
    ("gendyn-points", 12.0),
    ("gendyn-amp-step", 0.2),
    ("gendyn-dur-step", 0.1),
    ("gendyn-spread", 0.1),
    ("gendyn-dist", 1.0),
];

/// Stable indices for the named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const GENDYN_POINTS: usize = 1;
    pub const GENDYN_AMP_STEP: usize = 2;
    pub const GENDYN_DUR_STEP: usize = 3;
    pub const GENDYN_SPREAD: usize = 4;
    pub const GENDYN_DIST: usize = 5;
}

const MAX_POINTS: usize = 32;
const PHASE_BOUNDARY_EPSILON: f32 = 1.0e-6;
const AMP_BASE: usize = 0;
const AMP_VEL_BASE: usize = AMP_BASE + MAX_POINTS;
const DUR_BASE: usize = AMP_VEL_BASE + MAX_POINTS;
const DUR_VEL_BASE: usize = DUR_BASE + MAX_POINTS;
const SEGMENT_INDEX: usize = DUR_VEL_BASE + MAX_POINTS;
const SEGMENT_PHASE: usize = SEGMENT_INDEX + 1;
const LATCHED_POINTS: usize = SEGMENT_PHASE + 1;
const DC_INPUT: usize = LATCHED_POINTS + 1;
const DC_OUTPUT: usize = DC_INPUT + 1;
/// Four 32-point arrays plus eight scalar slots reserved for kernel state.
pub const STATE_FLOATS: usize = 4 * MAX_POINTS + 8;

#[cfg(test)]
pub(crate) fn test_segment_index(mem: &[f32]) -> usize {
    mem[SEGMENT_INDEX] as usize
}

#[cfg(test)]
pub(crate) fn test_period_duration(mem: &[f32], points: usize) -> f32 {
    mem[DUR_BASE..DUR_BASE + points].iter().sum()
}

#[inline]
fn finite_clamp(value: f32, fallback: f32, low: f32, high: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
}

#[inline]
fn mirrored(value: f32, low: f32, high: f32) -> f32 {
    let width = high - low;
    if width <= f32::EPSILON || !value.is_finite() {
        return low;
    }
    let folded = (value - low).rem_euclid(2.0 * width);
    if folded <= width {
        low + folded
    } else {
        high - (folded - width)
    }
}

#[inline]
fn distribution_step(rng: &mut Rng, distribution: usize) -> f32 {
    let u = rng.unit().clamp(1.0e-6, 1.0 - 1.0e-6);
    let raw = match distribution {
        1 => (PI * (u - 0.5)).tan().clamp(-8.0, 8.0) / 8.0,
        2 => (u / (1.0 - u)).ln().clamp(-8.0, 8.0) / 8.0,
        3 => (PI * u * 0.5).tan().ln().clamp(-8.0, 8.0) / 8.0,
        _ => 2.0 * u - 1.0,
    };
    finite_clamp(raw, 0.0, -1.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn update_period(
    mem: &mut [f32],
    rng: &mut Rng,
    points: usize,
    base_duration: f32,
    sample_rate: f32,
    amp_step: f32,
    dur_step: f32,
    spread: f32,
    distribution: usize,
) {
    for index in 0..points {
        let amp_velocity = mirrored(
            mem[AMP_VEL_BASE + index] + distribution_step(rng, distribution) * amp_step,
            -amp_step,
            amp_step,
        );
        mem[AMP_VEL_BASE + index] = amp_velocity;
        mem[AMP_BASE + index] = mirrored(mem[AMP_BASE + index] + amp_velocity, -1.0, 1.0);

        if spread == 0.0 {
            mem[DUR_VEL_BASE + index] = 0.0;
            mem[DUR_BASE + index] = base_duration;
        } else {
            let low = 1.0 - spread;
            let high = 1.0 + spread;
            let dur_velocity = mirrored(
                mem[DUR_VEL_BASE + index] + distribution_step(rng, distribution) * dur_step,
                -dur_step,
                dur_step,
            );
            mem[DUR_VEL_BASE + index] = dur_velocity;
            let factor = mirrored(
                mem[DUR_BASE + index] / base_duration + dur_velocity,
                low,
                high,
            );
            // Positive spread permits zero in the model's lower bound. Keep
            // segments at least one sample long to bound audio-thread work.
            mem[DUR_BASE + index] = (factor * base_duration).max(1.0 / sample_rate);
        }
    }
}

/// Render the cyclic, linearly interpolated breakpoint waveform.
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
    let sr = finite_clamp(kx.sr, 48_000.0, 1.0, f32::MAX);
    let frequency = finite_clamp(ins[port::FREQ].first(), 440.0, 1.0, sr * 0.49);
    let points = finite_clamp(ins[port::GENDYN_POINTS].first().round(), 12.0, 3.0, 32.0) as usize;
    let amp_step = finite_clamp(ins[port::GENDYN_AMP_STEP].first(), 0.2, 0.0, 1.0);
    let dur_step = finite_clamp(ins[port::GENDYN_DUR_STEP].first(), 0.1, 0.0, 1.0);
    let spread = finite_clamp(ins[port::GENDYN_SPREAD].first(), 0.1, 0.0, 1.0);
    let distribution = finite_clamp(ins[port::GENDYN_DIST].first().round(), 1.0, 0.0, 3.0) as usize;
    let base_duration = 1.0 / (frequency * points as f32);

    if st.u[1] == 0 {
        let mut rng = Rng::new(kx.seed ^ 0x4745_4E44);
        for index in 0..MAX_POINTS {
            mem[AMP_BASE + index] = rng.bipolar() * 0.5;
            mem[DUR_BASE + index] = base_duration;
            mem[AMP_VEL_BASE + index] = 0.0;
            mem[DUR_VEL_BASE + index] = 0.0;
        }
        mem[SEGMENT_INDEX] = 0.0;
        mem[SEGMENT_PHASE] = 0.0;
        mem[LATCHED_POINTS] = points as f32;
        mem[DC_INPUT] = 0.0;
        mem[DC_OUTPUT] = 0.0;
        st.u[0] = rng.s;
        st.u[1] = 1;
    }

    let mut rng = Rng::new(st.u[0]);
    let mut segment = (mem[SEGMENT_INDEX] as usize).min(MAX_POINTS - 1);
    let mut phase = finite_clamp(mem[SEGMENT_PHASE], 0.0, 0.0, 1.0);
    let mut latched_points = finite_clamp(mem[LATCHED_POINTS], 12.0, 3.0, 32.0) as usize;
    let mut previous_input = mem[DC_INPUT];
    let mut dc_output = mem[DC_OUTPUT];
    let dc_coefficient = (-std::f32::consts::TAU * 10.0 / sr).exp();

    for sample in out {
        let next = if segment + 1 == latched_points {
            0
        } else {
            segment + 1
        };
        let start = mem[AMP_BASE + segment];
        let end = mem[AMP_BASE + next];
        let raw = start + (end - start) * phase;
        let bounded = finite_clamp(raw, 0.0, -1.0, 1.0);
        dc_output = bounded - previous_input + dc_coefficient * dc_output;
        previous_input = bounded;
        *sample = finite_clamp(dc_output, 0.0, -1.0, 1.0);

        let duration = finite_clamp(
            mem[DUR_BASE + segment],
            base_duration,
            f32::MIN_POSITIVE,
            f32::MAX,
        );
        phase += 1.0 / (duration * sr);
        while phase >= 1.0 - PHASE_BOUNDARY_EPSILON {
            let overshoot = phase - 1.0;
            phase = if overshoot.abs() <= PHASE_BOUNDARY_EPSILON {
                0.0
            } else {
                overshoot.max(0.0)
            };
            segment += 1;
            if segment >= latched_points {
                segment = 0;
                latched_points = points;
                mem[LATCHED_POINTS] = points as f32;
                update_period(
                    mem,
                    &mut rng,
                    latched_points,
                    base_duration,
                    sr,
                    amp_step,
                    dur_step,
                    spread,
                    distribution,
                );
            }
        }
    }

    mem[SEGMENT_INDEX] = segment as f32;
    mem[SEGMENT_PHASE] = phase;
    mem[DC_INPUT] = previous_input;
    mem[DC_OUTPUT] = dc_output;
    st.u[0] = rng.s;
}
