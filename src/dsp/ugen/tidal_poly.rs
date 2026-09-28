//! Original four-role Tides2 ramp adaptation with selectable output lanes.
//!
//! Each node computes four bounded channels, selecting one for its mono graph
//! output. Two nodes can route any two channels to main and aux. This does not
//! provide four simultaneous host channels or reproduce source LUT/BLEP DSP.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 16;

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn shaped(phase: f32, pw: f32, shape: f32) -> f32 {
    let raw = if phase < pw {
        phase / pw
    } else {
        1.0 - (phase - pw) / (1.0 - pw)
    };
    let exponent = 2.0_f32.powf((shape - 0.5) * 3.0);
    raw.clamp(0.0, 1.0).powf(exponent)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
    let sr = kx.sr.max(1.0);
    for (frame, sample) in out.iter_mut().enumerate() {
        let pw = (0.04 + unit(ins[1].at(frame)) * 0.92).clamp(0.04, 0.96);
        let shape = unit(ins[2].at(frame));
        let smoothness = unit(ins[3].at(frame));
        let shift = unit(ins[4].at(frame));
        let gate = ins[5].at(frame) >= 0.5;
        let clock = ins[6].at(frame) >= 0.5;
        let mode = ins[7].at(frame).round().clamp(0.0, 2.0) as u8;
        let output_mode = ins[8].at(frame).round().clamp(0.0, 3.0) as u8;
        let range = ins[9].at(frame) >= 0.5;
        let channel = ins[10].at(frame).round().clamp(0.0, 3.0) as usize;
        let rising = gate && mem[8] < 0.5;
        let clock_rising = clock && mem[9] < 0.5;
        mem[8] = if gate { 1.0 } else { 0.0 };
        mem[9] = if clock { 1.0 } else { 0.0 };
        if mode == 1 && st.u[0] == 0 {
            st.u[0] = 1;
        }
        if rising || clock_rising {
            mem[..4].fill(0.0);
            mem[12..16].fill(1.0);
            st.u[0] = 1;
        }
        let base_hz = ins[0].at(frame).clamp(0.01, sr * 0.24) * if range { 1.0 } else { 0.02 };
        let step = shift * 2.0 - 1.0;
        let mut channels = [0.0; 4];
        for lane in 0..4 {
            let lane_fraction = lane as f32 / 3.0;
            let ratio = if output_mode == 3 {
                2.0_f32.powf((lane_fraction - 0.5) * (0.4 + step * 2.0))
            } else {
                1.0
            };
            let hz = (base_hz * ratio).clamp(0.0001, sr * 0.24);
            let previous = mem[lane];
            let mut phase = previous;
            let pulse_width = if output_mode == 2 && mode == 2 {
                (pw + step * lane_fraction * 0.18).clamp(0.04, 0.96)
            } else {
                pw
            };
            if mode == 1 || mem[12 + lane] >= 0.5 {
                if mode == 2 && gate && phase >= pulse_width {
                    phase = pulse_width;
                } else {
                    if mode == 2 && !gate && phase < pulse_width {
                        phase = pulse_width;
                    }
                    phase += hz / sr;
                }
                if phase >= 1.0 {
                    if mode == 1 {
                        phase = phase.fract();
                    } else {
                        phase = 1.0;
                        mem[12 + lane] = 0.0;
                    }
                }
            }
            mem[lane] = phase;
            let phase_shift = if output_mode == 2 {
                step * lane as f32 * 0.19
            } else {
                0.0
            };
            let shifted = (phase + phase_shift).rem_euclid(1.0);
            let raw = shaped(shifted, pulse_width, shape);
            let value = match output_mode {
                0 => match lane {
                    0 => (raw * (0.35 + shift * 0.65) * 2.0 - 1.0).tanh(),
                    1 => raw * 2.0 - 1.0,
                    2 => {
                        if phase >= pulse_width {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    _ => {
                        if phase >= 0.98 || (phase < previous && mode == 1) {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                },
                1 => {
                    let focus = shift * 3.0;
                    let gain = (0.04
                        + lane as f32 * 0.02
                        + (1.0 - (lane as f32 - focus).abs()).max(0.0) * 0.88)
                        .min(1.0);
                    (raw * 2.0 - 1.0) * gain
                }
                2 => raw * 2.0 - 1.0,
                _ => (TAU * phase).sin() * (0.6 + shape * 0.4),
            };
            let alpha = 1.0 - (-1.0 / (sr * (0.00002 + (1.0 - smoothness) * 0.015))).exp();
            mem[4 + lane] += alpha * (value - mem[4 + lane]);
            channels[lane] = mem[4 + lane].clamp(-1.0, 1.0);
        }
        *sample = channels[channel];
    }
}
