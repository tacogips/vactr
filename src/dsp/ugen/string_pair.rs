//! Three bounded strings with source-inspired excitation and nonlinearity.
//!
//! Vactrol events own their voices, so the active string is selected per event
//! instead of using the source's persistent cross-event rotation. This is an
//! adaptation; see THIRD_PARTY_NOTICES.md.

use std::f32::consts::PI;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const STRINGS: usize = 3;
const DELAY_LEN: usize = 2048;
const STRETCH_LEN: usize = 512;
const SCALARS: usize = 12;
const STRIDE: usize = DELAY_LEN + STRETCH_LEN + SCALARS;
/// Three string delay and stretch lines plus scalar state, per output node.
pub const STATE_FLOATS: usize = STRINGS * STRIDE;

const WRITE: usize = 0;
const STRETCH_WRITE: usize = 1;
const EXCITER: usize = 2;
const REMAIN: usize = 3;
const DAMPING: usize = 4;
const DC_INPUT: usize = 5;
const DC_OUTPUT: usize = 6;
const BRIDGE: usize = 7;
const DISPERSION: usize = 8;

#[derive(Clone, Copy)]
struct StringControls {
    freq: f32,
    structure: f32,
    brightness: f32,
    damping: f32,
    sr: f32,
}

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn delay_read(line: &[f32], write: usize, delay: f32) -> f32 {
    let offset = delay.floor() as usize;
    let fraction = delay.fract();
    let a = line[(write + line.len() - offset) % line.len()];
    let b = line[(write + line.len() - offset - 1) % line.len()];
    a + fraction * (b - a)
}

#[inline]
fn string_sample(
    state: &mut [f32],
    excitation: f32,
    controls: StringControls,
    rng: &mut Rng,
) -> f32 {
    let StringControls {
        freq,
        structure,
        brightness,
        damping,
        sr,
    } = controls;
    let (line, rest) = state.split_at_mut(DELAY_LEN);
    let (stretch, scalars) = rest.split_at_mut(STRETCH_LEN);
    let write = scalars[WRITE] as usize % DELAY_LEN;
    let stretch_write = scalars[STRETCH_WRITE] as usize % STRETCH_LEN;
    let damping_cutoff = (12.0 + damping * damping * 60.0 + brightness * 24.0).min(84.0);
    let ratio = 2.0_f32.powf(damping_cutoff / 12.0);
    // MIT lookup_tables.py formula, evaluated analytically, no imported table.
    let shift = 1.0 - (1.0 / ratio).atan() / PI;
    let nominal_delay = (sr / freq * shift).clamp(4.0, (DELAY_LEN - 4) as f32);
    let nonlinearity = if structure < 0.24 {
        (structure - 0.24) * 4.166
    } else if structure > 0.26 {
        (structure - 0.26) * 1.351_35
    } else {
        0.0
    };
    let amount = nonlinearity.abs().min(1.0);
    let delay = if nonlinearity > 0.0 {
        let noise_coefficient = 0.06 + 0.94 * brightness * brightness;
        scalars[DISPERSION] += noise_coefficient * (rng.unit() - 0.5 - scalars[DISPERSION]);
        nominal_delay * (1.0 + scalars[DISPERSION] * (amount - 0.75).max(0.0).powi(2) * 1.6)
    } else {
        nominal_delay * (1.0 - scalars[BRIDGE] * amount * amount * 0.01)
    }
    .clamp(4.0, (DELAY_LEN - 4) as f32);
    let mut feedback = delay_read(line, write, delay);
    if nonlinearity > 0.0 {
        let stretch_delay =
            (delay * amount * (2.0 - amount) * 0.225).clamp(1.0, (STRETCH_LEN - 2) as f32);
        let coefficient = -0.618 * amount / (0.15 + amount);
        let delayed = delay_read(stretch, stretch_write, stretch_delay);
        let written = feedback + coefficient * delayed;
        stretch[stretch_write] = written;
        feedback = delayed - coefficient * written;
    } else {
        let value = feedback.abs() - 0.025;
        scalars[BRIDGE] = (value.abs() + value) * if feedback > 0.0 { 1.0 } else { -1.5 };
    }
    let cutoff = (freq * 2.0_f32.powf(damping_cutoff / 12.0)).min(sr * 0.49);
    let coefficient = 1.0 - (-2.0 * PI * cutoff / sr).exp();
    let driven = (feedback + excitation).clamp(-20.0, 20.0);
    scalars[DAMPING] += coefficient * (driven - scalars[DAMPING]);
    let dc = scalars[DAMPING] - scalars[DC_INPUT]
        + (1.0 - 20.0 / sr).clamp(0.0, 1.0) * scalars[DC_OUTPUT];
    scalars[DC_INPUT] = scalars[DAMPING];
    scalars[DC_OUTPUT] = dc;
    line[write] = dc;
    scalars[WRITE] = ((write + 1) % DELAY_LEN) as f32;
    scalars[STRETCH_WRITE] = ((stretch_write + 1) % STRETCH_LEN) as f32;
    dc
}

