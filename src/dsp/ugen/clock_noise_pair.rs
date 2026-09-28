//! Event-local clocked noise with three source-stage SVF signal paths.
//!
//! Informed by MIT Plaits `noise_engine.cc`, `clocked_noise.h`, and MIT stmlib
//! BLEP/SVF helpers. No global RNG, generated table or audio asset is used.
//! The Vactr clock uses the source's patched-trigger pitch range; its RNG,
//! host-rate timing and exact filter coefficients remain numerical differences.

use std::f32::consts::PI;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

#[derive(Clone, Copy)]
struct Clock {
    phase: f32,
    held: f32,
    next: f32,
    frequency: f32,
}

impl Clock {
    fn from_slice(state: &[f32]) -> Self {
        Self {
            phase: state[0],
            held: state[1],
            next: state[2],
            frequency: state[3],
        }
    }

    fn write(self, state: &mut [f32]) {
        state.copy_from_slice(&[self.phase, self.held, self.next, self.frequency]);
    }

    fn sample(&mut self, rng: &mut Rng) -> f32 {
        let mut current = self.next;
        self.next = 0.0;
        let raw = rng.bipolar();
        let white_mix = (4.0 * (self.frequency - 0.25)).clamp(0.0, 1.0);
        self.phase += self.frequency;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            let crossing = self.phase / self.frequency.max(f32::MIN_POSITIVE);
            let jump = raw - self.held;
            current += jump * 0.5 * crossing * crossing;
            let following = 1.0 - crossing;
            self.next -= jump * 0.5 * following * following;
            self.held = raw;
        }
        self.next += self.held;
        current + white_mix * (raw - current)
    }
}

#[derive(Clone, Copy)]
struct Svf {
    band: f32,
    low: f32,
}

impl Svf {
    fn step(&mut self, input: f32, frequency: f32, q: f32) -> (f32, f32, f32) {
        let g = (PI * frequency.clamp(0.000_01, 0.49)).tan();
        let r = 1.0 / q.max(0.001);
        let h = 1.0 / (1.0 + r * g + g * g);
        let high = (input - (r + g) * self.band - self.low) * h;
        let band = g * high + self.band;
        self.band = g * high + band;
        let low = g * band + self.low;
        self.low = g * band + low;
        (low, band, high)
    }
}

