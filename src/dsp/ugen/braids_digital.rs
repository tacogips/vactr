//! Original analytic adaptations of Braids positions 13–16.
//!
//! No upstream waveform, FIR, pitch or filter lookup data is used. The
//! event-local sync clock and strike envelope replace external sync pulses.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

const SCALARS: usize = 18;
const COMB_SECONDS: f32 = 0.04;

/// Preallocated event state, including a 40 ms comb ring at the host rate.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn mem_len(sr: f32) -> usize {
    SCALARS + (sr * COMB_SECONDS).ceil() as usize + 2
}

fn saw(phase: f32) -> f32 {
    2.0 * phase - 1.0
}

fn advance(phase: &mut f32, step: f32) {
    *phase = (*phase + step).fract();
}

/// Render one of four source-ordered shapes into a preallocated block.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.22);
    let shape = ins[1].first().round().clamp(13.0, 16.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let root_step = freq / sr;
    let sync_step = root_step * (1.0 + 3.0 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    let (state, delay) = mem.split_at_mut(SCALARS);
    let delay_len = delay.len();
    for sample in out.iter_mut() {
        if sync > 0.0 {
            advance(&mut state[14], sync_step);
            if state[14] < sync_step {
                state[..7].fill(0.0);
                state[9] = 0.0;
            }
        }
        let body = match shape {
            13 => {
                advance(&mut state[0], root_step);
                advance(&mut state[1], (root_step * (0.5 + 3.0 * color)).min(0.45));
                advance(&mut state[2], (root_step * (0.5 + 3.0 * timbre)).min(0.45));
                let carrier = (TAU * state[0]).sin();
                let a = (TAU * state[1]).sin();
                let b = (TAU * state[2]).sin();
                (carrier * a * b * 2.5).tanh()
            }
            14 => {
                let mut sum = 0.0;
                for (index, phase) in state[..7].iter_mut().enumerate() {
                    let detune = (index as f32 - 3.0) * color * 0.045;
                    advance(phase, root_step * (1.0 + detune));
                    sum += saw(*phase);
                }
                let raw = (sum * 0.19).tanh();
                let cutoff = 40.0 + timbre * (sr * 0.16 - 40.0);
                let coeff = (TAU * cutoff / sr).min(0.95);
                state[10] += coeff * (raw - state[10]);
                raw - state[10]
            }
            15 => {
                advance(&mut state[0], root_step);
                let input = saw(state[0]);
                let seconds = 0.001 + 0.039 * (1.0 - color);
                let taps = (seconds * sr).round() as usize;
                let taps = taps.clamp(1, delay_len - 1);
                let head = (state[11] as usize) % delay_len;
                let read = (head + delay_len - taps) % delay_len;
                let delayed = delay[read];
                let feedback = (timbre * 1.7 - 0.85).clamp(-0.85, 0.85);
                delay[head] = (input * 0.5 + delayed * feedback).clamp(-1.5, 1.5);
                state[11] = ((head + 1) % delay_len) as f32;
                (input * 0.55 + delayed * 0.45).clamp(-1.0, 1.0)
            }
            _ => {
                advance(&mut state[0], root_step);
                let hold = (1.0 + (1.0 - color) * 31.0).round();
                if state[12] <= 0.0 {
                    let steps = 3.0 + (timbre * 29.0).round();
                    let staircase = ((state[0] * steps).floor() / steps) * 2.0 - 1.0;
                    state[13] = (staircase * (1.0 + timbre * 2.0)).clamp(-1.0, 1.0);
                    state[12] = hold;
                }
                state[12] -= 1.0;
                state[13]
            }
        };
        let transient = if state[15] == 0.0 { strike } else { state[15] };
        state[15] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (1.0 - 0.2 * transient) + transient * 0.2 * (TAU * state[0] * 5.0).sin())
            .clamp(-1.0, 1.0);
    }
}
