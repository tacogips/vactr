//! Six-particle resonant impulse voice, with an original short diffuser.
//!
//! The six impulse/SVF roles follow the MIT Plaits particle engine. The
//! two-line diffuser, TPT filters and seeded RNG differ from its firmware.
//! See THIRD_PARTY_NOTICES.md; this is an adaptation, not a source port.

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

const PARTICLES: usize = 6;
const STRIDE: usize = 4;
const DELAY_LEN: usize = 1024;
const DELAY_BASE: usize = PARTICLES * STRIDE;
/// Floats reserved at graph installation for each main or auxiliary node.
pub const STATE_FLOATS: usize = DELAY_BASE + 2 * DELAY_LEN;

#[inline]
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

#[inline]
fn impulse_filter(state: &mut [f32], input: f32, sr: f32, q: f32) -> f32 {
    let g = (std::f32::consts::PI * state[2] / sr).tan();
    let damping = 1.0 / q;
    let high = (input - state[0] - damping * state[1]) / (1.0 + g * (g + damping));
    let band = state[1] + g * high;
    state[0] += g * (state[1] + band);
    state[1] = band + g * high;
    band * state[3]
}

#[inline]
fn allpass(mem: &mut [f32], position: usize, delay: usize, input: f32, coefficient: f32) -> f32 {
    let read = (position + DELAY_LEN - delay) % DELAY_LEN;
    let delayed = mem[read];
    let output = delayed - coefficient * input;
    mem[position] = input + coefficient * output;
    output
}

/// `mode=0` emits filtered/diffused resonances; `mode=1` emits impulses.
/// Separate nodes use the same seed and controls, retaining aligned events.
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
    let spread = 48.0 * unit(ins[1].first()).powi(2);
    let timbre = unit(ins[2].first());
    let morph = unit(ins[3].first());
    let impulse_output = ins[4].first() >= 0.5;
    let density_hz = 261.625_55 * 2.0_f32.powf(6.0 * timbre * timbre);
    let probability =
        ((density_hz / 48_000.0).powi(2) * 48_000.0 / sr / PARTICLES as f32).clamp(1.0e-8, 0.1);
    let q = 0.5 + 2.0_f32.powf(20.0 * (morph - 0.5).max(0.0));
    let diffusion = if morph < 0.5 {
        (1.0 - 2.0 * morph).powi(2)
    } else {
        0.0
    };
    let first_block = st.u[1] == 0;
    if first_block {
        st.u[0] = Rng::new(kx.seed ^ 0x5EED_1800).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    let mut position = st.u[2] as usize % DELAY_LEN;
    let delay_a = ((149.0 * sr / 48_000.0) as usize).clamp(1, DELAY_LEN - 1);
    let delay_b = ((337.0 * sr / 48_000.0) as usize).clamp(1, DELAY_LEN - 1);
    let post_coefficient = 1.0 - (-2.0 * std::f32::consts::PI * freq / sr).exp();
    for (sample_index, sample) in out.iter_mut().enumerate() {
        let mut impulses = 0.0;
        let mut resonant = 0.0;
        for particle in mem[..DELAY_BASE].chunks_exact_mut(STRIDE) {
            let fired = (sample_index == 0 && first_block) || rng.unit() <= probability;
            let input = if fired { 1.0 } else { 0.0 };
            if fired {
                let offset = 2.0 * rng.unit() - 1.0;
                particle[2] = (freq * 2.0_f32.powf(spread * offset / 12.0)).clamp(20.0, sr * 0.24);
                particle[3] = (0.5 / (q * (particle[2] / sr) * probability.sqrt()).sqrt()).min(8.0);
            }
            impulses += input;
            if particle[2] > 0.0 {
                resonant += impulse_filter(particle, input, sr, q);
            }
        }
        let filtered = st.s[0] + post_coefficient * (resonant - st.s[0]);
        st.s[0] = filtered;
        let (_, delays) = mem.split_at_mut(DELAY_BASE);
        let (first, second) = delays.split_at_mut(DELAY_LEN);
        let wet_a = allpass(first, position, delay_a, filtered, 0.57);
        let wet_b = allpass(second, position, delay_b, wet_a, 0.57);
        *sample = if impulse_output {
            (impulses / PARTICLES as f32).clamp(-1.0, 1.0)
        } else {
            ((1.0 - diffusion) * filtered + diffusion * wet_b).tanh()
        };
        position = (position + 1) % DELAY_LEN;
    }
    st.u[0] = rng.s;
    st.u[2] = position as u32;
}
