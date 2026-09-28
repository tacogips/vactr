//! Original dual-FM/spatial alternate for Elements roles.
//!
//! Fourfold analytic sine generation and a one-pole anti-alias low-pass are
//! Vactrol designs. No upstream 101-tap FIR, LUT, sample or ratio array is
//! imported. State is caller-owned fixed memory, never callback-allocated.

use std::f32::consts::TAU;

pub const STATE_FLOATS: usize = 320;
const ECHO_START: usize = 64;
const ECHO_LEN: usize = 256;

/// One host-rate sample of a dual two-operator FM voice. `p` contains the
/// twenty Patch fields followed by gate, note, modulation and strength.
#[allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn sample(p: &[f32], state: &mut [f32], blow: f32, strike: f32, sr: f32) -> (f32, f32) {
    if p.len() < 24 || state.len() < STATE_FLOATS {
        return (0.0, 0.0);
    }
    let sr = sr.max(1.0);
    let age = state[0];
    let gate = p[20].clamp(0.0, 1.0);
    let strength = p[23].clamp(0.0, 1.0);
    let decay = 0.05 + p[0].clamp(0.0, 1.0) * 2.0;
    let envelope = if gate >= 0.5 {
        0.3 + 0.7 * (-age / decay).exp()
    } else {
        (-age / decay).exp()
    };
    let level = (envelope * (0.2 + strength * 0.8)).clamp(0.0, 1.0);
    let frequency = (220.0
        * 2.0_f32.powf((p[21].clamp(-48.0, 48.0) + p[22].clamp(-1.0, 1.0) * 12.0) / 12.0))
    .clamp(20.0, sr * 0.35);
    state[15] = (state[15] + (0.02 + p[19] * 8.0) / sr).fract();
    let tremolo = 1.0 + p[22].clamp(-1.0, 1.0) * (TAU * state[15]).sin() * 0.35;
    let cutoff = (130.0 + p[11] * 11_000.0 + level * p[12] * 2_500.0).min(sr * 0.43);
    let lowpass = 1.0 - (-TAU * cutoff / (sr * 4.0)).exp();
    let mut main = 0.0;
    let mut aux = 0.0;
    for osc in 0..2 {
        let offset = if osc == 0 { 1 } else { 4 };
        let external = if osc == 0 { blow } else { strike };
        let ratio = 0.6 + (if osc == 0 { p[4] } else { p[7] }) * 3.5;
        let index = 0.2 + (if osc == 0 { p[5] } else { p[8] }) * 5.0;
        let gain = if osc == 0 { p[3] } else { p[6] };
        let detune = if osc == 0 { 1.0 } else { 1.0 + p[1] * 0.08 };
        let feedback = (0.05 + p[2] * 0.7) * (0.3 + p[9] * 0.7);
        let rate = (frequency * detune).min(sr * 0.4);
        let mut downsampled = 0.0;
        for _ in 0..4 {
            state[offset] = (state[offset] + rate / (sr * 4.0)).fract();
            state[offset + 1] = (state[offset + 1] + rate * ratio / (sr * 4.0)).fract();
            let modulator = (TAU * state[offset + 1] + feedback * state[offset + 2]).sin();
            let carrier =
                (TAU * state[offset] + modulator * index + external * (0.2 + gain * 1.8)).sin();
            state[offset + 2] = modulator;
            let filtered = state[7 + osc] + lowpass * (carrier - state[7 + osc]);
            state[7 + osc] = filtered;
            downsampled += filtered * 0.25;
        }
        let filter_color = p[10].clamp(0.0, 1.0);
        state[11 + osc] += (0.04 + p[15] * 0.6) * (downsampled - state[11 + osc]);
        let colored = (downsampled * (1.0 - filter_color) + state[11 + osc] * filter_color)
            * gain
            * level
            * tremolo;
        state[9 + osc] = (state[9 + osc]
            + (0.02 + p[14] * 5.0) * (if osc == 0 { 1.0 } else { 1.13 }) / sr)
            .fract();
        let spread = p[13].clamp(0.0, 1.0);
        let pan = ((TAU * state[9 + osc]).sin() * spread + if osc == 0 { -0.45 } else { 0.45 })
            .clamp(-1.0, 1.0);
        main += colored * (1.0 - pan) * 0.5;
        aux += colored * (1.0 + pan) * 0.5;
    }
    let write = state[14] as usize % ECHO_LEN;
    let echo = state[ECHO_START + write];
    state[13] += (0.02 + p[17] * 0.8) * (echo - state[13]);
    if p[18] < 1.75 {
        state[ECHO_START + write] =
            ((main + aux) * 0.2 + state[13] * (0.1 + p[16] * 0.82)).clamp(-2.0, 2.0);
    }
    state[14] = ((write + 1) % ECHO_LEN) as f32;
    let space = (p[18] * 0.5).clamp(0.0, 1.0);
    state[0] = (age + 1.0 / sr).min(10.0);
    (
        (main + state[13] * space * 0.3).tanh(),
        (aux + state[13] * space * 0.2).tanh(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_space_freezes_bounded_echo_without_stopping_spatial_voice() {
        let mut patch = [0.5; 24];
        patch[20] = 1.0;
        let mut state = [0.0; STATE_FLOATS];
        for _ in 0..300 {
            let _ = sample(&patch, &mut state, 0.3, 0.1, 48_000.0);
        }
        let captured = state[ECHO_START..ECHO_START + ECHO_LEN].to_vec();
        patch[18] = 1.75;
        let mut energy = 0.0;
        for _ in 0..600 {
            let (main, aux) = sample(&patch, &mut state, 0.3, 0.1, 48_000.0);
            assert!(main.is_finite() && aux.is_finite());
            energy += main.abs() + aux.abs();
        }
        assert!(energy > 1.0);
        assert_eq!(&state[ECHO_START..ECHO_START + ECHO_LEN], captured);
        patch[18] = 1.749;
        for _ in 0..ECHO_LEN {
            let _ = sample(&patch, &mut state, 0.3, 0.1, 48_000.0);
        }
        assert_ne!(&state[ECHO_START..ECHO_START + ECHO_LEN], captured);
    }
}
