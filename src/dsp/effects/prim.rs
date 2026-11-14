//! Shared DSP primitives (design 12.8.8): biquad, one-pole, delay line,
//! allpass, envelope follower, LFO, waveshapers and a seeded RNG.
//!
//! Every primitive is `Copy` POD with its state inline; delay lines index
//! into a caller-provided `mem` slice that was carved out at install time.
//! Nothing here allocates.

use std::f32::consts::{PI, TAU};

/// A transposed direct-form II biquad.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Biquad {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
    pub z1: f32,
    pub z2: f32,
}

/// The biquad responses (RBJ cookbook).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Allpass,
    Peak,
    LowShelf,
    HighShelf,
}

impl Biquad {
    /// An identity filter (passes its input).
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Sets the coefficients, keeping the state. `freq` is clamped into
    /// `(10 Hz, 0.49 sr)`, `q` to at least 0.05.
    pub fn set(&mut self, shape: Shape, freq: f32, q: f32, gain_db: f32, sr: f32) {
        let f = clampf(freq, 10.0, 0.49 * sr);
        let q = if q.is_finite() { q.max(0.05) } else { 0.707 };
        let w = TAU * f / sr;
        let (sn, cs) = w.sin_cos();
        let alpha = sn / (2.0 * q);
        let a = 10f32.powf(clampf(gain_db, -48.0, 48.0) / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::Lowpass => {
                let b = (1.0 - cs) / 2.0;
                (b, 1.0 - cs, b, 1.0 + alpha, -2.0 * cs, 1.0 - alpha)
            }
            Shape::Highpass => {
                let b = (1.0 + cs) / 2.0;
                (b, -(1.0 + cs), b, 1.0 + alpha, -2.0 * cs, 1.0 - alpha)
            }
            Shape::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
            Shape::Notch => (1.0, -2.0 * cs, 1.0, 1.0 + alpha, -2.0 * cs, 1.0 - alpha),
            Shape::Allpass => (
                1.0 - alpha,
                -2.0 * cs,
                1.0 + alpha,
                1.0 + alpha,
                -2.0 * cs,
                1.0 - alpha,
            ),
            Shape::Peak => (
                1.0 + alpha * a,
                -2.0 * cs,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cs,
                1.0 - alpha / a,
            ),
            Shape::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cs + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                    a * ((a + 1.0) - (a - 1.0) * cs - s),
                    (a + 1.0) + (a - 1.0) * cs + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                    (a + 1.0) + (a - 1.0) * cs - s,
                )
            }
            Shape::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cs + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                    a * ((a + 1.0) + (a - 1.0) * cs - s),
                    (a + 1.0) - (a - 1.0) * cs + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cs),
                    (a + 1.0) - (a - 1.0) * cs - s,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    /// Filters one sample.
    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if !self.z1.is_finite() || !self.z2.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
            return 0.0;
        }
        y
    }

    /// Filters a buffer in place.
    pub fn run_buf(&mut self, buf: &mut [f32]) {
        for x in buf {
            *x = self.run(*x);
        }
    }
}

/// A one-pole lowpass smoother.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct OnePole {
    pub z: f32,
}

impl OnePole {
    /// The coefficient of a lowpass at `freq` Hz.
    #[must_use]
    pub fn coef(freq: f32, sr: f32) -> f32 {
        let f = clampf(freq, 0.01, 0.49 * sr);
        1.0 - (-TAU * f / sr).exp()
    }

    /// The coefficient reaching ~63% in `seconds`.
    #[must_use]
    pub fn time_coef(seconds: f32, sr: f32) -> f32 {
        let s = seconds.max(1.0e-5);
        1.0 - (-1.0 / (s * sr)).exp()
    }

    /// One lowpass step with coefficient `a` (0..1).
    #[inline]
    pub fn lp(&mut self, x: f32, a: f32) -> f32 {
        self.z += a * (x - self.z);
        if !self.z.is_finite() {
            self.z = 0.0;
        }
        self.z
    }

    /// One highpass step (input minus the lowpass).
    #[inline]
    pub fn hp(&mut self, x: f32, a: f32) -> f32 {
        x - self.lp(x, a)
    }
}

/// A peak/RMS-ish envelope follower with separate attack and release.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Follower {
    pub env: f32,
}

impl Follower {
    /// Follows `|x|`; `att`/`rel` are `OnePole::time_coef` values.
    #[inline]
    pub fn run(&mut self, x: f32, att: f32, rel: f32) -> f32 {
        let a = x.abs();
        let c = if a > self.env { att } else { rel };
        self.env += c * (a - self.env);
        if !self.env.is_finite() {
            self.env = 0.0;
        }
        self.env
    }
}

/// A sine LFO.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Lfo {
    pub phase: f32,
}

