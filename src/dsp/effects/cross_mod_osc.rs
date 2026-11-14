//! Table-free internal carrier stages adapted from MIT Warps `oscillator.cc`.
//! Each of the XMOD and vocoder oscillators owns its phase, pending quadratic
//! BLEP sample and filters. Host-rate operation replaces source 96 kHz SRC.

use std::f32::consts::TAU;

use super::super::prim::Rng;

const WORDS: usize = 7;
pub(super) const XMOD_OFFSET: usize = 10;
pub(super) const VOCODER_OFFSET: usize = XMOD_OFFSET + WORDS;

#[derive(Clone, Copy)]
pub(super) struct Oscillator {
    phase: f32,
    increment: f32,
    next: f32,
    lp: f32,
    hp: f32,
    high: bool,
    external_level: f32,
}

impl Oscillator {
    pub(super) fn load(state: &[f32; 32], offset: usize) -> Self {
        Self {
            phase: state[offset],
            increment: state[offset + 1],
            next: state[offset + 2],
            lp: state[offset + 3],
            hp: state[offset + 4],
            high: state[offset + 5] != 0.0,
            external_level: state[offset + 6],
        }
    }

    pub(super) fn save(self, state: &mut [f32; 32], offset: usize) {
        state[offset] = self.phase;
        state[offset + 1] = self.increment;
        state[offset + 2] = self.next;
        state[offset + 3] = self.lp;
        state[offset + 4] = self.hp;
        state[offset + 5] = f32::from(u8::from(self.high));
        state[offset + 6] = self.external_level;
    }

    fn next_phase(&mut self, base: f32, sr: f32) -> f32 {
        if self.increment <= 0.0 {
            self.increment = base;
        } else {
            let smooth = 1.0 - (-1.0 / (0.00075 * sr)).exp();
            self.increment += smooth * (base - self.increment);
        }
        self.increment
    }

    pub(super) fn sine(&mut self, base: f32, external: f32, sr: f32) -> f32 {
        self.phase = (self.phase + self.next_phase(base, sr)).fract();
        (TAU * (self.phase + 0.5 * external)).sin()
    }

    pub(super) fn polyblep(&mut self, kind: u8, base: f32, external: f32, sr: f32) -> (f32, f32) {
        let mut sample = self.next;
        self.next = 0.0;
        let increment = (self.next_phase(base, sr) * (1.0 + external)).clamp(1.0e-7, 0.49);
        self.phase += increment;
        let filter_scale = 96_000.0 / sr;
        if kind == 1 {
            if !self.high && self.phase >= 0.5 {
                let t = ((self.phase - 0.5) / increment).clamp(0.0, 1.0);
                sample += this_blep(t);
                self.next += next_blep(t);
                self.high = true;
            }
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                let t = (self.phase / increment).clamp(0.0, 1.0);
                sample -= this_blep(t);
                self.next -= next_blep(t);
                self.high = false;
            }
            self.next += if self.phase < 0.5 { 0.0 } else { 1.0 };
            let input = 128.0 * (sample - 0.5);
            // Increment is already normalized by the host sample rate, so
            // this source integrator coefficient needs no second rate scale.
            self.lp += (increment * 0.0625).min(1.0) * (input - self.lp);
            (self.lp, 1.0)
        } else {
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                let t = (self.phase / increment).clamp(0.0, 1.0);
                sample -= this_blep(t);
                self.next -= next_blep(t);
            }
            self.next += self.phase;
            if kind == 2 {
                let coefficient = 1.0 - 0.7f32.powf(filter_scale);
                self.lp += coefficient * (2.0 * sample - 1.0 - self.lp);
                (self.lp, 1.0)
            } else {
                let coefficient = 1.0 - 0.75f32.powf(filter_scale);
                self.lp += coefficient * (self.hp - sample - self.lp);
                self.hp = sample;
                let gain = (0.025 / (0.0002 + self.increment)).min(32.0);
                (4.0 * self.lp, gain)
            }
        }
    }

    pub(super) fn noise(&mut self, base: f32, external: f32, sr: f32, rng: &mut Rng) -> f32 {
        let error = external * external - self.external_level;
        let source_rate = 96_000.0 / sr;
        let coefficient = if error > 0.0 {
            1.0 - 0.99f32.powf(source_rate)
        } else {
            1.0 - 0.9999f32.powf(source_rate)
        };
        self.external_level += coefficient * error;
        let gain = (1.0 - 32.0 * self.external_level).max(0.0);
        let ducked = external + gain * (rng.bipolar() - external);
        // Original, bounded one-pole in place of source SVF and random stream.
        let cutoff = (4.0 * base).min(0.45);
        let a = 1.0 - (-TAU * cutoff).exp();
        self.lp += a * (ducked - self.lp);
        self.lp
    }
}