/// Mode 0 emits the three-string resonator; mode 1 the filtered excitation.
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
    let freq = ins[0].first().clamp(20.0, sr * 0.24);
    let structure = unit(ins[1].first());
    let accent = unit(ins[4].first());
    let mut brightness = unit(ins[2].first()).powi(2);
    let mut damping = unit(ins[3].first());
    let aux = ins[5].first() >= 0.5;
    let sustain = ins[6].first() >= 0.5;
    let density = brightness * brightness;
    brightness += 0.25 * accent * (1.0 - brightness);
    damping += 0.25 * accent * (1.0 - damping);
    let first_block = st.u[1] == 0;
    if first_block {
        st.u[0] = Rng::new(kx.seed ^ 0x5712_1900).s;
        st.u[1] = 1;
        st.u[2] = kx.seed % STRINGS as u32;
        let active = st.u[2] as usize;
        let scalars = &mut mem[active * STRIDE + DELAY_LEN + STRETCH_LEN..][..SCALARS];
        scalars[REMAIN] = (sr / freq).min((DELAY_LEN - 4) as f32);
    }
    let active = st.u[2] as usize;
    let mut rng = Rng::new(st.u[0]);
    let cutoff =
        (4.0 * freq * 2.0_f32.powf((brightness * (2.0 - brightness) - 0.5) * 6.0)).min(sr * 0.49);
    let exciter_coefficient = 1.0 - (-2.0 * PI * cutoff / sr).exp();
    let dust_probability = (0.00005 + 0.99995 * density * density) * 48_000.0 / sr;
    let controls = StringControls {
        freq,
        structure,
        brightness,
        damping,
        sr,
    };
    for sample in out.iter_mut() {
        let mut resonant = 0.0;
        let mut excitation_sum = 0.0;
        for (index, state) in mem[..STATE_FLOATS].chunks_exact_mut(STRIDE).enumerate() {
            let scalars = &mut state[DELAY_LEN + STRETCH_LEN..];
            let raw = if sustain && index == active {
                let random = rng.unit();
                if random < dust_probability {
                    random / dust_probability * (8.0 - 6.0 * dust_probability) * accent
                } else {
                    0.0
                }
            } else if index == active && scalars[REMAIN] > 0.0 {
                scalars[REMAIN] -= 1.0;
                2.0 * rng.unit() - 1.0
            } else {
                0.0
            };
            scalars[EXCITER] += exciter_coefficient * (raw - scalars[EXCITER]);
            let excitation = scalars[EXCITER];
            excitation_sum += excitation;
            resonant += string_sample(state, excitation, controls, &mut rng);
        }
        *sample = if aux { excitation_sum } else { resonant };
    }
    st.u[0] = rng.s;
}
