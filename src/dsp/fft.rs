//! A fixed-size radix-2 FFT with preallocated twiddles (design 12.8.8).
//!
//! `Fft::new` allocates the twiddle and bit-reversal tables once; `forward`
//! transforms caller buffers in place and never allocates.

use std::f32::consts::TAU;

/// The analysis FFT size used by analyzers and spectral approximations.
pub const FFT_SIZE: usize = 512;

/// An in-place complex FFT of one power-of-two size.
#[derive(Clone, Debug)]
pub struct Fft {
    n: usize,
    cos: Box<[f32]>,
    sin: Box<[f32]>,
    rev: Box<[u32]>,
}

impl Fft {
    /// An FFT of `n` points (rounded up to a power of two, at least 2).
    #[must_use]
    pub fn new(n: usize) -> Self {
        let n = n.max(2).next_power_of_two();
        let bits = n.trailing_zeros();
        #[allow(clippy::cast_precision_loss)]
        let (cos, sin): (Vec<f32>, Vec<f32>) = (0..n / 2)
            .map(|k| {
                let w = -TAU * k as f32 / n as f32;
                (w.cos(), w.sin())
            })
            .unzip();
        let rev = (0..n)
            .map(|i| {
                let r = u32::try_from(i).unwrap_or(0).reverse_bits();
                r >> (32 - bits)
            })
            .collect();
        Self {
            n,
            cos: cos.into_boxed_slice(),
            sin: sin.into_boxed_slice(),
            rev,
        }
    }

    /// The transform size.
    #[must_use]
    pub fn len(&self) -> usize {
        self.n
    }

    /// Always false: an FFT has at least 2 points.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The forward transform of `(re, im)` in place. Buffers shorter than
    /// the size are left untouched.
    pub fn forward(&self, re: &mut [f32], im: &mut [f32]) {
        let n = self.n;
        if re.len() < n || im.len() < n {
            return;
        }
        for i in 0..n {
            let j = self.rev[i] as usize;
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut size = 2;
        while size <= n {
            let half = size / 2;
            let step = n / size;
            let mut start = 0;
            while start < n {
                for k in 0..half {
                    let (wr, wi) = (self.cos[k * step], self.sin[k * step]);
                    let a = start + k;
                    let b = a + half;
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
                start += size;
            }
            size *= 2;
        }
    }

    /// The magnitude spectrum of `input` (Hann-windowed) into `mags`
    /// (`n / 2` bins), using `re`/`im` as scratch of `n` floats each.
    pub fn magnitudes(&self, input: &[f32], re: &mut [f32], im: &mut [f32], mags: &mut [f32]) {
        let n = self.n;
        if re.len() < n || im.len() < n {
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        for i in 0..n {
            let w = 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos();
            re[i] = input.get(i).copied().unwrap_or(0.0) * w;
            im[i] = 0.0;
        }
        self.forward(re, im);
        #[allow(clippy::cast_precision_loss)]
        let scale = 2.0 / n as f32;
        for (k, m) in mags.iter_mut().enumerate().take(n / 2) {
            *m = (re[k] * re[k] + im[k] * im[k]).sqrt() * scale;
        }
    }

    /// The peak magnitude of 8 octave bands (bins `2^b..2^(b+1)`) of
    /// `input`, using `scratch` (at least `2.5 * n` floats).
    pub fn octave_bands(&self, input: &[f32], scratch: &mut [f32]) -> [f32; 8] {
        let n = self.n;
        let mut bands = [0.0; 8];
        if scratch.len() < 2 * n + n / 2 {
            return bands;
        }
        let (re, rest) = scratch.split_at_mut(n);
        let (im, mags) = rest.split_at_mut(n);
        let mags = &mut mags[..n / 2];
        self.magnitudes(input, re, im, mags);
        for (b, band) in bands.iter_mut().enumerate() {
            let (lo, hi) = ((1usize << b).min(n / 2), (2usize << b).min(n / 2));
            *band = mags[lo..hi].iter().copied().fold(0.0, f32::max);
        }
        bands
    }
}