pub(super) fn this_blep(t: f32) -> f32 {
    0.5 * t * t
}

pub(super) fn next_blep(t: f32) -> f32 {
    -0.5 * (1.0 - t) * (1.0 - t)
}

#[cfg(test)]
mod tests {
    use super::{next_blep, this_blep, Oscillator};

    fn fresh() -> Oscillator {
        Oscillator::load(&[0.0; 32], 0)
    }

    #[test]
    fn quadratic_blep_and_pending_sample_smooth_saw_wrap() {
        assert_eq!(this_blep(0.0), 0.0);
        assert_eq!(this_blep(1.0), 0.5);
        assert_eq!(next_blep(0.0), -0.5);
        assert_eq!(next_blep(1.0), 0.0);
        let mut saw = fresh();
        let mut previous = 0.0;
        let mut max_jump: f32 = 0.0;
        for frame in 0..256 {
            let (sample, gain) = saw.polyblep(2, 0.125, 0.0, 48_000.0);
            assert_eq!(gain, 1.0);
            assert!(sample.is_finite());
            if frame > 16 {
                max_jump = max_jump.max((sample - previous).abs());
            }
            previous = sample;
        }
        assert!(
            max_jump < 1.0,
            "BLEP/filter avoid a raw two-unit wrap: {max_jump}"
        );
        assert!(saw.next.is_finite());
    }

    #[test]
    fn sine_phase_modulation_triangle_and_pulse_have_distinct_states() {
        let mut plain = fresh();
        let mut modulated = fresh();
        let plain_sample = plain.sine(0.1, 0.0, 48_000.0);
        let modulated_sample = modulated.sine(0.1, 0.5, 48_000.0);
        assert!((plain_sample - modulated_sample).abs() > 0.1);
        let mut triangle = fresh();
        let mut pulse = fresh();
        let mut difference = 0.0;
        for _ in 0..256 {
            let tri = triangle.polyblep(1, 0.07, 0.0, 48_000.0).0;
            let (pul, gain) = pulse.polyblep(3, 0.07, 0.0, 48_000.0);
            assert!(tri.is_finite() && pul.is_finite() && gain.is_finite());
            difference += (tri - pul).abs();
        }
        assert!(difference > 1.0);
        assert!(triangle.high != pulse.high || triangle.lp != pulse.lp);
        let (_, high_gain) = pulse.polyblep(3, 0.2, 0.0, 48_000.0);
        assert!(high_gain < 32.0 && high_gain > 0.0);
    }

    #[test]
    fn fm_increment_has_nonzero_floor_and_noise_ducks_with_external_signal() {
        let mut low = fresh();
        for _ in 0..32 {
            let (sample, _) = low.polyblep(2, 0.2, -2.0, 48_000.0);
            assert!(sample.is_finite());
        }
        assert!(low.phase > 0.0 && low.phase < 0.001);
        let mut free = fresh();
        let mut ducked = fresh();
        let mut free_rng = super::Rng::new(123);
        let mut duck_rng = super::Rng::new(123);
        let mut free_energy = 0.0;
        let mut ducked_energy = 0.0;
        for _ in 0..4096 {
            free_energy += free.noise(0.01, 0.0, 48_000.0, &mut free_rng).abs();
            let sample = ducked.noise(0.01, 0.5, 48_000.0, &mut duck_rng);
            ducked_energy += (sample - 0.5).abs();
        }
        assert!(ducked_energy < free_energy * 0.5);
    }
}
