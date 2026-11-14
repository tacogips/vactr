//! Fixed vocoder output limiter adapted from Emilie Gillet's MIT Warps
//! `warps/dsp/limiter.h` (2015) and `stmlib/dsp/dsp.h`.
//! The full MIT notice is preserved in THIRD_PARTY_NOTICES.md.

pub(super) const PEAK_SLOT: usize = 25;
pub(super) const INITIAL_PEAK: f32 = 0.5;

pub(super) struct Limiter {
    pub(super) peak: f32,
    attack: f32,
    release: f32,
}

impl Limiter {
    pub(super) fn new(peak: f32, sr: f32) -> Self {
        // Source coefficients apply once per frame at 96 kHz. Host scaling
        // preserves their approximate time constants at other rates.
        let source_frames = 96_000.0 / sr.max(1.0);
        Self {
            peak,
            attack: 1.0 - 0.95f32.powf(source_frames),
            release: 1.0 - 0.99998f32.powf(source_frames),
        }
    }

    pub(super) fn process(&mut self, input: f32) -> f32 {
        let sample = if input.is_finite() {
            input.clamp(-1_000.0, 1_000.0) * 1.4
        } else {
            0.0
        };
        let error = sample.abs() - self.peak;
        self.peak += if error > 0.0 {
            self.attack * error
        } else {
            self.release * error
        };
        let gain = if self.peak <= 1.0 {
            1.0
        } else {
            self.peak.recip()
        };
        let limited = sample * gain * 0.8;
        let square = limited * limited;
        limited * (27.0 + square) / (27.0 + 9.0 * square)
    }
}

#[cfg(test)]
mod tests {
    use super::{Limiter, INITIAL_PEAK};

    #[test]
    fn source_pregain_slope_normalization_and_softlimit() {
        let mut limiter = Limiter::new(INITIAL_PEAK, 96_000.0);
        let input = 0.5;
        let pre = input * 1.4;
        let expected_peak = 0.5 + 0.05 * (pre - 0.5);
        let scaled = pre * 0.8;
        let expected = scaled * (27.0 + scaled * scaled) / (27.0 + 9.0 * scaled * scaled);
        let actual = limiter.process(input);
        assert!((limiter.peak - expected_peak).abs() < 1.0e-6);
        assert!((actual - expected).abs() < 1.0e-6);

        for _ in 0..256 {
            assert!(limiter.process(1.0).is_finite());
        }
        assert!(limiter.peak > 1.0);
        let peak_before_decay = limiter.peak;
        let expected_decay = peak_before_decay + 0.00002 * (0.0 - peak_before_decay);
        let quiet = limiter.process(0.0);
        assert_eq!(quiet, 0.0);
        assert!((limiter.peak - expected_decay).abs() < 1.0e-6);
        assert!(limiter.process(f32::INFINITY).is_finite());
    }

    #[test]
    fn overload_is_bounded_and_rate_scaled_release_is_consistent() {
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            let mut limiter = Limiter::new(INITIAL_PEAK, sr);
            let frames = sr as usize / 20;
            for _ in 0..frames {
                let output = limiter.process(1.0);
                assert!(output.is_finite() && output.abs() < 1.0);
            }
            assert!(limiter.peak > 1.0);
            let overload_peak = limiter.peak;
            for _ in 0..frames {
                limiter.process(0.0);
            }
            assert!(limiter.peak < overload_peak * 0.92);
            assert!(limiter.peak > 1.0, "source release is deliberately slow");
        }
    }
}
