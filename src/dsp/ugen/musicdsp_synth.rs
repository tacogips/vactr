//! Original mathematical synthesis inspired by the audited MusicDSP entries.
//! All oscillators exclude fixed partials at/above Nyquist. No source snippets,
//! vowel data, random-number libraries or tables are imported.
use super::{Inp, Kx, NodeState, MAX_PORTS};
use crate::dsp::effects::prim::Rng;
use std::f32::consts::TAU;

mod chaos;
pub use chaos::{lorenz, rossler};

fn bounded(x: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        default.clamp(lo, hi)
    }
}
fn frequency(x: f32, sr: f32) -> f32 {
    bounded(x, 110.0, 0.0, 0.49 * sr.max(100.0))
}
fn advance(phase: f32, increment: f32) -> f32 {
    (phase + increment).rem_euclid(1.0)
}

/// Normalized finite geometric sine sum. The complex geometric series gives
/// constant work, with a bounded explicit sum near its removable singularity.
pub(crate) fn dsf_value(base: f32, spacing: f32, rolloff: f32, count: usize) -> f32 {
    let n = count.clamp(1, 128);
    let a = rolloff.clamp(0.0, 1.0);
    let (sin, cos) = spacing.sin_cos();
    let den = (1.0 - a * cos).powi(2) + (a * sin).powi(2);
    let norm = if a == 1.0 {
        n as f32
    } else {
        (1.0 - a.powi(n as i32)) / (1.0 - a)
    };
    if den < 1e-6 || (1.0 - a).abs() < 1e-5 {
        let mut weight = 1.0;
        let mut sum = 0.0;
        for j in 0..n {
            sum += weight * (base + j as f32 * spacing).sin();
            weight *= a;
        }
        return sum / norm;
    }
    let power = a.powi(n as i32);
    let (sn, cn) = (n as f32 * spacing).sin_cos();
    let (nr, ni) = (1.0 - power * cn, -power * sn);
    let (dr, di) = (1.0 - a * cos, -a * sin);
    let real = (nr * dr + ni * di) / den;
    let imag = (ni * dr - nr * di) / den;
    let (sb, cb) = base.sin_cos();
    (sb * real + cb * imag) / norm
}

/// `dsf-osc freq spacing rolloff count`: independent geometric partial spacing.
pub fn dsf(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let sr = kx.sr.max(100.0);
    for (i, y) in out.iter_mut().enumerate() {
        let f = frequency(ins[0].at(i), sr);
        let spacing = bounded(ins[1].at(i), 1.0, 0.1, 8.0);
        let rolloff = bounded(ins[2].at(i), 0.7, 0.0, 1.0);
        let requested = bounded(ins[3].at(i), 32.0, 1.0, 128.0).round() as usize;
        let allowed = if f > 0.0 {
            ((0.5 * sr / f - 1.0) / spacing).ceil().max(1.0) as usize
        } else {
            128
        };
        *y = dsf_value(
            TAU * st.s[0],
            TAU * st.s[1],
            rolloff,
            requested.min(allowed),
        );
        st.s[0] = advance(st.s[0], f / sr);
        st.s[1] = advance(st.s[1], f * spacing / sr);
    }
}

/// `chebyshev-osc freq harmonic-1..8`: signed, independent harmonics.
pub fn chebyshev(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let sr = kx.sr.max(100.0);
    for (i, y) in out.iter_mut().enumerate() {
        let freq = frequency(ins[0].at(i), sr);
        let x = (TAU * st.s[0]).cos();
        let (mut prev, mut current) = (1.0, x);
        let (mut sum, mut norm) = (0.0, 0.0);
        for (h, input) in ins.iter().enumerate().take(9).skip(1) {
            let weight = bounded(input.at(i), if h == 1 { 1.0 } else { 0.0 }, -1.0, 1.0);
            if freq * (h as f32) < 0.5 * sr {
                sum += weight * current;
                norm += weight.abs();
            }
            let next = 2.0 * x * current - prev;
            prev = current;
            current = next;
        }
        *y = if norm > 1e-12 { sum / norm } else { 0.0 };
        st.s[0] = advance(st.s[0], freq / sr);
    }
}

