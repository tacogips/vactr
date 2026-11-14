//! Five-mode 808-style shell and filtered snare noise (MIT translation).

use std::f32::consts::TAU;

use super::{rate_pole, soft_clip, svf};
use crate::dsp::effects::prim::Rng;
use crate::dsp::ugen::NodeState;

const MODES: [f32; 5] = [1.0, 2.0, 3.18, 4.16, 5.62];
const PHASE: usize = 10;
const PULSE: usize = 15;
const PULSE_LP: usize = 16;
const NOISE_ENV: usize = 17;
const SUSTAIN_GAIN: usize = 18;
const NOISE_SVF: usize = 19;

fn mode_gains(tone: f32) -> [f32; 5] {
    let mut gain = [0.0; 5];
    if tone < 2.0 / 3.0 {
        let shape = tone * 1.5;
        gain[0] = 1.5 + (1.0 - shape).powi(2) * 4.5;
        gain[1] = 2.0 * shape + 0.15;
    } else {
        let mut shape = (tone - 2.0 / 3.0) * 3.0;
        gain[0] = 1.5 - shape * 0.5;
        gain[1] = 2.15 - shape * 0.7;
        for item in &mut gain[2..] {
            *item = shape;
            shape *= shape;
        }
    }
    gain
}

#[inline]
fn filtered_band(input: f32, f: f32, q: f32, m: &mut [f32], index: usize) -> f32 {
    let (left, right) = m.split_at_mut(index + 1);
    svf(input, f, q, &mut left[index], &mut right[0]).0
}

/// Trigger pulse, five resonant shell modes, positive-half noise envelope,
/// source soft-clip and bandpass output mix.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    m: &mut [f32],
    st: &mut NodeState,
    freq: f32,
    harmonics: f32,
    tone: f32,
    decay: f32,
    accent: f32,
    sustain: bool,
    gate: bool,
    sr: f32,
    remaining: f32,
    seed: u32,
) -> f32 {
    if st.u[2] == 0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            st.u[1] = (0.001 * sr).round() as u32;
        }
        st.u[0] = Rng::new(seed ^ 0xA71A_6021).s;
        st.u[2] = 1;
        m[NOISE_ENV] = 2.0;
    }
    let mut rng = Rng::new(st.u[0]);
    let f0 = (freq / sr).min(0.4);
    let decay_xt = decay * (1.0 + decay * (decay - 1.0));
    let q = 2000.0 * 2.0_f32.powf(decay_xt * 7.0);
    let snappy = (harmonics * 1.1 - 0.05).clamp(0.0, 1.0);
    let exciter_leak = harmonics * (2.0 - harmonics) * 0.1;
    let source_noise_decay = 1.0 - 0.0017 * 2.0_f32.powf(-decay * (50.0 + harmonics * 10.0) / 12.0);
    let noise_decay = source_noise_decay.clamp(0.0, 1.0).powf(48_000.0 / sr);
    let mut pulse = if st.u[1] != 0 {
        st.u[1] -= 1;
        m[PULSE] = 3.0 + 7.0 * accent - if st.u[1] == 0 { 1.0 } else { 0.0 };
        m[PULSE]
    } else {
        m[PULSE] *= (1.0 - 1.0 / (0.0001 * sr).max(1.0)).max(0.0);
        m[PULSE]
    };
    if !pulse.is_finite() {
        pulse = 0.0;
    }
    m[PULSE_LP] += rate_pole(0.75, sr) * (pulse - m[PULSE_LP]);
    let target_sustain = if gate { accent * decay } else { 0.0 };
    m[SUSTAIN_GAIN] += (target_sustain - m[SUSTAIN_GAIN]) / remaining;
    let gain = mode_gains(tone);
    let mut shell = 0.0;
    for (index, ratio) in MODES.into_iter().enumerate() {
        let f = (f0 * ratio).min(0.497);
        let resonance = 1.0 + f * if index == 0 { q } else { q * 0.25 };
        let excitation = if index == 0 {
            pulse - m[PULSE_LP] + 0.006 * pulse
        } else {
            0.026 * pulse
        };
        let response = if sustain {
            m[PHASE + index] = (m[PHASE + index] + f).fract();
            (TAU * m[PHASE + index]).sin() * m[SUSTAIN_GAIN] * 0.25
        } else {
            filtered_band(excitation, f, resonance, m, index * 2) + excitation * exciter_leak
        };
        shell += gain[index] * response;
    }
    shell = soft_clip(shell);
    let noise = rng.bipolar().max(0.0);
    st.u[0] = rng.s;
    m[NOISE_ENV] *= noise_decay;
    let noise = noise
        * (if sustain {
            m[SUSTAIN_GAIN]
        } else {
            m[NOISE_ENV]
        })
        * snappy
        * 2.0;
    let noise_f = (f0 * 16.0).min(0.497);
    let noise = filtered_band(noise, noise_f, 1.0 + noise_f * 1.5, m, NOISE_SVF);
    noise + shell * (1.0 - snappy)
}
