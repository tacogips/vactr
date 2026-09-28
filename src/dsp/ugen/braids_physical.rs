//! Original bounded string/wind adaptations of Braids positions 28–31.
//!
//! A preallocated event-local resonator replaces the source multi-voice
//! pluck and separate bow/jet/bore waveguides. No source envelope, friction,
//! jet or body-filter lookup data is imported.

use super::{Inp, Kx, NodeState, MAX_PORTS};

const SCALARS: usize = 10;
const MIN_FREQ: f32 = 20.0;

/// One full low-note period plus interpolation margin at the host rate.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn mem_len(sr: f32) -> usize {
    SCALARS + (sr / MIN_FREQ).ceil() as usize + 4
}

fn noise(state: &mut f32) -> f32 {
    let mut bits = if *state == 0.0 {
        0xACE1u16
    } else {
        *state as u16
    };
    bits ^= bits << 7;
    bits ^= bits >> 9;
    bits ^= bits << 8;
    *state = f32::from(bits);
    f32::from(bits) / 32767.5 - 1.0
}

fn advance(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

/// Render one of four event-local resonating string/wind voices.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let sr = kx.sr.max(1.0);
    if mem.len() < mem_len(sr) {
        out.fill(0.0);
        return;
    }
    let freq = ins[0].first().clamp(MIN_FREQ, sr * 0.22);
    let shape = ins[1].first().round().clamp(28.0, 31.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let (state, ring) = mem.split_at_mut(SCALARS);
    let len = ring.len();
    let period = (sr / freq).round().clamp(2.0, (len - 2) as f32) as usize;
    let sync_step = freq / sr * (0.2 + 0.7 * sync);
    let onset_decay = (-1.0 / (0.014 * sr)).exp();
    let damp = 0.90 + 0.095 * color;
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut state[4], sync_step) {
            state[3] = 1.0;
            state[8] = 0.0;
        }
        if state[9] == 0.0 {
            state[9] = 1.0;
            state[3] = 1.0;
        }
        let head = (state[0] as usize) % len;
        let read = (head + len - period) % len;
        let reflected = ring[read];
        let random = noise(&mut state[7]);
        let excitation = match shape {
            28 => {
                let pick = (1.0 - (2.0 * timbre - 1.0).abs()) * 0.8 + 0.2;
                random * state[3] * (0.18 + 0.42 * strike) * pick
            }
            29 => {
                let bow_speed = 0.12 + timbre * 0.75;
                let friction = 1.0 + color * 8.0;
                ((bow_speed - reflected) * friction).tanh() * (0.03 + 0.07 * strike)
            }
            30 => {
                let breath = 0.06 + 0.15 * (1.0 - color);
                let pressure = breath * (1.0 + random * (0.1 + timbre * 0.6));
                (pressure - reflected * (0.9 + 0.8 * timbre)).tanh() * 0.45
            }
            _ => {
                let breath = (0.09 + 0.12 * color) * (1.0 + random * 0.12);
                let edge = ((breath - state[8] * (0.5 + timbre)) * 4.0).tanh();
                state[8] += (0.02 + timbre * 0.2) * (reflected - state[8]);
                edge * 0.09
            }
        };
        // A generated sync edge re-excites every model, including sustained
        // bow and breath paths, without clearing the resonator in callback.
        let excitation = excitation + random * state[3] * (0.02 + strike * 0.08);
        state[3] *= onset_decay;
        let lowpass = 0.1 + 0.75 * timbre;
        state[1] += lowpass * (reflected - state[1]);
        let feedback = match shape {
            28 => state[1] * damp,
            29 => state[1] * (0.86 + 0.1 * timbre),
            30 => -state[1] * (0.76 + 0.17 * color),
            _ => -state[1] * (0.80 + 0.14 * timbre),
        };
        let new_sample = (feedback + excitation).clamp(-1.0, 1.0);
        ring[head] = new_sample;
        state[0] = ((head + 1) % len) as f32;
        let body = if shape == 31 {
            let hp = new_sample - state[2] + 0.995 * state[5];
            state[2] = new_sample;
            state[5] = hp;
            hp
        } else {
            new_sample
        };
        *sample = body.clamp(-1.0, 1.0);
    }
}