/// `gaussian-noise sigma mean`: deterministic Box-Muller pairs, no clipping.
pub fn gaussian(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    if st.u[0] == 0 {
        st.u[0] = Rng::new(kx.seed).s;
    }
    let mut rng = Rng::new(st.u[0]);
    for (i, y) in out.iter_mut().enumerate() {
        let standard = if st.u[1] != 0 {
            st.u[1] = 0;
            st.s[0]
        } else {
            let u = rng.unit().max(1.0 / 16_777_216.0);
            let angle = TAU * rng.unit();
            let radius = (-2.0 * u.ln()).sqrt();
            let (sin, cos) = angle.sin_cos();
            st.s[0] = radius * sin;
            st.u[1] = 1;
            radius * cos
        };
        *y = bounded(ins[1].at(i), 0.0, -1.0, 1.0)
            + bounded(ins[0].at(i), 0.25, 0.0, 1.0) * standard;
    }
    st.u[0] = rng.s;
}

/// Two 36-float coefficient caches, allocated at voice setup.
pub const AM_STATE_FLOATS: usize = 72;

// A finite cosine kernel AM-multiplied by adjacent harmonic carriers.
// Expansion of cos(k*p)cos(n*p) lets us drop out-of-band sidebands explicitly.
fn formant(phase: f32, freq: f32, center: f32, bandwidth: f32, sr: f32, cache: &mut [f32]) -> f32 {
    if cache[0] != freq || cache[1] != bandwidth {
        cache[0] = freq;
        cache[1] = bandwidth;
        let mut norm = 1.0;
        cache[3] = 1.0;
        for k in 1..=32 {
            let weight = (-0.5 * (k as f32 * freq / bandwidth).powi(2)).exp();
            cache[3 + k] = weight;
            norm += 2.0 * weight;
        }
        cache[2] = norm;
    }
    let target = center / freq;
    let lower = target.floor() as i32;
    let blend = target - lower as f32;
    let (sinphase, cosphase) = phase.sin_cos();
    let (sn, cn) = (lower as f32 * phase).sin_cos();
    let carriers = [
        (lower, cn, sn, 1.0 - blend),
        (
            lower + 1,
            cn * cosphase - sn * sinphase,
            sn * cosphase + cn * sinphase,
            blend,
        ),
    ];
    let mut out = 0.0;
    for (carrier, coscarrier, sincarrier, weight) in carriers {
        if carrier > 0 && (carrier as f32) * freq < 0.5 * sr {
            out += weight * coscarrier;
        }
        let (mut cosk, mut sink) = (cosphase, sinphase);
        for k in 1..=32 {
            let coefficient = weight * cache[3 + k];
            if ((carrier + k as i32) as f32) * freq < 0.5 * sr {
                out += coefficient * (coscarrier * cosk - sincarrier * sink);
            }
            let lower_sideband = carrier - k as i32;
            if lower_sideband != 0 && (lower_sideband.unsigned_abs() as f32) * freq < 0.5 * sr {
                out += coefficient * (coscarrier * cosk + sincarrier * sink);
            }
            let nextcos = cosk * cosphase - sink * sinphase;
            sink = sink * cosphase + cosk * sinphase;
            cosk = nextcos;
        }
    }
    out / cache[2]
}

/// Two independently moving formants with harmonic double-carrier AM.
/// The kernel has at most 32 envelope harmonics per carrier, bounded work.
pub fn am_formant(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < AM_STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(100.0);
    for (i, y) in out.iter_mut().enumerate() {
        let freq = frequency(ins[0].at(i), sr).max(1.0);
        let balance = bounded(ins[5].at(i), 0.5, 0.0, 1.0);
        let phase = TAU * st.s[0];
        let first = formant(
            phase,
            freq,
            bounded(ins[1].at(i), 700.0, 20.0, 0.49 * sr),
            bounded(ins[3].at(i), 100.0, 10.0, 8000.0),
            sr,
            &mut mem[..36],
        );
        let second = formant(
            phase,
            freq,
            bounded(ins[2].at(i), 1200.0, 20.0, 0.49 * sr),
            bounded(ins[4].at(i), 150.0, 10.0, 8000.0),
            sr,
            &mut mem[36..72],
        );
        *y = (1.0 - balance) * first + balance * second;
        st.s[0] = advance(st.s[0], freq / sr);
    }
}
