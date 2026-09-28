//! Single-phase FM percussion with analytic pitch and amplitude envelopes.
//!
//! Source-stage translation of Emilie Gillet's MIT Peaks FM drum. The
//! generated sine, exponential, oscillator-increment and overdrive tables,
//! fixed-point arithmetic and source preset maps are not imported. Vactrol
//! keeps independent FM, auxiliary sweep, noise and drive controls.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

fn bounded(value: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        default
    }
}

/// Analytic replacement for the source's normalized exponential envelope.
fn envelope(position: f32) -> f32 {
    if position >= 1.0 {
        0.0
    } else {
        ((-4.0 * position.max(0.0)).exp() - (-4.0_f32).exp()) / (1.0 - (-4.0_f32).exp())
    }
}

/// One event-local sine phase, with FM/aux pitch envelopes, a delayed
/// previous-sample pitch term, noise mix, AM envelope and soft drive.
/// Pitch is recomputed every four voice samples, regardless of host block
/// boundaries. The voice's `NodeState` and PRNG are reset on event start.
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0xF0D3_17A5).s;
        st.u[1] = 1;
    }
    let mut rng = Rng::new(st.u[0]);
    let sr = kx.sr.max(1.0);
    let mut phase = st.s[0];
    let mut fm_phase = st.s[1];
    let mut am_phase = st.s[2];
    let mut aux_phase = st.s[3];
    let mut previous = st.s[4];
    let mut pitch = st.s[5];
    let mut pitch_countdown = st.u[2];
    for (frame, output) in out.iter_mut().enumerate() {
        if st.done() {
            *output = 0.0;
            continue;
        }
        let frequency = bounded(ins[0].at(frame), 20.0, sr * 0.24, 110.0);
        let fm_amount = bounded(ins[1].at(frame), 0.0, 12.0, 1.5);
        let pitch_sweep = bounded(ins[2].at(frame), 0.0, 4.0, 0.5);
        let decay = bounded(ins[3].at(frame), 0.005, 10.0, 0.4);
        let noise = bounded(ins[4].at(frame), 0.0, 1.0, 0.0);
        let drive = bounded(ins[5].at(frame), 0.0, 1.0, 0.0);

        // The source's FM envelope is faster than its amplitude envelope;
        // its fixed auxiliary pitch pulse is about 20 ms at 48 kHz.
        fm_phase = (fm_phase + 1.0 / (sr * (decay * 0.45).max(0.002))).min(1.0);
        aux_phase = (aux_phase + 1.0 / (sr * 0.02)).min(1.0);
        if pitch_countdown == 0 {
            let semitones = fm_amount * envelope(fm_phase)
                + 4.0 * pitch_sweep * envelope(aux_phase)
                + 4.0 * previous;
            pitch = (frequency * 2.0_f32.powf(semitones / 12.0)).clamp(0.0, sr * 0.45);
            pitch_countdown = 4;
        }
        phase = (phase + pitch / sr).fract();
        let sine = (TAU * phase).sin();
        let mixture = sine * (1.0 - noise) + rng.bipolar() * noise;
        am_phase = (am_phase + 1.0 / (sr * decay)).min(1.0);
        let raw = mixture * envelope(am_phase);
        let overdriven = (5.0 * raw).tanh() / 5.0_f32.tanh();
        let shaped = raw * (1.0 - drive) + overdriven * drive;
        *output = shaped;
        previous = shaped;
        pitch_countdown -= 1;
        if am_phase >= 1.0 {
            st.finish();
        }
    }
    st.s[0] = phase;
    st.s[1] = fm_phase;
    st.s[2] = am_phase;
    st.s[3] = aux_phase;
    st.s[4] = previous;
    st.s[5] = pitch;
    st.u[0] = rng.s;
    st.u[2] = pitch_countdown;
}

#[cfg(test)]
mod tests {
    use super::envelope;

    #[test]
    fn analytic_exponential_envelope_is_bounded_and_finishes_at_one() {
        assert!((envelope(0.0) - 1.0).abs() < 1.0e-6);
        assert!(envelope(0.25) > envelope(0.5));
        assert!(envelope(0.5) > envelope(0.75));
        assert_eq!(envelope(1.0), 0.0);
        assert_eq!(envelope(2.0), 0.0);
    }
}
