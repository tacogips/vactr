//! Original Peaks envelope/LFO/tap-LFO functional adaptations.
//!
//! Source functions use fixed-point increments, generated wave/fold tables,
//! and shared processor flags. This kernel uses analytic curves and seeded
//! noise with caller-owned fixed state, without importing source resources.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 32;

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn random(seed: &mut f32) -> f32 {
    let next = ((*seed as u32).wrapping_mul(251).wrapping_add(37)) & 0xffff;
    *seed = next as f32;
    next as f32 / 32_767.5 - 1.0
}

fn envelope(mem: &mut [f32], gate: bool, trigger: bool, half: bool, p: &[f32; 4], sr: f32) -> f32 {
    let stage = mem[2] as u8;
    let mut stage = if trigger || (gate && mem[3] < 0.5) {
        1
    } else {
        stage
    };
    if !half && !gate && mem[3] >= 0.5 && stage != 0 {
        stage = 4;
    }
    mem[3] = if gate { 1.0 } else { 0.0 };
    let target = match stage {
        1 => 1.0,
        2 => {
            if half {
                0.0
            } else {
                p[2]
            }
        }
        3 => p[2],
        4 => 0.0,
        _ => 0.0,
    };
    let seconds = match stage {
        1 => p[0],
        2 => p[1],
        4 => p[3],
        _ => 1.0,
    };
    if matches!(stage, 1 | 2 | 4) {
        let alpha = 1.0 - (-5.0 / (seconds * sr).max(1.0)).exp();
        mem[1] += alpha * (target - mem[1]);
        mem[4] += 1.0 / sr;
        if mem[4] >= seconds || (mem[1] - target).abs() < 0.001 {
            mem[1] = target;
            mem[4] = 0.0;
            stage = match stage {
                1 => 2,
                2 => {
                    if half {
                        0
                    } else if gate {
                        3
                    } else {
                        4
                    }
                }
                _ => 0,
            };
        }
    } else {
        mem[1] = target;
    }
    mem[2] = f32::from(stage);
    mem[1].clamp(0.0, 1.0)
}

fn lfo(phase: f32, shape: u8, color: f32, held: f32, next: f32) -> f32 {
    match shape {
        0 => {
            let sine = (TAU * phase).sin();
            ((1.0 - color.abs()) * sine
                + color.max(0.0) * (sine * 2.0).tanh()
                + (-color).max(0.0) * (TAU * phase).cos())
            .clamp(-1.0, 1.0)
        }
        1 => {
            let skew = (0.5 + color * 0.45).clamp(0.05, 0.95);
            if phase < skew {
                phase / skew * 2.0 - 1.0
            } else {
                1.0 - (phase - skew) / (1.0 - skew) * 2.0
            }
        }
        2 => {
            if phase < (0.5 + color * 0.45).clamp(0.05, 0.95) {
                1.0
            } else {
                -1.0
            }
        }
        3 => {
            let steps = (2.0 + ((color + 1.0) * 0.5 * 14.0).round()).clamp(2.0, 16.0);
            ((phase * steps).floor() / (steps - 1.0) * 2.0 - 1.0).clamp(-1.0, 1.0)
        }
        _ => {
            let t = if color < 0.0 {
                0.0
            } else if color < 0.5 {
                phase * color * 2.0
            } else {
                let x = phase * phase * (3.0 - 2.0 * phase);
                phase * (2.0 - 2.0 * color) + x * (2.0 * color - 1.0)
            };
            held + (next - held) * t
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
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
    if st.u[0] == 0 {
        mem[10] = ((kx.seed ^ 0x6917) & 0xffff).max(1) as f32;
        st.u[0] = 1;
    }
    for (frame, sample) in out.iter_mut().enumerate() {
        let get = |index: usize| unit(ins[index].at(frame));
        let base_hz = ins[0].at(frame).clamp(0.01, sr * 0.4);
        let attack = 0.001 + get(1).powi(2) * 2.0;
        let decay = 0.001 + get(2).powi(2) * 2.0;
        let sustain = get(3);
        let release = 0.001 + get(4).powi(2) * 2.0;
        let rate = get(5);
        let mut shape = ins[6].at(frame).round().clamp(0.0, 4.0) as u8;
        let mut color = ins[7].at(frame).clamp(-1.0, 1.0);
        let reset_phase = get(8);
        let level = get(9);
        let half = ins[10].at(frame) >= 0.5;
        let gate = ins[11].at(frame) >= 0.5;
        let trigger = ins[12].at(frame) >= 0.5;
        let tap = ins[13].at(frame) >= 0.5;
        let sync = ins[14].at(frame) >= 0.5;
        let function = ins[15].at(frame).round().clamp(0.0, 2.0) as u8;
        let channel = ins[16].at(frame) >= 0.5;
        let trigger_rise = trigger && mem[11] < 0.5;
        let tap_rise = tap && mem[12] < 0.5;
        mem[11] = if trigger { 1.0 } else { 0.0 };
        mem[12] = if tap { 1.0 } else { 0.0 };
        let value = if function == 0 {
            let scale = (220.0 / base_hz).clamp(0.1, 10.0);
            let p = [attack * scale, decay * scale, sustain, release * scale];
            let env = envelope(mem, gate, trigger_rise, half, &p, sr);
            if channel {
                env * 2.0 - 1.0
            } else {
                env
            }
        } else {
            let tapped = function == 2 || sync;
            if half && !tapped {
                let preset = ins[17].at(frame).round().clamp(0.0, 6.0) as u8;
                shape = match preset {
                    0 => 0,
                    1 | 2 => 1,
                    3 => 2,
                    4 => 3,
                    _ => 4,
                };
                color = if preset == 2 || preset == 6 {
                    0.8
                } else if preset == 5 {
                    -1.0
                } else {
                    0.0
                };
            }
            let hz = if tapped && mem[13] > 1.0 {
                sr / mem[13]
            } else {
                base_hz * (0.25 + rate * 3.75)
            };
            if tap_rise {
                if mem[14] > 1.0 {
                    mem[13] = mem[14];
                }
                mem[14] = 0.0;
            }
            mem[14] = (mem[14] + 1.0).min(sr * 60.0);
            if trigger_rise || tap_rise && tapped {
                mem[0] = reset_phase;
            }
            let previous = mem[0];
            mem[0] = (mem[0] + hz.clamp(0.001, sr * 0.4) / sr).fract();
            if mem[0] < previous {
                mem[9] = mem[15];
                mem[15] = random(&mut mem[10]);
            }
            let shifted = (mem[0] + if channel { 0.25 } else { 0.0 }).fract();
            lfo(shifted, shape, color, mem[9], mem[15]) * level
        };
        *sample = value.clamp(-1.0, 1.0);
    }
}