fn bounded(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

fn targets(ins: &[Inp<'_>; MAX_PORTS], frame: usize, sr: f32) -> [f32; 6] {
    let hz = bounded(ins[0].at(frame), 20.0, sr * 0.4, 220.0);
    let harmonics = bounded(ins[1].at(frame), 0.0, 1.0, 0.5);
    let timbre = bounded(ins[2].at(frame), 0.0, 1.0, 0.5);
    let morph = bounded(ins[3].at(frame), 0.0, 1.0, 0.5);
    let f0 = (hz / sr).min(0.49);
    let f1 = (f0 * 2.0_f32.powf(harmonics * 4.0 - 2.0)).min(0.49);
    // Patched-trigger convention: source clock note runs from -24 to 128.
    let clock_note = timbre * 152.0 - 24.0;
    let clock_hz = 440.0 * 2.0_f32.powf((clock_note - 69.0) / 12.0);
    let clock0 = (clock_hz / sr).clamp(0.0, 1.0);
    let clock1 = (clock0 * f1 / f0).clamp(0.0, 1.0);
    let q = 0.5 * 2.0_f32.powf(morph * 10.0);
    [f0, f1, q, harmonics, clock0, clock1]
}

/// Mode zero emits the triangular LP/BP/negative-HP sweep. Mode one emits
/// two band-pass paths. Voice start resets both clocks and the three SVFs.
/// `s` stores the two four-float clocks. Two `bq` records hold three SVF
/// states and the four filter interpolation values/increments. The unused
/// `dl.off/len` fields hold the two clock increment bit patterns; `u[3]`
/// holds the event-local half-millisecond period countdown.
#[allow(clippy::too_many_lines)]
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    if out.is_empty() {
        return;
    }
    let sr = kx.sr.max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let period = (24.0 * sr / 48_000.0).round().clamp(1.0, 192.0) as u32;
    let period_f = period as f32;
    if st.u[2] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0xB01D_4A73).s;
        st.u[1] = Rng::new(kx.seed ^ 0x71CF_930D).s;
        st.u[2] = 1;
        st.s[0] = 1.0;
        st.s[3] = 0.001;
        st.s[4] = 1.0;
        st.s[7] = 0.001;
    }
    let mut first_rng = Rng::new(st.u[0]);
    let mut second_rng = Rng::new(st.u[1]);
    let mut first = Clock::from_slice(&st.s[..4]);
    let mut second = Clock::from_slice(&st.s[4..]);
    let mut main = Svf {
        band: st.bq[0].z1,
        low: st.bq[0].z2,
    };
    let mut bp0 = Svf {
        band: st.bq[1].z1,
        low: st.bq[1].z2,
    };
    let mut bp1 = Svf {
        band: st.bq[0].b0,
        low: st.bq[0].b1,
    };
    let (mut f0, mut f1, mut q, mut mode) = (st.bq[0].b2, st.bq[0].a1, st.bq[0].a2, st.bq[1].b0);
    let mut clock0_step = f32::from_bits(st.dl.off);
    let mut clock1_step = f32::from_bits(st.dl.len);
    let mut left = st.u[3];
    for (frame, sample) in out.iter_mut().enumerate() {
        if left == 0 {
            let target = targets(ins, frame, sr);
            st.bq[1].b1 = (target[0] - f0) / period_f;
            st.bq[1].b2 = (target[1] - f1) / period_f;
            st.bq[1].a1 = (target[2] - q) / period_f;
            st.bq[1].a2 = (target[3] - mode) / period_f;
            clock0_step = (target[4] - first.frequency) / period_f;
            clock1_step = (target[5] - second.frequency) / period_f;
            left = period;
        }
        f0 += st.bq[1].b1;
        f1 += st.bq[1].b2;
        q += st.bq[1].a1;
        mode += st.bq[1].a2;
        first.frequency = (first.frequency + clock0_step).clamp(0.0, 1.0);
        second.frequency = (second.frequency + clock1_step).clamp(0.0, 1.0);
        let noise0 = first.sample(&mut first_rng);
        let noise1 = second.sample(&mut second_rng);
        let gain = 1.0 / ((0.5 + q.max(0.0)) * 40.0 * f0.max(1.0e-5)).sqrt();
        let input0 = noise0 * gain;
        let input1 = noise1 * gain;
        let (lp, main_band, hp) = main.step(input0, f0, q);
        let (_, band0, _) = bp0.step(input0, f0, q);
        let (_, band1, _) = bp1.step(input1, f1, q);
        let low_weight = (1.0 - mode * 2.0).max(0.0);
        let high_weight = (1.0 - mode * 2.0).min(0.0);
        let band_weight = 1.0 - 2.0 * (mode - 0.5).abs();
        let value = if ins[4].at(frame) >= 0.5 {
            band0 + band1
        } else {
            low_weight * lp + band_weight * main_band + high_weight * hp
        };
        *sample = if value.is_finite() {
            value.clamp(-8.0, 8.0)
        } else {
            0.0
        };
        left -= 1;
    }
    first.write(&mut st.s[..4]);
    second.write(&mut st.s[4..]);
    st.bq[0].z1 = main.band;
    st.bq[0].z2 = main.low;
    st.bq[1].z1 = bp0.band;
    st.bq[1].z2 = bp0.low;
    st.bq[0].b0 = bp1.band;
    st.bq[0].b1 = bp1.low;
    st.bq[0].b2 = f0;
    st.bq[0].a1 = f1;
    st.bq[0].a2 = q;
    st.bq[1].b0 = mode;
    st.dl.off = clock0_step.to_bits();
    st.dl.len = clock1_step.to_bits();
    st.u[0] = first_rng.s;
    st.u[1] = second_rng.s;
    st.u[3] = left;
}

#[cfg(test)]
mod tests {
    use super::{Clock, Svf};
    use crate::dsp::effects::prim::Rng;

    #[test]
    fn quadratic_blep_spreads_a_held_sample_jump_into_two_frames() {
        let mut clock = Clock {
            phase: 1.0,
            held: 0.0,
            next: 0.0,
            frequency: 0.25,
        };
        let mut rng = Rng::new(19);
        let first = clock.sample(&mut rng);
        assert_ne!(first, clock.held);
        assert_ne!(clock.next, 0.0);
        let second = clock.sample(&mut rng);
        assert_ne!(first, second);
        assert!(first.is_finite() && second.is_finite());
    }

    #[test]
    fn svf_mode_endpoint_has_negative_highpass_sign() {
        let mut low = Svf {
            band: 0.0,
            low: 0.0,
        };
        let mut high = low;
        let (lp, _, _) = low.step(1.0, 0.1, 0.5);
        let (_, _, hp) = high.step(1.0, 0.1, 0.5);
        assert!(lp > 0.0 && hp > 0.0);
        assert!(-hp < 0.0);
    }
}
