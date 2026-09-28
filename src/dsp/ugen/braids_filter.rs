//! Procedural phase-reset carrier/window adaptations of Braids positions 17–20.
//!
//! The four source roles use a reset sine carrier, saw/triangle window, pulse
//! and bounded integrator. Analytic sine replaces `wav_sine`; frequencies and
//! smoothing are original, so this is not a numeric source port.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Root, two carriers, integrator, polarity, sync and strike state.
pub const STATE_FLOATS: usize = 7;

fn phase_step(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

/// Render one of four phase-reset digital filter shape roles.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
    let sr = kx.sr.max(1.0);
    let freq = ins[0].first().clamp(20.0, sr * 0.22);
    let shape = ins[1].first().round().clamp(17.0, 20.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let root_step = freq / sr;
    let mod_step = (root_step * 2.0f32.powf((color - 0.5) * 5.0)).min(0.45);
    let sync_step = root_step * (0.25 + sync * 0.7);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 && phase_step(&mut mem[5], sync_step) {
            mem[..4].fill(0.0);
            mem[4] = 0.0;
        }
        let old_phase = mem[0];
        if phase_step(&mut mem[0], root_step) {
            mem[1] = if shape == 18 || shape == 20 { 0.5 } else { 0.0 };
        }
        if old_phase < 0.5 && mem[0] >= 0.5 {
            mem[4] = 1.0 - mem[4];
            mem[2] = if shape == 18 || shape == 20 {
                0.5
            } else {
                0.25
            };
        }
        phase_step(&mut mem[1], mod_step);
        phase_step(&mut mem[2], mod_step);
        let carrier = (TAU * mem[1]).sin();
        let square_carrier = (TAU * mem[2]).sin();
        let saw = 1.0 - 2.0 * mem[0];
        let triangle = 1.0 - 4.0 * (mem[0] - 0.5).abs();
        let window = if timbre < 0.5 { saw } else { triangle };
        let double_saw = 1.0 - 2.0 * (mem[0] * 2.0).fract();
        let pulse = square_carrier * double_saw * if mem[4] > 0.5 { -1.0 } else { 1.0 };
        // Leaky, bounded integral of each polarity-switched pulse.
        mem[3] = (mem[3] * 0.997 + pulse * mod_step * 3.0).clamp(-1.0, 1.0);
        let primary = if shape >= 19 {
            carrier * window
        } else {
            (window * (carrier + 1.0) - 1.0) * 0.5
        };
        let secondary = match shape {
            17 => mem[3],
            18 => (pulse + mem[3]) * 0.5,
            _ => pulse,
        };
        let balance = if timbre < 0.5 {
            timbre * 2.0
        } else {
            (1.0 - timbre) * 2.0
        };
        let body = primary * (1.0 - balance) + secondary * balance;
        let transient = if mem[6] == 0.0 { strike } else { mem[6] };
        mem[6] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (body * (1.0 - 0.2 * transient) + 0.2 * transient * carrier).clamp(-1.0, 1.0);
    }
}
