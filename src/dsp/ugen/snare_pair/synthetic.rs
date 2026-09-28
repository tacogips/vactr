//! Two coupled 909-style oscillators and filtered snare noise (MIT translation).

use std::f32::consts::PI;

use super::svf;
use crate::dsp::effects::prim::Rng;
use crate::dsp::ugen::NodeState;

const PHASE_0: usize = 0;
const PHASE_1: usize = 1;
const DRUM_AMP: usize = 2;
const SNARE_AMP: usize = 3;
const FM: usize = 4;
const SUSTAIN_GAIN: usize = 5;
const DRUM_LP: usize = 6;
const SNARE_SVF_BP: usize = 7;
const SNARE_SVF_LP: usize = 8;
const SNARE_HP: usize = 9;

#[inline]
fn distorted_sine(phase: f32) -> f32 {
    let triangle = (if phase < 0.5 { phase } else { 1.0 - phase }) * 4.0 - 1.3;
    2.0 * triangle / (1.0 + triangle.abs())
}

#[inline]
fn one_pole(input: f32, f: f32, state: &mut f32) -> (f32, f32) {
    let g = (PI * f.min(0.497)).tan();
    let lp = (g * input + *state) / (1.0 + g);
    *state = g * (input - lp) + lp;
    (lp, input - lp)
}

#[inline]
fn snare_lowpass(input: f32, f: f32, q: f32, m: &mut [f32]) -> f32 {
    let (left, right) = m.split_at_mut(SNARE_SVF_LP);
    svf(input, f, q, &mut left[SNARE_SVF_BP], &mut right[0]).1
}

/// Source drum/snare envelopes, 40–70 ms noise hold, coupled oscillators,
/// drum lowpass and lowpass/highpass snare coloration.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    m: &mut [f32],
    st: &mut NodeState,
    freq: f32,
    harmonics: f32,
    timbre: f32,
    decay: f32,
    accent: f32,
    sustain: bool,
    gate: bool,
    sr: f32,
    remaining: f32,
    seed: u32,
) -> f32 {
    if st.s[2] == 0.0 {
        m[DRUM_AMP] = 0.3 + 0.7 * accent;
        m[SNARE_AMP] = m[DRUM_AMP];
        m[FM] = 1.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            st.u[1] = ((0.04 + decay * 0.03) * sr).round() as u32;
        }
        st.u[0] = Rng::new(seed ^ 0x5A4E_1D21).s;
        st.s[2] = 1.0;
    }
    let mut rng = Rng::new(st.u[0]);
    let f0 = (freq / sr).min(0.4);
    let fm_amount = timbre * timbre;
    let decay_xt = decay * (1.0 + decay * (decay - 1.0));
    let drum_decay = (1.0
        - 2.0_f32.powf((-decay_xt * 72.0 - fm_amount * 12.0 + harmonics * 7.0) / 12.0)
            / (0.015 * sr))
        .clamp(0.0, 1.0);
    let snare_decay = (1.0 - 2.0_f32.powf((-decay * 60.0 - harmonics * 7.0) / 12.0) / (0.01 * sr))
        .clamp(0.0, 1.0);
    let fm_decay = (1.0 - 1.0 / (0.007 * sr)).clamp(0.0, 1.0);
    let snappy = (harmonics * 1.1 - 0.05).clamp(0.0, 1.0);
    let drum_level = (1.0 - snappy).sqrt();
    let snare_level = snappy.sqrt();
    let target_sustain = if gate { accent * decay } else { 0.0 };
    m[SUSTAIN_GAIN] += (target_sustain - m[SUSTAIN_GAIN]) / remaining;
    if sustain {
        m[DRUM_AMP] = m[SUSTAIN_GAIN];
        m[SNARE_AMP] = m[SUSTAIN_GAIN];
        m[FM] = 0.0;
    } else {
        // The source halves its very quiet drum tail's decay updates.
        if m[DRUM_AMP] > 0.03 || st.u[2] & 1 == 0 {
            m[DRUM_AMP] *= drum_decay;
        }
        if st.u[1] != 0 {
            st.u[1] -= 1;
        } else {
            m[SNARE_AMP] *= snare_decay;
        }
        m[FM] *= fm_decay;
    }
    st.u[2] = st.u[2].wrapping_add(1);
    let mut reset_noise =
        (if m[PHASE_0] > 0.5 { -1.0 } else { 1.0 }) + (if m[PHASE_1] > 0.5 { -1.0 } else { 1.0 });
    let reset_amount = ((0.125 - f0) * 8.0).clamp(0.0, 1.0).powi(2) * fm_amount;
    reset_noise *= reset_amount * 0.025;
    let f = f0 * (1.0 + fm_amount * 4.0 * m[FM]);
    m[PHASE_0] += f;
    m[PHASE_1] += f * 1.47;
    for index in [PHASE_0, PHASE_1] {
        if reset_amount > 0.1 {
            if m[index] >= 1.0 + reset_noise {
                m[index] = 1.0 - m[index];
            }
        } else if m[index] >= 1.0 {
            m[index] -= 1.0;
        }
    }
    let drum = (-0.1 + distorted_sine(m[PHASE_0]) * 0.60 + distorted_sine(m[PHASE_1]) * 0.25)
        * m[DRUM_AMP]
        * drum_level;
    let drum = one_pole(drum, (3.0 * f0).min(0.497), &mut m[DRUM_LP]).0;
    let noise = (rng.bipolar() + 1.0) * 0.5;
    st.u[0] = rng.s;
    let snare = snare_lowpass(noise, (35.0 * f0).min(0.497), 0.5 + 2.0 * snappy, m);
    let snare = one_pole(snare, (10.0 * f0).min(0.497), &mut m[SNARE_HP]).1;
    let snare = (snare + 0.1) * (m[SNARE_AMP] + m[FM]) * snare_level;
    snare + drum
}
