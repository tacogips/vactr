//! Lo-fi and media-degradation effects (design 12.5, 12.8.8).
//!
//! Every kind is a bounded, allocation-free algorithm over the shared
//! primitives in `prim`. Kinds that need a short modulated delay line
//! (`jitter`, `tape`, `cassette`, `codec`'s pre-echo smear) carve one or two
//! `prim::DelayLine`s out of `mem` at install time; a zero-length line
//! (short or empty `mem`) simply reads silence, so every kind stays valid
//! however little delay memory the caller grants.

use super::prim::{clampf, db_to_gain, soft_clip, tanh, DelayLine, OnePole, Shape};
use super::{FxCtx, FxState, ParamDef};
use crate::dsp::graph::EffectKind;

/// Reads parameter `i`, falling back to `default` if the caller passed a
/// shorter slice than `params(kind)` (never indexes out of bounds).
#[inline]
fn pv(p: &[f32], i: usize, default: f32) -> f32 {
    p.get(i).copied().unwrap_or(default)
}

const BITCRUSH: &[ParamDef] = &[
    ParamDef::new("bits", 8.0, 1.0, 16.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DECIMATE: &[ParamDef] = &[
    ParamDef::unit("rate", 8000.0, 200.0, 20_000.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const JITTER: &[ParamDef] = &[
    ParamDef::new("amount", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const NOISE_BLEND: &[ParamDef] = &[
    ParamDef::unit("level", -24.0, -60.0, 0.0, "dB"),
    ParamDef::new("color", 0.5, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const HUM: &[ParamDef] = &[
    ParamDef::unit("freq", 50.0, 40.0, 70.0, "Hz"),
    ParamDef::unit("level", -30.0, -60.0, 0.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const TAPE: &[ParamDef] = &[
    ParamDef::new("drive", 0.3, 0.0, 1.0),
    ParamDef::new("wow", 0.15, 0.0, 1.0),
    ParamDef::new("flutter", 0.1, 0.0, 1.0),
    ParamDef::unit("hiss", -40.0, -80.0, -10.0, "dB"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const CASSETTE: &[ParamDef] = &[
    ParamDef::new("drive", 0.3, 0.0, 1.0),
    ParamDef::new("wow", 0.15, 0.0, 1.0),
    ParamDef::new("flutter", 0.1, 0.0, 1.0),
    ParamDef::unit("hiss", -40.0, -80.0, -10.0, "dB"),
    ParamDef::new("age", 0.2, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const VINYL: &[ParamDef] = &[
    ParamDef::new("crackle", 0.2, 0.0, 1.0),
    ParamDef::unit("noise", -30.0, -60.0, 0.0, "dB"),
    ParamDef::new("wear", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const VINYL_ARTIFACTS: &[ParamDef] = &[
    ParamDef::new("pops", 0.1, 0.0, 1.0),
    ParamDef::new("crackle", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const CODEC: &[ParamDef] = &[
    ParamDef::new("kind", 0.0, 0.0, 4.0),
    ParamDef::new("quality", 0.5, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const RADIO: &[ParamDef] = &[
    ParamDef::new("kind", 0.0, 0.0, 2.0),
    ParamDef::new("tuning", 0.2, 0.0, 1.0),
    ParamDef::new("noise", 0.15, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const TV_AUDIO: &[ParamDef] = &[
    ParamDef::new("amount", 0.4, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DIGITAL_ERROR: &[ParamDef] = &[
    ParamDef::unit("rate", 2.0, 0.0, 50.0, "Hz"),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

const DSD_IMD: &[ParamDef] = &[
    ParamDef::new("amount", 0.3, 0.0, 1.0),
    ParamDef::new("mix", 1.0, 0.0, 1.0),
];

/// The named parameters of a lo-fi kind.
pub(super) fn params(kind: EffectKind) -> &'static [ParamDef] {
    use EffectKind as K;
    match kind {
        K::Bitcrush => BITCRUSH,
        K::Decimate => DECIMATE,
        K::Jitter => JITTER,
        K::NoiseBlend => NOISE_BLEND,
        K::Hum => HUM,
        K::Tape => TAPE,
        K::Cassette => CASSETTE,
        K::Vinyl => VINYL,
        K::VinylArtifacts => VINYL_ARTIFACTS,
        K::Codec => CODEC,
        K::Radio => RADIO,
        K::TvAudio => TV_AUDIO,
        K::DigitalError => DIGITAL_ERROR,
        K::DsdImd => DSD_IMD,
        _ => &[],
    }
}

/// A short modulated delay's capacity in frames, at least one sample.
fn frames_ms(ms: f32, sr: f32) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (ms * 0.001 * sr).ceil() as usize;
    n.max(1)
}

/// Wow/flutter and jitter delay lines stay short: a few ms of warble or
/// timing noise is enough, and it keeps the memory footprint tiny.
const JITTER_MS: f32 = 12.0;
const TAPE_MS: f32 = 30.0;
const CODEC_MS: f32 = 12.0;

/// The preferred delay memory of a lo-fi kind, in floats (two channels).
pub(super) fn mem_len(kind: EffectKind, sr: f32) -> usize {
    use EffectKind as K;
    match kind {
        K::Jitter => 2 * frames_ms(JITTER_MS, sr),
        K::Tape | K::Cassette => 2 * frames_ms(TAPE_MS, sr),
        K::Codec => 2 * frames_ms(CODEC_MS, sr),
        _ => 0,
    }
}

/// Carves the stereo delay line(s) a kind needs; a no-op for kinds that
/// only use scalar state.
pub(super) fn init(kind: EffectKind, st: &mut FxState, mem_len: usize, sr: f32) {
    use EffectKind as K;
    let want = match kind {
        K::Jitter => frames_ms(JITTER_MS, sr),
        K::Tape | K::Cassette => frames_ms(TAPE_MS, sr),
        K::Codec => frames_ms(CODEC_MS, sr),
        _ => return,
    };
    let mut cursor = 0usize;
    st.dl[0] = DelayLine::carve(&mut cursor, mem_len, want);
    st.dl[1] = DelayLine::carve(&mut cursor, mem_len, want);
}

/// Processes one stereo block in place, fully wet.
#[allow(clippy::too_many_arguments)]
pub(super) fn process(
    kind: EffectKind,
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    use EffectKind as K;
    let sr = ctx.sr;
    match kind {
        K::Bitcrush => bitcrush(p, l, r),
        K::Decimate => decimate(p, st, l, r, sr),
        K::Jitter => jitter(p, st, mem, l, r),
        K::NoiseBlend => noise_blend(p, st, l, r, sr),
        K::Hum => hum(p, st, l, r, sr),
        K::Tape => tape(p, st, mem, l, r, sr),
        K::Cassette => cassette(p, st, mem, l, r, sr),
        K::Vinyl => vinyl(p, st, l, r, sr),
        K::VinylArtifacts => vinyl_artifacts(p, st, l, r),
        K::Codec => codec(p, st, mem, l, r, sr),
        K::Radio => radio(p, st, l, r, sr),
        K::TvAudio => tv_audio(p, st, l, r, sr),
        K::DigitalError => digital_error(p, st, l, r, sr),
        K::DsdImd => dsd_imd(p, st, l, r, sr),
        _ => {}
    }
}

/// Quantizes to `bits` of amplitude resolution.
fn bitcrush(p: &[f32], l: &mut [f32], r: &mut [f32]) {
    let bits = pv(p, 0, 8.0).clamp(1.0, 16.0);
    let step = 2f32.powf(bits - 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        l[i] = clampf((l[i] * step).round() / step, -16.0, 16.0);
        r[i] = clampf((r[i] * step).round() / step, -16.0, 16.0);
    }
}

/// Sample-and-holds at a reduced effective rate.
fn decimate(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 8000.0).max(50.0);
    let step = (sr / rate).max(1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        st.s[0] += 1.0;
        if st.s[0] >= step {
            st.s[1] = l[i];
            st.s[0] -= step;
        }
        l[i] = clampf(st.s[1], -16.0, 16.0);
        st.s[2] += 1.0;
        if st.s[2] >= step {
            st.s[3] = r[i];
            st.s[2] -= step;
        }
        r[i] = clampf(st.s[3], -16.0, 16.0);
    }
}

/// Random sample-timing jitter: writes into a short delay line and reads
/// back at a randomized offset.
fn jitter(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32]) {
    let amount = pv(p, 0, 0.3).clamp(0.0, 1.0);
    #[allow(clippy::cast_precision_loss)]
    let max_delay = ((st.dl[0].capacity().min(st.dl[1].capacity())).max(2) as f32 - 1.0).max(1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        st.dl[0].write(mem, l[i]);
        let d = 1.0 + st.rng.unit() * amount * max_delay;
        l[i] = clampf(st.dl[0].read(mem, d), -16.0, 16.0);

        st.dl[1].write(mem, r[i]);
        let d = 1.0 + st.rng.unit() * amount * max_delay;
        r[i] = clampf(st.dl[1].read(mem, d), -16.0, 16.0);
    }
}

/// Adds noise, color-filtered between white (`color` 0) and dull (`color`
/// 1) with a one-pole lowpass.
fn noise_blend(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let gain = db_to_gain(pv(p, 0, -24.0));
    let color = pv(p, 1, 0.5).clamp(0.0, 1.0);
    let freq = 20_000.0 - color * 19_500.0;
    let a = OnePole::coef(freq, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let f = st.op[0].lp(st.rng.bipolar(), a);
        l[i] = clampf(l[i] + f * gain, -16.0, 16.0);
        let f = st.op[1].lp(st.rng.bipolar(), a);
        r[i] = clampf(r[i] + f * gain, -16.0, 16.0);
    }
}

/// Adds a mains-hum sine tone (mono, correlated across channels).
fn hum(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let freq = pv(p, 0, 50.0).clamp(40.0, 70.0);
    let gain = db_to_gain(pv(p, 1, -30.0));
    let n = l.len().min(r.len());
    for i in 0..n {
        let h = st.lfo[0].next(freq, sr) * gain;
        l[i] = clampf(l[i] + h, -16.0, 16.0);
        r[i] = clampf(r[i] + h, -16.0, 16.0);
    }
}

/// Tape wow/flutter (a modulated delay), saturation and hiss.
fn tape(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let drive_g = 1.0 + pv(p, 0, 0.3).clamp(0.0, 1.0) * 6.0;
    let wow = pv(p, 1, 0.15).clamp(0.0, 1.0);
    let flutter = pv(p, 2, 0.1).clamp(0.0, 1.0);
    let hiss_g = db_to_gain(pv(p, 3, -40.0));
    #[allow(clippy::cast_precision_loss)]
    let cap = st.dl[0].capacity().min(st.dl[1].capacity()).max(2) as f32;
    let base = (cap * 0.4).max(1.0);
    let depth = cap * 0.3;
    let n = l.len().min(r.len());
    for i in 0..n {
        let w = st.lfo[0].next(0.6, sr) * wow;
        let f = st.lfo[1].next(7.0, sr) * flutter;
        let d = (base + (w + f) * depth).clamp(1.0, (cap - 1.0).max(1.0));

        st.dl[0].write(mem, l[i]);
        let sat = tanh(st.dl[0].read(mem, d) * drive_g);
        l[i] = clampf(sat + st.rng.bipolar() * hiss_g, -16.0, 16.0);

        st.dl[1].write(mem, r[i]);
        let sat = tanh(st.dl[1].read(mem, d) * drive_g);
        r[i] = clampf(sat + st.rng.bipolar() * hiss_g, -16.0, 16.0);
    }
}

/// Tape processing plus `age`: extra hiss, deeper wow and a dulled top end.
fn cassette(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    let drive_g = 1.0 + pv(p, 0, 0.3).clamp(0.0, 1.0) * 6.0;
    let wow = pv(p, 1, 0.15).clamp(0.0, 1.0);
    let flutter = pv(p, 2, 0.1).clamp(0.0, 1.0);
    let hiss_g = db_to_gain(pv(p, 3, -40.0));
    let age = pv(p, 4, 0.2).clamp(0.0, 1.0);
    let lp_freq = 16_000.0 - age * 10_000.0;
    st.bq[0].set(Shape::Lowpass, lp_freq, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, lp_freq, 0.707, 0.0, sr);
    #[allow(clippy::cast_precision_loss)]
    let cap = st.dl[0].capacity().min(st.dl[1].capacity()).max(2) as f32;
    let base = (cap * 0.4).max(1.0);
    let depth = cap * 0.3 * (1.0 + age);
    let extra_hiss = hiss_g * (1.0 + age * 2.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        let w = st.lfo[0].next(0.6, sr) * wow * (1.0 + age * 0.5);
        let f = st.lfo[1].next(7.0, sr) * flutter;
        let d = (base + (w + f) * depth).clamp(1.0, (cap - 1.0).max(1.0));

        st.dl[0].write(mem, l[i]);
        let sat = tanh(st.dl[0].read(mem, d) * drive_g);
        let sat = st.bq[0].run(sat);
        l[i] = clampf(sat + st.rng.bipolar() * extra_hiss, -16.0, 16.0);

        st.dl[1].write(mem, r[i]);
        let sat = tanh(st.dl[1].read(mem, d) * drive_g);
        let sat = st.bq[1].run(sat);
        r[i] = clampf(sat + st.rng.bipolar() * extra_hiss, -16.0, 16.0);
    }
}

/// Surface noise, crackle impulses and high-frequency wear roll-off.
fn vinyl(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let crackle = pv(p, 0, 0.2).clamp(0.0, 1.0);
    let noise_g = db_to_gain(pv(p, 1, -30.0));
    let wear = pv(p, 2, 0.3).clamp(0.0, 1.0);
    let freq = 20_000.0 - wear * 15_000.0;
    st.bq[0].set(Shape::Lowpass, freq, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, freq, 0.707, 0.0, sr);
    let prob = crackle * 0.003;
    let n = l.len().min(r.len());
    for i in 0..n {
        st.s[0] *= 0.75;
        if st.rng.unit() < prob {
            st.s[0] = st.rng.bipolar() * 1.5;
        }
        let noise = st.rng.bipolar() * noise_g;
        let cl = st.bq[0].run(l[i]);
        l[i] = clampf(cl * (1.0 - wear * 0.1) + noise + st.s[0] * 0.5, -16.0, 16.0);
        let cr = st.bq[1].run(r[i]);
        r[i] = clampf(cr * (1.0 - wear * 0.1) + noise + st.s[0] * 0.5, -16.0, 16.0);
    }
}

/// Isolated pop and crackle impulses without the surface-noise bed.
fn vinyl_artifacts(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32]) {
    let pops = pv(p, 0, 0.1).clamp(0.0, 1.0);
    let crackle = pv(p, 1, 0.3).clamp(0.0, 1.0);
    let pop_prob = pops * 0.0006;
    let crackle_prob = crackle * 0.004;
    let n = l.len().min(r.len());
    for i in 0..n {
        st.s[0] *= 0.85;
        if st.rng.unit() < pop_prob {
            st.s[0] = st.rng.bipolar() * 3.0;
        }
        st.s[1] *= 0.6;
        if st.rng.unit() < crackle_prob {
            st.s[1] = st.rng.bipolar() * 0.8;
        }
        let add = st.s[0] + st.s[1];
        l[i] = clampf(l[i] + add, -16.0, 16.0);
        r[i] = clampf(r[i] + add, -16.0, 16.0);
    }
}

/// codec: approximation — band-limit + bit-depth/sample-rate reduction +
/// pre-echo smear per `kind` (`:mp3 :gsm :sbc :atrac :g726`); no transform
/// codec (MDCT/subband/ADPCM) is implemented.
fn codec(p: &[f32], st: &mut FxState, mem: &mut [f32], l: &mut [f32], r: &mut [f32], sr: f32) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let idx = (pv(p, 0, 0.0).max(0.0) as usize).min(4);
    let quality = pv(p, 1, 0.5).clamp(0.0, 1.0);
    const BASE_FREQ: [f32; 5] = [16_000.0, 3400.0, 8000.0, 15_000.0, 3400.0];
    const BASE_RATE: [f32; 5] = [44_100.0, 8000.0, 16_000.0, 44_100.0, 8000.0];
    let freq = (BASE_FREQ[idx] * (0.4 + 0.6 * quality)).clamp(300.0, (0.49 * sr).max(300.0));
    let bits = (4.0 + quality * 8.0).clamp(3.0, 14.0);
    let step_bits = 2f32.powf(bits - 1.0);
    let hold_step = (sr / (BASE_RATE[idx] * (0.5 + 0.5 * quality)).max(50.0)).max(1.0);
    let smear = (1.0 - quality) * 0.4;
    st.bq[0].set(Shape::Lowpass, freq, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, freq, 0.707, 0.0, sr);
    #[allow(clippy::cast_precision_loss)]
    let cap = st.dl[0].capacity().min(st.dl[1].capacity()).max(2) as f32;
    let delay_d = (cap * 0.5).clamp(1.0, (cap - 1.0).max(1.0));
    let n = l.len().min(r.len());
    for i in 0..n {
        let lp = st.bq[0].run(l[i]);
        st.s[0] += 1.0;
        if st.s[0] >= hold_step {
            st.s[1] = lp;
            st.s[0] -= hold_step;
        }
        let y = (st.s[1] * step_bits).round() / step_bits;
        st.dl[0].write(mem, y);
        let delayed = st.dl[0].read(mem, delay_d);
        l[i] = clampf(y * (1.0 - smear) + delayed * smear, -16.0, 16.0);

        let lp = st.bq[1].run(r[i]);
        st.s[2] += 1.0;
        if st.s[2] >= hold_step {
            st.s[3] = lp;
            st.s[2] -= hold_step;
        }
        let y = (st.s[3] * step_bits).round() / step_bits;
        st.dl[1].write(mem, y);
        let delayed = st.dl[1].read(mem, delay_d);
        r[i] = clampf(y * (1.0 - smear) + delayed * smear, -16.0, 16.0);
    }
}

/// Bandpasses to a broadcast-band shape by `kind` (`:am :fm :sw`), adds
/// tuning warble and static.
fn radio(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let idx = (pv(p, 0, 0.0).max(0.0) as usize).min(2);
    let tuning = pv(p, 1, 0.2).clamp(0.0, 1.0);
    let noise = pv(p, 2, 0.15).clamp(0.0, 1.0);
    const LO: [f32; 3] = [300.0, 50.0, 300.0];
    const HI: [f32; 3] = [3000.0, 15_000.0, 3400.0];
    st.bq[0].set(Shape::Lowpass, HI[idx], 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, HI[idx], 0.707, 0.0, sr);
    st.bq[2].set(Shape::Highpass, LO[idx], 0.707, 0.0, sr);
    st.bq[3].set(Shape::Highpass, LO[idx], 0.707, 0.0, sr);
    let warble_rate = 0.4 + tuning * 2.0;
    let n = l.len().min(r.len());
    for i in 0..n {
        let m = 1.0 - tuning * 0.25 * (1.0 - st.lfo[0].uni(warble_rate, sr));
        let stat = st.rng.bipolar() * noise * (0.3 + tuning * 0.7);
        let x = st.bq[0].run(l[i]);
        let x = st.bq[2].run(x);
        l[i] = clampf(x * m * (1.0 - noise * 0.3) + stat, -16.0, 16.0);
        let x = st.bq[1].run(r[i]);
        let x = st.bq[3].run(x);
        r[i] = clampf(x * m * (1.0 - noise * 0.3) + stat, -16.0, 16.0);
    }
}

/// A narrow telephony-ish band plus soft compression and a faint hum.
fn tv_audio(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let amount = pv(p, 0, 0.4).clamp(0.0, 1.0);
    let lp_freq = 8000.0 - amount * 3000.0;
    let hp_freq = 300.0 + amount * 300.0;
    st.bq[0].set(Shape::Lowpass, lp_freq, 0.707, 0.0, sr);
    st.bq[1].set(Shape::Lowpass, lp_freq, 0.707, 0.0, sr);
    st.bq[2].set(Shape::Highpass, hp_freq, 0.707, 0.0, sr);
    st.bq[3].set(Shape::Highpass, hp_freq, 0.707, 0.0, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let hum = st.lfo[0].next(60.0, sr) * 0.015 * amount;
        let x = st.bq[0].run(l[i]);
        let x = st.bq[2].run(x);
        l[i] = clampf(soft_clip(x * (1.0 + amount)) + hum, -16.0, 16.0);
        let x = st.bq[1].run(r[i]);
        let x = st.bq[3].run(x);
        r[i] = clampf(soft_clip(x * (1.0 + amount)) + hum, -16.0, 16.0);
    }
}

/// Random digital dropouts: holds the last good sample for a short burst.
fn digital_error(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let rate = pv(p, 0, 2.0).clamp(0.0, 50.0);
    let p_err = (rate / sr).clamp(0.0, 1.0);
    let n = l.len().min(r.len());
    for i in 0..n {
        if st.s[1] > 0.0 {
            st.s[1] -= 1.0;
            l[i] = clampf(st.s[0], -16.0, 16.0);
        } else if st.rng.unit() < p_err {
            st.s[0] = l[i];
            st.s[1] = 4.0 + st.rng.unit() * 12.0;
            l[i] = clampf(st.s[0], -16.0, 16.0);
        } else {
            l[i] = clampf(l[i], -16.0, 16.0);
        }
        if st.s[3] > 0.0 {
            st.s[3] -= 1.0;
            r[i] = clampf(st.s[2], -16.0, 16.0);
        } else if st.rng.unit() < p_err {
            st.s[2] = r[i];
            st.s[3] = 4.0 + st.rng.unit() * 12.0;
            r[i] = clampf(st.s[2], -16.0, 16.0);
        } else {
            r[i] = clampf(r[i], -16.0, 16.0);
        }
    }
}

/// dsd-imd: approximation — a quadratic intermodulation term plus
/// envelope-shaped high-frequency noise, standing in for 1-bit
/// noise-shaping artifacts (no real DSD modulator is implemented).
fn dsd_imd(p: &[f32], st: &mut FxState, l: &mut [f32], r: &mut [f32], sr: f32) {
    let amount = pv(p, 0, 0.3).clamp(0.0, 1.0);
    let att = OnePole::time_coef(0.002, sr);
    let rel = OnePole::time_coef(0.05, sr);
    let n = l.len().min(r.len());
    for i in 0..n {
        let env = st.env[0].run(l[i], att, rel);
        let imd = (l[i] * l[i] - st.s[0] * st.s[0]) * amount * 0.5;
        let hf = st.rng.bipolar() * env * amount * 0.3;
        st.s[0] = l[i];
        l[i] = clampf(l[i] + imd + hf, -16.0, 16.0);

        let env = st.env[1].run(r[i], att, rel);
        let imd = (r[i] * r[i] - st.s[1] * st.s[1]) * amount * 0.5;
        let hf = st.rng.bipolar() * env * amount * 0.3;
        st.s[1] = r[i];
        r[i] = clampf(r[i] + imd + hf, -16.0, 16.0);
    }
}
