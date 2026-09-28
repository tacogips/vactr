//! Original analytic two-operator FM adaptations of Braids positions 25–27.
//!
//! Source sine lookup, fixed-point phase arithmetic and block interpolation
//! are replaced. Feedback uses previous carrier output; chaotic mode changes
//! modulator rate from the current output, with bounded phase increments.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use std::f32::consts::TAU;

/// Carrier/modulator phases, feedback, sync clock and strike envelope.
pub const STATE_FLOATS: usize = 5;

fn advance(phase: &mut f32, step: f32) -> bool {
    *phase += step;
    if *phase >= 1.0 {
        *phase -= 1.0;
        true
    } else {
        false
    }
}

/// Render FM, feedback FM or chaotic feedback FM, selected by source position.
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
    let shape = ins[1].first().round().clamp(25.0, 27.0) as usize;
    let color = ins[2].first().clamp(0.0, 1.0);
    let timbre = ins[3].first().clamp(0.0, 1.0);
    let strike = ins[4].first().clamp(0.0, 1.0);
    let sync = ins[5].first().clamp(0.0, 1.0);
    let carrier_step = freq / sr;
    let mod_step = (carrier_step * 2.0f32.powf((timbre - 0.5) * 5.0)).min(0.45);
    let index = 0.05 + color * 4.5;
    let sync_step = carrier_step * (0.25 + 0.7 * sync);
    let strike_decay = (-1.0 / (0.025 * sr)).exp();
    for sample in out.iter_mut() {
        if sync > 0.0 && advance(&mut mem[3], sync_step) {
            mem[..3].fill(0.0);
        }
        advance(&mut mem[0], carrier_step);
        let rate = if shape == 27 {
            // Output-dependent rate, limited to a positive, sub-Nyquist step.
            (mod_step * (1.0 + 0.75 * mem[2])).clamp(mod_step * 0.25, 0.45)
        } else {
            mod_step
        };
        advance(&mut mem[1], rate);
        let feedback = if shape == 26 { mem[2] * 0.42 } else { 0.0 };
        let modulator = (TAU * (mem[1] + feedback)).sin();
        let output = (TAU * mem[0] + index * modulator).sin();
        mem[2] = output;
        let transient = if mem[4] == 0.0 { strike } else { mem[4] };
        mem[4] = (transient * strike_decay).max(f32::MIN_POSITIVE);
        *sample = (output * (1.0 - 0.15 * transient)
            + 0.15 * transient * (TAU * mem[0] * 3.0).sin())
        .clamp(-1.0, 1.0);
    }
}
