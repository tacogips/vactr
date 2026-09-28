//! Plaits position 23 dual hi-hat signal stages (MIT translation).
//!
//! Square-noise/SwingVCA main and ring-mod-noise/LinearVCA auxiliary share
//! source coloration, clocked-noise, envelope and highpass stages. See
//! THIRD_PARTY_NOTICES.md for source attribution and numerical differences.

mod ring;
mod square;

use crate::dsp::effects::prim::Rng;

use super::{Inp, Kx, NodeState, MAX_PORTS};

/// Six phase floats and ten common/filter-state floats per output node.
pub const STATE_FLOATS: usize = 16;
const ENV: usize = 6;
const NOISE_CLOCK: usize = 7;
const NOISE_SAMPLE: usize = 8;
const SUSTAIN_GAIN: usize = 9;
const COLOR_BP: usize = 10;
const HP_BP: usize = 12;

#[inline]
fn unit(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

#[inline]
fn svf(input: f32, f: f32, q: f32, bp: &mut f32, lp: &mut f32) -> (f32, f32) {
    let g = (std::f32::consts::PI * f.min(0.497)).tan();
    let r = 1.0 / q.max(0.01);
    let h = 1.0 / (1.0 + r * g + g * g);
    let hp = (input - (r + g) * *bp - *lp) * h;
    let next_bp = g * hp + *bp;
    *bp = g * hp + next_bp;
    let next_lp = g * next_bp + *lp;
    *lp = g * next_bp + next_lp;
    (hp, next_bp)
}

#[inline]
fn svf_at(input: f32, f: f32, q: f32, m: &mut [f32], index: usize) -> (f32, f32) {
    let (left, right) = m.split_at_mut(index + 1);
    svf(input, f, q, &mut left[index], &mut right[0])
}

/// Both modes are preallocated at install, and `Voice::start` clears their
/// phase, filter and envelope state on each event/retrigger.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = kx.sr.max(1.0);
    let block_len = out.len();
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0x4A17_8E33).s;
        st.u[1] = 1;
        let accent = unit(ins[4].first(), 1.0);
        let decay = unit(ins[3].first(), 0.5);
        mem[ENV] = (1.5 + 0.5 * (1.0 - decay)) * (0.3 + 0.7 * accent);
    }
    let mut rng = Rng::new(st.u[0]);
    for (i, sample) in out.iter_mut().enumerate() {
        let raw_freq = ins[0].at(i);
        let freq = if raw_freq.is_finite() {
            raw_freq.clamp(20.0, sr * 0.4)
        } else {
            414.0
        };
        let harmonics = unit(ins[1].at(i), 0.5);
        let tone = unit(ins[2].at(i), 0.5);
        let decay = unit(ins[3].at(i), 0.5);
        let accent = unit(ins[4].at(i), 1.0);
        let sustain = unit(ins[6].at(i), 0.0) >= 0.5;
        let auxiliary = ins[5].at(i) >= 0.5;
        let metallic = if auxiliary {
            ring::sample(mem, freq, sr)
        } else {
            square::sample(mem, freq, sr)
        };
        let cutoff_hz = (150.0 * 2.0_f32.powf(tone * 6.0)).min(16_000.0);
        let cutoff = (cutoff_hz / sr).min(0.497);
        let resonance = if auxiliary { 1.0 } else { 3.0 + 3.0 * tone };
        let colored = svf_at(metallic, cutoff, resonance, mem, COLOR_BP).1;
        let noisiness = harmonics * harmonics;
        let noise_clock_hz = (2.0 * freq * (32.0 - 16.0 * noisiness)).min(sr * 0.5);
        mem[NOISE_CLOCK] += noise_clock_hz / sr;
        if mem[NOISE_CLOCK] >= 1.0 {
            mem[NOISE_CLOCK] -= 1.0;
            mem[NOISE_SAMPLE] = rng.bipolar() * 0.5;
        }
        let blended = colored + noisiness * (mem[NOISE_SAMPLE] - colored);
        let env_decay_48 = (1.0 - 0.003 * 2.0_f32.powf(-decay * 7.0)).clamp(0.0, 1.0);
        let cut_decay_48 = (1.0 - 0.0025 * 2.0_f32.powf(-decay * 3.0)).clamp(0.0, 1.0);
        let decay_coeff = if !auxiliary || mem[ENV] > 0.5 {
            env_decay_48
        } else {
            cut_decay_48
        };
        mem[ENV] *= decay_coeff.powf(48_000.0 / sr);
        #[allow(clippy::cast_precision_loss)]
        let remaining = (block_len - i) as f32;
        mem[SUSTAIN_GAIN] +=
            ((if i < kx.gate { accent * decay } else { 0.0 }) - mem[SUSTAIN_GAIN]) / remaining;
        let gain = if sustain { mem[SUSTAIN_GAIN] } else { mem[ENV] };
        let vca = if auxiliary {
            blended * gain
        } else {
            let asym = blended * if blended > 0.0 { 4.0 } else { 0.1 };
            (asym / (1.0 + asym.abs()) + 0.1) * gain
        };
        let high = svf_at(vca, cutoff, 0.5, mem, HP_BP).0;
        *sample = if high.is_finite() {
            high.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        st.s[1] += 1.0 / sr;
        if (!sustain && st.s[1] > 4.0) || (sustain && i >= kx.gate && st.s[1] > 0.1) {
            st.finish();
        }
    }
    st.u[0] = rng.s;
}
