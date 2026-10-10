//! VOSIM pulse-train kernel based on the model in
//! `design-docs/specs/design-fm1-voices.md`.

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Number of ports in the pinned contract.
pub const PORT_COUNT: usize = 4;
/// Stable port names and defaults; order is part of the template contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 440.0),
    ("vosim-formant", 900.0),
    ("vosim-pulses", 3.0),
    ("vosim-decay", 0.7),
];

/// Stable indices for the named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const VOSIM_FORMANT: usize = 1;
    pub const VOSIM_PULSES: usize = 2;
    pub const VOSIM_DECAY: usize = 3;
}

const PHASE: usize = 0;
const PULSES: usize = 1;
const DC_INPUT: usize = 2;
const DC_OUTPUT: usize = 3;

/// Number of floats in the per-voice phase and DC-blocker state.
pub const STATE_FLOATS: usize = 4;

/// Renders a period-latched train of decaying sin-squared pulses.
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

    let sample_rate = if kx.sr.is_finite() {
        kx.sr.max(1.0)
    } else {
        48_000.0
    };
    let dc_coefficient = (-std::f32::consts::TAU * 10.0 / sample_rate).exp();
    let mut phase = finite_or(mem[PHASE], 0.0).clamp(0.0, 1.0 - f32::EPSILON);
    let mut pulse_count = finite_or(mem[PULSES], 0.0).round().clamp(0.0, 8.0) as usize;
    let mut previous_input = finite_or(mem[DC_INPUT], 0.0);
    let mut previous_output = finite_or(mem[DC_OUTPUT], 0.0);

    for (frame, sample) in out.iter_mut().enumerate() {
        let frequency = finite_or(ins[port::FREQ].at(frame), 440.0).clamp(20.0, 8_000.0);
        let period = 1.0 / frequency;
        if pulse_count == 0 {
            pulse_count = requested_pulse_count(ins[port::VOSIM_PULSES].at(frame));
        }

        let formant = finite_or(ins[port::VOSIM_FORMANT].at(frame), 900.0).clamp(100.0, 8_000.0);
        let pulse_width = (1.0 / formant).min(period / pulse_count as f32);
        let time = phase * period;
        let pulse_index = (time / pulse_width).floor() as usize;
        let raw = if pulse_index < pulse_count {
            let within_pulse =
                ((time - pulse_index as f32 * pulse_width) / pulse_width).clamp(0.0, 1.0);
            let pulse = (std::f32::consts::PI * within_pulse).sin();
            let decay = finite_or(ins[port::VOSIM_DECAY].at(frame), 0.7).clamp(0.0, 1.0);
            decay.powi(pulse_index as i32) * pulse * pulse
        } else {
            0.0
        };

        let filtered = raw - previous_input + dc_coefficient * previous_output;
        let bounded = finite_or(filtered, 0.0).clamp(-1.0, 1.0);
        *sample = bounded;
        previous_input = raw;
        previous_output = bounded;

        phase += frequency / sample_rate;
        if phase >= 1.0 {
            phase -= phase.floor();
            pulse_count = requested_pulse_count(ins[port::VOSIM_PULSES].at(frame));
        }
    }

    mem[PHASE] = phase;
    mem[PULSES] = pulse_count as f32;
    mem[DC_INPUT] = previous_input;
    mem[DC_OUTPUT] = previous_output;
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn requested_pulse_count(value: f32) -> usize {
    finite_or(value, 3.0).round().clamp(1.0, 8.0) as usize
}
