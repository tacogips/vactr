//! Plaits position 20 modal signal stages (MIT translation, not bit-exact).
//!
//! Twenty-four resonators follow an analytic stiffness curve; the auxiliary
//! output is the filtered strike/dust excitation. See THIRD_PARTY_NOTICES.md.

use std::f32::consts::PI;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const MODES: usize = 24;
/// Two filter integrators per resonant mode, reserved before audio callbacks.
pub const STATE_FLOATS: usize = MODES * 2;

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

// Analytic evaluation of the MIT lookup_tables.py stiffness generator.
#[inline]
fn stiffness(structure: f32) -> f32 {
    if structure < 0.25 {
        -0.25 * (0.25 - structure)
    } else if structure < 0.3 {
        0.0
    } else if structure < 0.9 {
        0.01 * 10.0_f32.powf((structure - 0.3) / 0.6 * 2.005) - 0.01
    } else {
        let g = ((structure - 0.9) / 0.1).clamp(0.0, 1.0).powi(2);
        1.5 - 0.5 * (PI * g).cos()
    }
}

#[inline]
fn svf_low(state: &mut [f32; 2], input: f32, f: f32, q: f32, sr: f32) -> f32 {
    let g = (PI * (f / sr).min(0.49)).tan();
    let r = 1.0 / q;
    let hp = (input - (r + g) * state[0] - state[1]) / (1.0 + r * g + g * g);
    let bp = g * hp + state[0];
    state[0] = g * hp + bp;
    let lp = g * bp + state[1];
    state[1] = g * bp + lp;
    lp
}

#[inline]
fn svf_band(state: &mut [f32], input: f32, f: f32, q: f32, sr: f32) -> f32 {
    let g = (PI * (f / sr).min(0.49)).tan();
    let r = 1.0 / q;
    let hp = (input - (r + g) * state[0] - state[1]) / (1.0 + r * g + g * g);
    let bp = g * hp + state[0];
    state[0] = g * hp + bp;
    let lp = g * bp + state[1];
    state[1] = g * bp + lp;
    bp
}

/// Mode 0 emits the resonator; mode 1 emits filtered excitation. New events
/// reset both paths and create a deterministic strike unless sustain is set.
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
    let frequency = ins[0].first().clamp(20.0, sr * 0.25);
    let structure = unit(ins[1].first());
    let accent = unit(ins[4].first());
    let mut brightness = unit(ins[2].first());
    let mut damping = unit(ins[3].first());
    let excitation_output = ins[5].first() >= 0.5;
    let sustain = ins[6].first() >= 0.5;
    let density = brightness * brightness;
    brightness += 0.25 * accent * (1.0 - brightness);
    damping += 0.25 * accent * (1.0 - damping);
    let range = if sustain { 36.0 } else { 60.0 };
    let excitation_f = if sustain { 4.0 } else { 2.0 } * frequency;
    let cutoff = (excitation_f
        * 2.0_f32.powf((brightness * (2.0 - brightness) - 0.5) * range / 12.0))
    .min(sr * 0.49);
    let exciter_q = if sustain { 0.7 } else { 1.5 };
    let stiffness = stiffness(structure);
    let compensation =
        1.0 / (1.0 + stiffness + stiffness * (if stiffness < 0.0 { 0.93 } else { 0.98 }));
    let fundamental = frequency * compensation;
    let mut harmonic = fundamental;
    let mut stretch = 1.0;
    let mut stiffness_step = stiffness;
    let q_base = 500.0 * 2.0_f32.powf(damping * 79.7 / 6.0);
    let adjusted_brightness = brightness * (1.0 - structure * 0.3) * (1.0 - damping * 0.3);
    let q_loss = adjusted_brightness * (2.0 - adjusted_brightness) * 0.85 + 0.15;
    let mut mode_frequency = [0.0; MODES];
    let mut mode_q = [0.0; MODES];
    let mut mode_gain = [0.0; MODES];
    let mut q = q_base;
    for index in 0..MODES {
        let f = (harmonic * stretch).clamp(1.0, sr * 0.49);
        mode_frequency[index] = f;
        mode_q[index] = 1.0 + (f / sr) * q;
        #[allow(clippy::cast_precision_loss)]
        let amplitude = (PI * (index as f32 + 0.5) * 0.015).cos() * 0.25;
        mode_gain[index] = amplitude * (1.0 - 2.0 * f / sr);
        stretch += stiffness_step;
        stiffness_step *= if stiffness_step < 0.0 { 0.93 } else { 0.98 };
        harmonic += fundamental;
        q *= q_loss;
    }
    let first_block = st.u[1] == 0;
    if first_block {
        st.u[0] = Rng::new(kx.seed ^ 0xC0DA_1200).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    let dust_probability = (0.00005 + 0.99995 * density * density) * 48_000.0 / sr;
    let strike =
        (0.12 + 0.08 * accent) * (1.0 - damping * 0.5) * 2.0_f32.powf((cutoff / sr).powi(2) * 2.0)
            / (cutoff / sr).max(1.0e-4);
    let mut exciter_state = [st.s[0], st.s[1]];
    for (sample_index, sample) in out.iter_mut().enumerate() {
        let raw = if sustain {
            let random = rng.unit();
            if random < dust_probability {
                random / dust_probability * (4.0 - 3.0 * dust_probability) * accent
            } else {
                0.0
            }
        } else if first_block && sample_index == 0 {
            strike
        } else {
            0.0
        };
        let excitation = svf_low(&mut exciter_state, raw, cutoff, exciter_q, sr);
        if excitation_output {
            *sample = excitation;
        } else {
            let mut resonant = 0.0;
            for (index, state) in mem[..STATE_FLOATS].chunks_exact_mut(2).enumerate() {
                resonant += mode_gain[index]
                    * svf_band(state, excitation, mode_frequency[index], mode_q[index], sr);
            }
            *sample = resonant;
        }
    }
    st.s[0] = exciter_state[0];
    st.s[1] = exciter_state[1];
    st.u[0] = rng.s;
}
