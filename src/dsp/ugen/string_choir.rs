//! Original event-local adaptation of Rings' separate StringSynthPart path.
//!
//! Four analytic chord voices and rate-sized comb lines replace the source's
//! 12 divide-down oscillators. Six FX roles are distinct bounded formulas;
//! no registration/chord tables, generated sine data or source FX code is used.

use std::f32::consts::TAU;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const VOICES: usize = 4;
const MIN_FREQUENCY: f32 = 20.0;
const EXTRA: usize = 48;

/// Capacity for each comb voice; enough to hold a 20 Hz period.
#[must_use]
pub fn line_len(sr: f32) -> usize {
    (sr.max(1.0) / MIN_FREQUENCY).ceil() as usize + 2
}

fn fx_len(sr: f32) -> usize {
    (sr.max(1.0) * 0.03).ceil() as usize + 2
}

/// Fixed per-output memory: four comb lines, a 30 ms FX delay, and state.
#[must_use]
pub fn mem_len(sr: f32) -> usize {
    VOICES * line_len(sr) + fx_len(sr) + EXTRA
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

fn authored_interval(chord: usize, voice: usize, structure: f32) -> f32 {
    let degree = ((chord * 5 + voice * (2 + chord % 4)) % 12) as f32;
    degree + voice as f32 * structure * 0.08
}

fn random(seed: &mut f32) -> f32 {
    let next = ((*seed as u32).wrapping_mul(109).wrapping_add(117)) & 0xffff;
    *seed = next as f32;
    next as f32 / 32_767.5 - 1.0
}

fn tap(mem: &[f32], base: usize, length: usize, write: usize, delay: usize) -> f32 {
    mem[base + (write + length - delay.min(length - 1)) % length]
}

fn formant(input: f32, vowel: f32, variant: bool, sr: f32, mem: &mut [f32], base: usize) -> f32 {
    let centers = if variant {
        [420.0 + vowel * 500.0, 1_300.0 + vowel * 950.0]
    } else {
        [300.0 + vowel * 850.0, 900.0 + (1.0 - vowel) * 1_150.0]
    };
    let mut result = 0.0;
    for (mode, hz) in centers.into_iter().enumerate() {
        let offset = base + 20 + mode * 2;
        let radius = if variant { 0.994 } else { 0.987 };
        let coefficient = 2.0 * radius * (TAU * hz.min(sr * 0.42) / sr).cos();
        let next = (input * 0.07 + coefficient * mem[offset] - radius * radius * mem[offset + 1])
            .clamp(-3.0, 3.0);
        mem[offset + 1] = mem[offset];
        mem[offset] = next;
        result += next;
    }
    result * if variant { 0.32 } else { 0.25 }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::too_many_lines
)]
/// Render one independently selected main/aux channel.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let sr = kx.sr.max(1.0);
    let line_len = line_len(sr);
    let delay_base = VOICES * line_len;
    let effect_len = fx_len(sr);
    let state_base = delay_base + effect_len;
    if mem.len() < mem_len(sr) {
        out.fill(0.0);
        return;
    }
    if st.u[0] == 0 {
        mem[state_base + 4] = ((kx.seed ^ 0x6171) & 0xffff).max(1) as f32;
        st.u[0] = 1;
    }
    let auxiliary = ins[15].first() >= 0.5;
    for (frame, output) in out.iter_mut().enumerate() {
        let structure = unit(ins[1].at(frame), 0.5);
        let brightness = unit(ins[2].at(frame), 0.5);
        let damping = unit(ins[3].at(frame), 0.5);
        let position = unit(ins[4].at(frame), 0.5);
        let strum = unit(ins[5].at(frame), 1.0);
        let exciter = ins[6].at(frame) >= 0.5;
        let internal_strum = ins[7].at(frame) >= 0.5;
        let internal_note = ins[8].at(frame) >= 0.5;
        let tonic = semitone(ins[9].at(frame));
        let note = semitone(ins[10].at(frame));
        let fm = semitone(ins[11].at(frame));
        let chord = ins[12].at(frame).round().clamp(0.0, 10.0) as usize;
        let poly = ins[13].at(frame).round().clamp(1.0, 4.0) as usize;
        let fx = ins[14].at(frame).round().clamp(0.0, 5.0) as usize;
        let base_hz = ins[0].at(frame).clamp(MIN_FREQUENCY, sr * 0.3);
        let pitch = (base_hz
            * 2.0_f32.powf((tonic + fm + if internal_note { note } else { 0.0 }) / 12.0))
        .clamp(MIN_FREQUENCY, sr * 0.3);
        let age = mem[state_base + 5];
        let attack = if internal_strum {
            0.001 + (1.0 - strum) * 0.04
        } else {
            0.02 + (1.0 - strum) * 0.12
        };
        let envelope = (age / attack).min(1.0) * (-age / (0.1 + damping * 4.0)).exp() * strum;
        let noise = random(&mut mem[state_base + 4]);
        let mut main = 0.0;
        let mut aux = 0.0;
        for voice in 0..poly {
            let interval = authored_interval(chord, voice, structure);
            let frequency = (pitch * 2.0_f32.powf(interval / 12.0)).clamp(MIN_FREQUENCY, sr * 0.4);
            let phase_idx = state_base + voice;
            mem[phase_idx] = (mem[phase_idx] + frequency / sr).fract();
            let phase = mem[phase_idx];
            let square = if phase < 0.5 { 1.0 } else { -1.0 };
            let saw = 2.0 * phase - 1.0;
            let octave = (TAU * phase * 2.0).sin();
            let registration = (square * (1.0 - brightness)
                + saw * brightness * 0.7
                + octave * brightness * structure * 0.35)
                * envelope;
            let write_idx = state_base + 8 + voice;
            let write = mem[write_idx] as usize % line_len;
            let period = (sr / frequency).round().clamp(2.0, (line_len - 2) as f32) as usize;
            let previous = tap(mem, voice * line_len, line_len, write, period);
            let pluck = if exciter && age < 0.004 {
                noise * (0.12 + brightness * 0.25)
            } else {
                0.0
            };
            let smooth_idx = state_base + 12 + voice;
            let smooth = mem[smooth_idx] + (0.15 + structure * 0.3) * (previous - mem[smooth_idx]);
            mem[smooth_idx] = smooth;
            mem[voice * line_len + write] =
                (smooth * (1.0 - (2.0 + 14.0 * damping) / sr) + pluck).clamp(-2.0, 2.0);
            mem[write_idx] = ((write + 1) % line_len) as f32;
            let voice_main = (registration + previous * 0.3) * 0.23;
            let voice_aux = (registration * (0.5 - brightness) + (previous - smooth) * 0.55) * 0.23;
            if voice % 2 == 0 {
                main += voice_main;
                aux += voice_aux * 0.4;
            } else {
                aux += voice_aux;
                main += voice_main * 0.4;
            }
        }
        let input = if auxiliary { aux } else { main };
        let write = mem[state_base + 16] as usize % effect_len;
        let lfo = (TAU * age * (0.25 + structure * 0.3)).sin();
        let wet = match fx {
            0 | 3 => formant(input, position, fx == 3, sr, mem, state_base),
            1 => {
                let delay = ((0.009 + position * 0.005 + lfo * position * 0.003) * sr) as usize;
                tap(mem, delay_base, effect_len, write, delay) * 0.7
            }
            4 => {
                let d1 = ((0.006 + position * 0.004 + lfo * 0.002) * sr) as usize;
                let d2 = ((0.019 + position * 0.005 - lfo * 0.003) * sr) as usize;
                (tap(mem, delay_base, effect_len, write, d1)
                    + tap(mem, delay_base, effect_len, write, d2))
                    * 0.43
            }
            2 | 5 => {
                let delay = if fx == 2 {
                    (sr * 0.021) as usize
                } else {
                    (sr * 0.028) as usize
                };
                let echo = tap(mem, delay_base, effect_len, write, delay);
                mem[state_base + 17] +=
                    (if fx == 2 { 0.16 } else { 0.06 }) * (echo - mem[state_base + 17]);
                mem[state_base + 17] * if fx == 2 { 0.8 } else { 1.1 }
            }
            _ => 0.0,
        };
        let feedback = if fx == 2 || fx == 5 {
            wet * (0.25 + position * 0.55)
        } else {
            0.0
        };
        mem[delay_base + write] = (input + feedback).clamp(-2.0, 2.0);
        mem[state_base + 16] = ((write + 1) % effect_len) as f32;
        *output = (input * (1.0 - position * 0.45) + wet * (0.2 + position * 0.7)).tanh();
        mem[state_base + 5] = age + 1.0 / sr;
    }
}