impl Lfo {
    /// Advances by `rate` Hz and returns the sine in -1..1.
    #[inline]
    pub fn next(&mut self, rate: f32, sr: f32) -> f32 {
        let y = (TAU * self.phase).sin();
        self.phase += rate.max(0.0) / sr;
        self.phase -= self.phase.floor();
        y
    }

    /// Advances and returns the unipolar value 0..1.
    #[inline]
    pub fn uni(&mut self, rate: f32, sr: f32) -> f32 {
        0.5 + 0.5 * self.next(rate, sr)
    }
}

/// A circular delay line over a region of caller memory.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DelayLine {
    pub off: u32,
    pub len: u32,
    pub pos: u32,
}

impl DelayLine {
    /// Carves `want` floats out of `mem` at `*cursor` (clamped to what is
    /// left) and advances the cursor. A zero-length line reads silence.
    pub fn carve(cursor: &mut usize, mem_len: usize, want: usize) -> Self {
        let start = (*cursor).min(mem_len);
        let len = want.min(mem_len - start);
        *cursor = start + len;
        Self {
            off: u32::try_from(start).unwrap_or(u32::MAX),
            len: u32::try_from(len).unwrap_or(0),
            pos: 0,
        }
    }

    /// The capacity in samples.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.len as usize
    }

    /// Writes one sample at the head and advances it.
    #[inline]
    pub fn write(&mut self, mem: &mut [f32], x: f32) {
        if self.len == 0 {
            return;
        }
        let i = self.off as usize + self.pos as usize;
        if let Some(slot) = mem.get_mut(i) {
            *slot = if x.is_finite() { x } else { 0.0 };
        }
        self.pos = (self.pos + 1) % self.len;
    }

    /// The sample written `d` writes ago (1 = the last one), `d` clamped
    /// into the line.
    #[inline]
    #[must_use]
    pub fn tap(&self, mem: &[f32], d: usize) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let len = self.len as usize;
        let d = d.clamp(1, len);
        let i = (self.pos as usize + len - d) % len;
        mem.get(self.off as usize + i).copied().unwrap_or(0.0)
    }

    /// A fractional tap with linear interpolation (`d` in samples).
    #[inline]
    #[must_use]
    pub fn read(&self, mem: &[f32], d: f32) -> f32 {
        let d = if d.is_finite() { d.max(1.0) } else { 1.0 };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let i = d as usize;
        let frac = d - i as f32;
        let a = self.tap(mem, i);
        let b = self.tap(mem, i + 1);
        a + (b - a) * frac
    }
}

/// A Schroeder allpass over a delay line.
#[inline]
pub fn allpass(dl: &mut DelayLine, mem: &mut [f32], x: f32, d: usize, g: f32) -> f32 {
    let delayed = dl.tap(mem, d);
    let v = x + g * delayed;
    dl.write(mem, v);
    delayed - g * v
}

/// A fast `tanh` approximation, exact at 0 and saturating at +-1.
#[inline]
#[must_use]
pub fn tanh(x: f32) -> f32 {
    if !x.is_finite() {
        return if x > 0.0 {
            1.0
        } else if x < 0.0 {
            -1.0
        } else {
            0.0
        };
    }
    let x = x.clamp(-4.5, 4.5);
    let x2 = x * x;
    (x * (27.0 + x2) / (27.0 + 9.0 * x2)).clamp(-1.0, 1.0)
}

/// A cubic soft clipper, linear near 0 and bounded to +-2/3.
#[inline]
#[must_use]
pub fn soft_clip(x: f32) -> f32 {
    let x = clampf(x, -1.0, 1.0);
    x - x * x * x / 3.0
}

/// `x` clamped to `[lo, hi]`; NaN maps to `lo`.
#[inline]
#[must_use]
pub fn clampf(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_nan() {
        lo
    } else {
        x.clamp(lo, hi)
    }
}

/// Decibels to linear gain.
#[inline]
#[must_use]
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(clampf(db, -120.0, 48.0) / 20.0)
}

/// Linear gain to decibels (floored at -120 dB).
#[inline]
#[must_use]
pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.abs().max(1.0e-6).log10()
}

/// Equal-power pan gains for `pan` in 0..1.
#[inline]
#[must_use]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let pan = clampf(pan, 0.0, 1.0);
    if pan >= 1.0 {
        return (0.0, 1.0);
    }
    let p = pan * PI / 2.0;
    (p.cos(), p.sin())
}

/// A seeded xorshift32 stream.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rng {
    pub s: u32,
}

impl Default for Rng {
    fn default() -> Self {
        Self::new(0x9E37_79B9)
    }
}

impl Rng {
    /// A stream from `seed` (0 is remapped, xorshift needs a nonzero state).
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            s: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }

    /// The next raw value.
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.s;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.s = x;
        x
    }

    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn unit(&mut self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let v = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        v
    }

    /// Uniform in `[-1, 1)`.
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}
