//! Original bounded adaptations of the six Rings `Part` model roles.
//!
//! No upstream oscillator, lookup, chord, wave or sample data is used. The
//! event-local four-line network replaces Rings' shared resonator/strum state;
//! External excitation is an event-local, mono host-input adaptation.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const LINES: usize = 4;
const MIN_FREQUENCY: f32 = 20.0;
const STATE_EXTRA: usize = 48 + 512;

/// The per-line capacity at this host rate, including two guard samples.
#[must_use]
pub fn line_len(sr: f32) -> usize {
    (sr.max(1.0) / MIN_FREQUENCY).ceil() as usize + 2
}

/// Four rate-sized comb lines, filter history, and a short diffuser.
#[must_use]
pub fn mem_len(sr: f32) -> usize {
    LINES * line_len(sr) + STATE_EXTRA
}

fn unit(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn semitone(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-48.0, 48.0)
    } else {
        0.0
    }
}

fn random(seed: &mut f32) -> f32 {
    let next = ((*seed as u32).wrapping_mul(251).wrapping_add(37)) & 0xffff;
    *seed = next as f32;
    next as f32 / 32_767.5 - 1.0
}

fn chord_ratio(chord: usize, index: usize) -> f32 {
    // Authored arithmetic interval map, not the source chord table.
    let degree = ((chord * 3 + index * (3 + chord % 3)) % 13) as f32;
    2.0_f32.powf(degree / 12.0)
}

fn modal(
    mem: &mut [f32],
    excitation: f32,
    frequency: f32,
    structure: f32,
    damping: f32,
    sr: f32,
    base: usize,
) -> (f32, f32) {
    let mut main = 0.0;
    let mut aux = 0.0;
    for mode in 0..4 {
        let ratio = 1.0 + mode as f32 * (1.22 + structure * 0.65);
        let hz = (frequency * ratio).min(sr * 0.43);
        let radius = (-(3.0 + damping * 35.0 + mode as f32 * 2.0) / sr).exp();
        let coef = 2.0 * radius * (TAU * hz / sr).cos();
        let off = base + mode * 2;
        let y = (excitation * (0.085 / (1.0 + mode as f32)) + coef * mem[off]
            - radius * radius * mem[off + 1])
            .clamp(-3.0, 3.0);
        mem[off + 1] = mem[off];
        mem[off] = y;
        main += y;
        aux += y * if mode % 2 == 0 { 1.0 } else { -1.0 };
    }
    (main * 0.27, aux * 0.27)
}

#[allow(clippy::too_many_arguments)]
fn strings(
    mem: &mut [f32],
    excitation: f32,
    frequency: f32,
    structure: f32,
    damping: f32,
    position: f32,
    chord: usize,
    poly: usize,
    sympathetic: bool,
    quantized: bool,
    sr: f32,
    line_len: usize,
    base: usize,
) -> (f32, f32) {
    let mut main = 0.0;
    let mut aux = 0.0;
    let lines = if sympathetic {
        poly.min(LINES)
    } else if poly == 1 {
        LINES
    } else {
        poly.min(LINES)
    };
    for line in 0..lines {
        let degree = if quantized {
            chord_ratio(chord, line)
        } else if sympathetic {
            chord_ratio(chord, line) * (1.0 + structure * line as f32 * 0.004)
        } else {
            1.0 + line as f32 * (0.02 + structure * 0.08)
        };
        let target = (frequency * degree).clamp(sr / (line_len as f32 - 2.0), sr * 0.4);
        let delay = (sr / target).round().clamp(2.0, (line_len - 2) as f32) as usize;
        let write = mem[base + 16 + line] as usize % line_len;
        let read = (write + line_len - delay) % line_len;
        let off = line * line_len;
        let sample = mem[off + read];
        let smooth_idx = base + 24 + line;
        let smooth = mem[smooth_idx] + (0.08 + structure * 0.45) * (sample - mem[smooth_idx]);
        mem[smooth_idx] = smooth;
        let loss = (1.0 - (2.0 + damping * 28.0) / sr).clamp(0.0, 0.999_99);
        mem[off + write] =
            (smooth * loss + excitation * (0.22 + position * 0.28) / lines as f32).clamp(-2.0, 2.0);
        mem[base + 16 + line] = ((write + 1) % line_len) as f32;
        let pickup = sample - smooth * (0.15 + position * 0.7);
        if poly == 1 {
            main += sample;
            aux += pickup * if line % 2 == 0 { 1.0 } else { -1.0 };
        } else if line % 2 == 0 {
            main += sample;
        } else {
            aux += pickup;
        }
    }
    let gain = 0.6 / (lines as f32).sqrt();
    (main * gain, aux * gain)
}

