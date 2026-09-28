//! Original pulse-shaping, pulse-randomizing and bouncing-ball adaptations.
//!
//! Peaks uses generated delay/gravity lookup tables and 32-entry pulse
//! buffers. This kernel uses analytic host-rate timing and one event-local
//! pulse train. A new trigger replaces the current train; it does not overlap.

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 32;

fn unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn random(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32) / (u32::MAX as f32)
}

fn seconds(x: f32, lo: f32, hi: f32, speed: f32) -> f32 {
    (lo + unit(x).powi(2) * (hi - lo)) / speed
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
        st.u[0] = kx.seed ^ 0x5a61_1097;
        if st.u[0] == 0 {
            st.u[0] = 1;
        }
    }
    for (i, sample) in out.iter_mut().enumerate() {
        let speed = (ins[0].at(i) / 220.0).clamp(0.1, 10.0);
        let mode = ins[1].at(i).round().clamp(0.0, 2.0) as u8;
        let half = ins[2].at(i) >= 0.5;
        let gate = ins[3].at(i) >= 0.5;
        let trigger = ins[4].at(i) >= 0.5;
        let edge = (gate && mem[0] < 0.5) || (trigger && mem[11] < 0.5);
        mem[0] = if gate { 1.0 } else { 0.0 };
        mem[11] = if trigger { 1.0 } else { 0.0 };
        let delay = seconds(ins[5].at(i), 0.0, 0.25, speed) * sr;
        let duration = seconds(ins[6].at(i), 0.001, 0.15, speed) * sr;
        let interval = seconds(ins[7].at(i), 0.003, 0.4, speed) * sr;
        let repeats = ins[8].at(i).round().clamp(0.0, 7.0);
        let accept = if half { 1.0 } else { unit(ins[9].at(i)) };
        let repeat_probability = unit(ins[10].at(i));
        let randomness = if half { 0.0 } else { unit(ins[11].at(i)) };
        let gravity = unit(ins[12].at(i));
        let loss = unit(ins[13].at(i));
        let amplitude = if half { 1.0 } else { unit(ins[14].at(i)) };
        let velocity = if half {
            0.0
        } else {
            ins[15].at(i).clamp(-1.0, 1.0)
        };
        let aux = ins[16].at(i) >= 0.5;
        let mut onset = 0.0;
        let main = match mode {
            0 => {
                if edge {
                    mem[1] = if half { 0.0 } else { delay };
                    mem[2] = 0.0;
                    mem[3] = repeats + 1.0;
                }
                if mem[3] > 0.0 {
                    if mem[1] > 0.0 {
                        mem[1] -= 1.0;
                    } else if mem[2] <= 0.0 {
                        let effective_interval = if half {
                            duration + 0.02 * sr / speed
                        } else {
                            interval
                        };
                        mem[2] = duration.min(effective_interval.max(1.0));
                        mem[1] = effective_interval;
                        mem[3] -= 1.0;
                        onset = 1.0;
                    }
                }
                let high = mem[2] > 0.0;
                if high {
                    mem[2] -= 1.0;
                }
                if high {
                    1.0
                } else {
                    0.0
                }
            }
            1 => {
                if edge && random(&mut st.u[0]) <= accept {
                    mem[4] = 1.0;
                    mem[5] = 0.0;
                    mem[6] = 0.0;
                }
                if (0.5..1.5).contains(&mem[4]) && mem[5] <= 0.0 && mem[6] <= 0.0 {
                    let variation = (random(&mut st.u[0]) * 2.0 - 1.0) * randomness;
                    mem[5] = (interval * (1.0 + variation * 0.8)).max(1.0);
                    mem[4] = 2.0;
                }
                if (1.5..2.5).contains(&mem[4]) {
                    if mem[5] > 0.0 {
                        mem[5] -= 1.0;
                    } else {
                        mem[6] = (0.004 * sr / speed).max(1.0);
                        mem[4] = 3.0;
                        onset = 1.0;
                    }
                }
                if mem[4] >= 2.5 && mem[6] <= 0.0 {
                    mem[4] = if random(&mut st.u[0]) < repeat_probability {
                        1.0
                    } else {
                        0.0
                    };
                }
                let high = mem[6] > 0.0;
                if high {
                    mem[6] -= 1.0;
                }
                if high {
                    1.0
                } else {
                    0.0
                }
            }
            _ => {
                if edge {
                    mem[7] = amplitude;
                    mem[8] = velocity * 2.0;
                    mem[9] = 1.0;
                    onset = 1.0;
                }
                if mem[9] > 0.5 {
                    mem[8] -= (4.0 + gravity * 60.0) / sr;
                    mem[7] += mem[8] / sr;
                    if mem[7] <= 0.0 {
                        mem[7] = 0.0;
                        mem[8] = -mem[8] * (0.12 + loss * 0.86);
                        onset = 1.0;
                        if mem[8] < 0.015 {
                            mem[9] = 0.0;
                        }
                    } else if mem[7] >= 1.0 {
                        mem[7] = 1.0;
                        mem[8] = -mem[8] * (0.12 + loss * 0.86);
                        onset = 1.0;
                    }
                }
                mem[7]
            }
        };
        if onset > 0.0 {
            mem[10] = (0.002 * sr / speed).max(1.0);
        }
        let impact = if mem[10] > 0.0 {
            mem[10] -= 1.0;
            1.0
        } else {
            0.0
        };
        *sample = if aux { impact } else { main }.clamp(0.0, 1.0);
    }
}
