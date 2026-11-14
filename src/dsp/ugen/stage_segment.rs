//! Original host-rate Stages single-segment and oscillator adaptations.
//! Source wave/frequency/portamento tables and fixed 31.25 kHz processing
//! are replaced by analytic curves and event-local fixed state.

use super::{Inp, Kx, NodeState, MAX_PORTS};

pub const STATE_FLOATS: usize = 16;

/// A rate-scaled capture window; longer CV delays use bounded decimated writes.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn mem_len(sr: f32) -> usize {
    STATE_FLOATS + (sr.max(1.0) * 576.0 / 31_250.0).ceil() as usize
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn delay(mem: &mut [f32], primary: f32, secondary: f32, sr: f32) -> (f32, f32) {
    let (state, ring) = mem.split_at_mut(STATE_FLOATS);
    if ring.is_empty() {
        return (0.0, -1.0);
    }
    // Source secondary spans a pitch-like 0.0625..4 second CV delay.
    let requested = (0.5 * 2.0_f32.powf((secondary - 0.5) * 6.0) * sr).max(1.0);
    let clock = (ring.len() as f32 / requested).min(1.0);
    state[12] += clock;
    state[13] += (primary - state[13]) * clock;
    if state[12] >= 1.0 {
        state[12] -= 1.0;
        let write = state[11] as usize;
        ring[write] = state[13];
        state[11] = ((write + 1) % ring.len()) as f32;
    }
    state[15] = (state[15] + 1.0 / requested).fract();
    let read = state[11] as usize;
    state[14] += (ring[read] - state[14]) * clock;
    (state[14], state[15] * 2.0 - 1.0)
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn shape(phase: f32, macro_control: f32) -> f32 {
    let phase = phase.rem_euclid(1.0);
    let triangle = 1.0 - 4.0 * (phase - 0.5).abs();
    let saw = phase * 2.0 - 1.0;
    let pulse = if phase < 0.5 { -1.0 } else { 1.0 };
    let position = macro_control.clamp(0.0, 1.0) * 2.0;
    if position < 1.0 {
        triangle + (saw - triangle) * position
    } else {
        saw + (pulse - saw) * (position - 1.0)
    }
}

fn curve(phase: f32, shape: f32) -> f32 {
    let exponent = 2.0_f32.powf((shape - 0.5) * 5.0);
    phase.clamp(0.0, 1.0).powf(exponent)
}

fn next_random(seed: &mut f32) -> f32 {
    let value = ((*seed as u32).wrapping_mul(25_173).wrapping_add(13_849)) & 0xffff;
    *seed = value as f32;
    value as f32 / 65_535.0
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < mem_len(kx.sr) {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    if st.u[0] == 0 {
        mem[10] = ((kx.seed ^ 0x51a6) & 0xffff).max(1) as f32;
        st.u[0] = 1;
    }
    for (frame, sample) in out.iter_mut().enumerate() {
        let freq = ins[0].at(frame).clamp(0.01, sr * 0.4);
        let typ = ins[1].at(frame).round().clamp(0.0, 3.0) as u8;
        let primary = unit(ins[2].at(frame));
        let secondary = unit(ins[3].at(frame));
        let looping = ins[4].at(frame) >= 0.5;
        let gate = ins[5].at(frame) >= 0.5;
        let trigger = ins[6].at(frame) >= 0.5;
        let clock = ins[7].at(frame) >= 0.5;
        let function = ins[8].at(frame).round().clamp(0.0, 9.0) as u8;
        let channel = ins[9].at(frame) >= 0.5;
        let gate_rise = gate && mem[2] < 0.5;
        let trigger_rise = trigger && mem[3] < 0.5;
        let clock_rise = clock && mem[4] < 0.5;
        mem[2] = if gate { 1.0 } else { 0.0 };
        mem[3] = if trigger { 1.0 } else { 0.0 };
        mem[4] = if clock { 1.0 } else { 0.0 };
        mem[8] = (mem[8] + 1.0).min(sr * 60.0);
        if clock_rise {
            mem[9] = mem[8];
            mem[8] = 0.0;
        }
        let onset = trigger_rise || gate_rise;
        if onset {
            mem[0] = 0.0;
            mem[5] = 1.0;
            mem[6] = if gate { primary } else { 0.0 };
        }
        let clock_hz = if mem[9] > 1.0 { sr / mem[9] } else { freq };
        let (value, phase) = match function {
            9 => delay(mem, primary, secondary, sr),
            1 => {
                // Triggered decay, with primary duration and secondary curve.
                if mem[5] > 0.5 {
                    mem[0] += 1.0 / ((0.002 + primary * primary * 3.0) * sr);
                    if mem[0] >= 1.0 {
                        mem[0] = 1.0;
                        mem[5] = 0.0;
                    }
                }
                (1.0 - curve(mem[0], secondary), mem[0] * 2.0 - 1.0)
            }
            2 => {
                // Timed pulse, primary height, secondary duration.
                if mem[5] > 0.5 {
                    mem[0] += 1.0 / ((0.002 + secondary * secondary * 2.0) * sr);
                    if mem[0] >= 1.0 {
                        mem[0] = 1.0;
                        mem[5] = 0.0;
                    }
                }
                (if mem[5] > 0.5 { primary } else { 0.0 }, mem[0] * 2.0 - 1.0)
            }
            3 => {
                // Gate acceptance uses authored deterministic per-event RNG.
                if onset {
                    mem[6] = if next_random(&mut mem[10]) <= secondary {
                        primary
                    } else {
                        0.0
                    };
                }
                (
                    if gate { mem[6] } else { 0.0 },
                    if gate { 1.0 } else { -1.0 },
                )
            }
            4 => {
                if onset {
                    mem[6] = primary;
                }
                let alpha = 1.0 - (-1.0 / ((0.001 + secondary * 0.5) * sr)).exp();
                mem[1] += (mem[6] - mem[1]) * alpha;
                (mem[1], if gate { 1.0 } else { -1.0 })
            }
            5..=8 => {
                // Free/tap LFO and free/PLL audio oscillators.
                let audio_rate = function >= 7;
                let sync = function == 6 || function == 8;
                let base = if audio_rate {
                    freq * 2.0_f32.powf((primary - 0.5) * 2.0)
                } else {
                    (0.05 + primary * primary * 24.0).min(24.05)
                };
                let hz = if sync { clock_hz } else { base }.clamp(0.01, sr * 0.4);
                if sync && clock_rise {
                    mem[0] = 0.0;
                }
                mem[0] = (mem[0] + hz / sr).fract();
                let wave = shape(mem[0], secondary);
                ((wave + 1.0) * 0.3125, wave)
            }
            _ => match typ {
                0 => {
                    if mem[5] > 0.5 || looping {
                        mem[0] += 1.0 / ((0.002 + primary * primary * 2.0) * sr);
                        if mem[0] >= 1.0 {
                            mem[0] = if looping { mem[0].fract() } else { 1.0 };
                            mem[5] = if looping { 1.0 } else { 0.0 };
                        }
                    }
                    (curve(mem[0], secondary), mem[0] * 2.0 - 1.0)
                }
                1 => {
                    let alpha = 1.0 - (-1.0 / ((0.001 + secondary * 0.5) * sr)).exp();
                    mem[1] += ((if gate { primary } else { 0.0 }) - mem[1]) * alpha;
                    (mem[1], if gate { 1.0 } else { -1.0 })
                }
                2 => {
                    if mem[5] > 0.5 {
                        mem[0] += 1.0 / ((0.002 + secondary * secondary * 2.0) * sr);
                        if mem[0] >= 1.0 {
                            mem[0] = 1.0;
                            mem[5] = 0.0;
                        }
                    }
                    (
                        if gate && (looping || mem[5] > 0.5) {
                            primary
                        } else {
                            0.0
                        },
                        mem[0] * 2.0 - 1.0,
                    )
                }
                _ => {
                    let hz = if looping { freq } else { freq * 0.01 };
                    mem[0] = (mem[0] + hz / sr).fract();
                    let wave = shape(mem[0], secondary);
                    ((wave + 1.0) * 0.3125, wave)
                }
            },
        };
        *sample = if channel { phase } else { value };
    }
}