/// Render main or auxiliary with fixed event-local state. `mode` is port 15.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines
)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let sr = kx.sr.max(1.0);
    let line_len = line_len(sr);
    let base = LINES * line_len;
    if mem.len() < mem_len(sr) {
        out.fill(0.0);
        return;
    }
    let auxiliary = ins[15].first() >= 0.5;
    if st.u[0] == 0 {
        mem[base + 8] = ((kx.seed ^ 0x4b31) & 0xffff).max(1) as f32;
        st.u[0] = 1;
    }
    for (frame, output) in out.iter_mut().enumerate() {
        let model = ins[1].at(frame).round().clamp(0.0, 5.0) as usize;
        let structure = unit(ins[2].at(frame), 0.5);
        let brightness = unit(ins[3].at(frame), 0.5);
        let damping = unit(ins[4].at(frame), 0.5);
        let position = unit(ins[5].at(frame), 0.5);
        let strum = unit(ins[6].at(frame), 1.0);
        let internal_exciter = ins[7].at(frame) >= 0.5;
        let internal_strum = ins[8].at(frame) >= 0.5;
        let internal_note = ins[9].at(frame) >= 0.5;
        let tonic = semitone(ins[10].at(frame));
        let note = semitone(ins[11].at(frame));
        let fm = semitone(ins[12].at(frame));
        let chord = ins[13].at(frame).round().clamp(0.0, 10.0) as usize;
        let poly = ins[14].at(frame).round().clamp(1.0, 4.0) as usize;
        let event_freq = ins[0].at(frame).clamp(20.0, sr * 0.3);
        let semis = tonic + if internal_note { note } else { 0.0 } + fm;
        let frequency = (event_freq
            * 2.0_f32.powf(semis / 12.0)
            * if model == 0 || model == 3 {
                chord_ratio(chord, poly - 1).sqrt()
            } else {
                1.0
            })
        .clamp(20.0, sr * 0.35);
        let age = mem[base + 9];
        let noise = random(&mut mem[base + 8]);
        let struck = if internal_strum {
            age < (0.001 + strum * 0.006)
        } else {
            strum >= 0.5 && age < 0.003
        };
        let envelope = if struck {
            1.0
        } else {
            (-age * (3.0 + damping * 12.0)).exp() * 0.05
        };
        let internal = if internal_exciter {
            (noise * (0.15 + brightness * 0.35)
                + (TAU * frequency * age).sin() * (0.1 + position * 0.15))
                * envelope
        } else {
            0.0
        };
        let external = ins[16].at(frame);
        let excitation = internal
            + if external.is_finite() {
                external.clamp(-2.0, 2.0) * 0.35
            } else {
                0.0
            };
        let (main, aux) = match model {
            0 => modal(mem, excitation, frequency, structure, damping, sr, base),
            3 => {
                mem[base + 10] =
                    (mem[base + 10] + frequency * (0.5 + structure * 3.0) / sr).fract();
                mem[base + 11] = (mem[base + 11] + frequency / sr).fract();
                let modulation = ((TAU * mem[base + 10]).sin() + excitation)
                    * (0.2 + brightness * 3.0)
                    * (0.5 + position)
                    * envelope;
                let sustain = (-age * (1.0 + damping * 9.0)).exp();
                (
                    (TAU * mem[base + 11] + modulation).sin() * sustain * 0.45,
                    (TAU * mem[base + 10]).sin() * sustain * 0.3,
                )
            }
            _ => {
                let (mut main, mut aux) = strings(
                    mem,
                    excitation,
                    frequency,
                    structure,
                    damping,
                    position,
                    chord,
                    poly,
                    model == 1 || model == 4,
                    model == 4,
                    sr,
                    line_len,
                    base,
                );
                if model == 1 {
                    main *= 0.65;
                    aux *= 1.15;
                }
                if model == 5 {
                    let write = mem[base + 40] as usize % 512;
                    let echo = mem[base + 48 + write];
                    mem[base + 48 + write] = (aux * 0.45 + echo * 0.68).clamp(-2.0, 2.0);
                    mem[base + 40] = ((write + 1) % 512) as f32;
                    main = (main + echo * 0.24).tanh();
                    aux = (aux + echo * 0.42).tanh();
                }
                (main, aux)
            }
        };
        *output = (if auxiliary { aux } else { main }).tanh();
        mem[base + 9] = age + 1.0 / sr;
    }
}
