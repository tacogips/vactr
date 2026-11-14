//! Four-voice divide-down string-machine adaptation with stereo ensemble.
//!
//! The chord intervals come from the MIT Plaits chord bank. Oscillators,
//! filters and analytic-sine delay modulation are independently written;
//! no generated waveform, lookup table or sample is imported.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::{PI, TAU};

const CHORDS: [[f32; 4]; 11] = [
    [0.0, 0.01, 11.99, 12.0],
    [0.0, 7.0, 7.01, 12.0],
    [0.0, 5.0, 7.0, 12.0],
    [0.0, 3.0, 7.0, 12.0],
    [0.0, 3.0, 7.0, 10.0],
    [0.0, 3.0, 10.0, 14.0],
    [0.0, 3.0, 10.0, 17.0],
    [0.0, 2.0, 9.0, 16.0],
    [0.0, 4.0, 11.0, 14.0],
    [0.0, 4.0, 7.0, 11.0],
    [0.0, 4.0, 7.0, 12.0],
];

const DELAY: usize = 1024;
/// Four phases, stereo filter/LFO/index, and two bounded ensemble lines.
pub const STATE_FLOATS: usize = 16 + 2 * DELAY;

#[inline]
fn unit(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn tap(mem: &[f32], start: usize, write: usize, offset: f32) -> f32 {
    let position = (write as f32 - offset).rem_euclid(DELAY as f32);
    let index = position.floor() as usize;
    let fraction = position.fract();
    let a = mem[start + index];
    let b = mem[start + (index + 1) % DELAY];
    a + fraction * (b - a)
}

#[inline]
fn voice(phase: f32, registration: f32) -> f32 {
    let fundamental = 2.0 * phase - 1.0;
    let second = 2.0 * (2.0 * phase).fract() - 1.0;
    let fourth = 2.0 * (4.0 * phase).fract() - 1.0;
    let eighth = 2.0 * (8.0 * phase).fract() - 1.0;
    let square = if phase < 0.5 { 1.0 } else { -1.0 };
    let bright = if (2.0 * phase).fract() < 0.5 {
        1.0
    } else {
        -1.0
    };
    let dark = 0.65 * fundamental + 0.25 * second + 0.10 * fourth;
    let rich = 0.42 * square + 0.26 * bright + 0.20 * fourth + 0.12 * eighth;
    dark + registration * (rich - dark)
}

/// Mode 0 renders ensemble left, mode 1 ensemble right.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    _st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render_worker(ins, mem, out, None, kx);
}

/// Renders both ensemble sides from one shared string-machine state.
pub fn render_pair(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    main: &mut [f32],
    aux: &mut [f32],
    kx: &Kx<'_>,
) {
    let _ = st;
    render_worker(ins, mem, main, Some(aux), kx);
}

fn render_worker(
    ins: &[Inp<'_>; MAX_PORTS],
    mem: &mut [f32],
    out: &mut [f32],
    mut aux_out: Option<&mut [f32]>,
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        if let Some(aux) = aux_out {
            aux.fill(0.0);
        }
        return;
    }
    let sr = kx.sr.max(1.0);
    let freq = ins[0].first().clamp(20.0, sr * 0.12);
    let chord = CHORDS[(unit(ins[1].first()) * 10.999) as usize];
    let timbre = unit(ins[2].first());
    let registration = unit(ins[3].first());
    let right = ins[4].first() >= 0.5;
    let cutoff = (2.2 * freq * 2.0_f32.powf(10.0 * timbre)).clamp(30.0, sr * 0.24);
    let alpha_l = 1.0 - (-2.0 * PI * cutoff / sr).exp();
    let alpha_r = 1.0 - (-2.0 * PI * (1.5 * cutoff).min(sr * 0.24) / sr).exp();
    let ensemble = (2.0 * timbre - 1.0).abs();
    let depth = 0.35 + 0.65 * timbre;
    let dry = 1.0 - 0.5 * ensemble;
    for (i, sample) in out.iter_mut().enumerate() {
        let mut left = 0.0;
        let mut right_signal = 0.0;
        for (voice_index, interval) in chord.iter().enumerate() {
            let voice_hz = freq * 2.0_f32.powf(*interval / 12.0);
            let step = (voice_hz / sr).min(0.24);
            mem[voice_index] = (mem[voice_index] + step).fract();
            let tone = voice(mem[voice_index], registration) * 0.12;
            if voice_index & 1 == 0 {
                left += tone;
            } else {
                right_signal += tone;
            }
        }
        mem[4] += alpha_l * (left - mem[4]);
        mem[5] += alpha_r * (right_signal - mem[5]);
        left = 0.66 * mem[4] + 0.33 * mem[5];
        right_signal = 0.66 * mem[5] + 0.33 * mem[4];
        let write = mem[8] as usize % DELAY;
        mem[16 + write] = left;
        mem[16 + DELAY + write] = right_signal;
        mem[8] = ((write + 1) % DELAY) as f32;
        mem[6] = (mem[6] + 0.75 / sr).fract();
        mem[7] = (mem[7] + 6.57 / sr).fract();
        let slow = (TAU * mem[6]).sin();
        let fast = (TAU * mem[7]).sin();
        let base = 0.004 * sr;
        let excursion = depth * sr * (0.0033 * slow + 0.00033 * fast);
        let lag_l = (base + excursion).clamp(1.0, (DELAY - 2) as f32);
        let lag_r = (base - excursion).clamp(1.0, (DELAY - 2) as f32);
        let wet_l = tap(mem, 16, write, lag_l);
        let wet_r = tap(mem, 16 + DELAY, write, lag_r);
        let left_value = dry * left + ensemble * (0.75 * wet_l + 0.25 * wet_r);
        let right_value = dry * right_signal + ensemble * (0.75 * wet_r + 0.25 * wet_l);
        *sample = if right { right_value } else { left_value };
        if let Some(aux) = aux_out.as_deref_mut() {
            aux[i] = if right { left_value } else { right_value };
        }
    }
}
