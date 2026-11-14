//! Three-component triggered percussion: feedback noise, filtered noise,
//! and sine FM. The architecture and controls are informed by Christopher
//! Arndt's MIT-declared `fbnfm_drumvoice.dsp`. All kernels are independent
//! Rust code, with no Faust library translation or generated source.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::{DelayLine, Rng, Shape};

use super::{Inp, Kx, NodeState, MAX_PORTS};

fn bounded(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        default
    }
}

fn db_gain(v: f32) -> f32 {
    10.0_f32.powf(bounded(v, -90.0, 0.0, -6.0) / 20.0)
}

fn velocity(v: f32, sensitivity: f32) -> f32 {
    let v = bounded(v, 0.0, 1.0, 1.0);
    let sensitivity = bounded(sensitivity, 0.0, 1.0, 1.0);
    1.0 - sensitivity + sensitivity * v
}

fn seeded_rng(st: &mut NodeState, seed: u32) -> Rng {
    if st.u[1] == 0 {
        st.u[0] = Rng::new(seed).s;
        st.u[1] = 1;
    }
    Rng::new(st.u[0])
}

/// Attack, hold, exponential decay, and gate release. `st.s[2]` is elapsed
/// time, `s[3]` the level at release and `s[4]` release elapsed time.
fn envelope(
    st: &mut NodeState,
    gate_open: bool,
    attack: f32,
    hold: f32,
    decay: f32,
    sr: f32,
) -> f32 {
    let t = st.s[2];
    let attack = attack.max(0.001);
    let decay = decay.max(0.001);
    let level = if t < attack {
        t / attack
    } else if t < attack + hold {
        1.0
    } else {
        (-(t - attack - hold) / decay).exp()
    };
    if !gate_open && st.u[2] == 0 {
        st.u[2] = 1;
        st.s[3] = level;
        st.s[4] = 0.0;
    }
    let result = if st.u[2] == 0 {
        level
    } else {
        st.s[4] += 1.0 / sr;
        st.s[3] * (-st.s[4] / decay).exp()
    };
    st.s[2] += 1.0 / sr;
    if st.s[2] > attack + hold + decay * 12.0 || (st.u[2] != 0 && st.s[4] > decay * 12.0) {
        st.finish();
    }
    result
}

/// Band-limited noise excited by a per-hit envelope and recirculated
/// through a short delay. Delay state lives in install-time voice memory.
pub fn feedback(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    let mut rng = seeded_rng(st, kx.seed ^ 0xFB12_78AF);
    if st.dl.capacity() == 0 {
        st.dl = DelayLine::carve(&mut 0, mem.len(), 1001);
    }
    let sr = kx.sr.max(1.0);
    for (i, sample) in out.iter_mut().enumerate() {
        if st.done() {
            *sample = 0.0;
            continue;
        }
        let delay = bounded(ins[1].at(i), 0.0, 1000.0, 100.0).max(1.0);
        let feedback = bounded(ins[2].at(i), 0.9, 0.999, 0.99);
        let decay = bounded(ins[3].at(i), 0.001, 5.0, 0.5);
        let cutoff = bounded(ins[4].at(i), 20.0, 0.45 * sr, 5000.0);
        let q = bounded(ins[5].at(i), 0.5, 30.0, 1.0);
        st.bq[0].set(Shape::Bandpass, cutoff, q, 0.0, sr);
        let env = envelope(st, i < kx.gate, 0.001, 0.0, decay, sr);
        let excitation = st.bq[0].run(rng.bipolar() * env);
        let delayed = st.dl.read(mem, delay);
        let resonating = (excitation + delayed * feedback).tanh();
        st.dl.write(mem, resonating);
        let gain = velocity(ins[0].at(i), ins[6].at(i)) * db_gain(ins[7].at(i));
        *sample = resonating * gain;
    }
    st.u[0] = rng.s;
}

/// Independent filtered-noise transient with envelope-controlled center
/// frequency. The pitch modulation is expressed in cents.
pub fn noise(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let mut rng = seeded_rng(st, kx.seed ^ 0xA015_E302);
    let sr = kx.sr.max(1.0);
    for (i, sample) in out.iter_mut().enumerate() {
        if st.done() {
            *sample = 0.0;
            continue;
        }
        let attack = bounded(ins[1].at(i), 0.001, 3.0, 0.001);
        let hold = bounded(ins[2].at(i), 0.0, 3.0, 0.0);
        let decay = bounded(ins[3].at(i), 0.001, 5.0, 0.5);
        let env = envelope(st, i < kx.gate, attack, hold, decay, sr);
        let cutoff = bounded(ins[4].at(i), 20.0, 0.45 * sr, 5000.0);
        let q = bounded(ins[5].at(i), 0.1, 30.0, 1.0);
        let cents = bounded(ins[6].at(i), -4800.0, 4800.0, 0.0);
        let swept = (cutoff * 2.0_f32.powf(cents * env / 1200.0)).clamp(20.0, 0.45 * sr);
        st.bq[0].set(Shape::Bandpass, swept, q, 0.0, sr);
        let gain = velocity(ins[0].at(i), ins[7].at(i)) * db_gain(ins[8].at(i));
        *sample = st.bq[0].run(rng.bipolar() * env) * gain;
    }
    st.u[0] = rng.s;
}

/// Sine carrier with a sine frequency modulator, key tracking, and a pitch
/// envelope. The note's `freq` in hertz replaces the source's MIDI key.
pub fn sine(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, out: &mut [f32], kx: &Kx<'_>) {
    let sr = kx.sr.max(1.0);
    let mut carrier_phase = st.s[0];
    let mut mod_phase = st.s[1];
    for (i, sample) in out.iter_mut().enumerate() {
        if st.done() {
            *sample = 0.0;
            continue;
        }
        let note_freq = bounded(ins[0].at(i), 20.0, 20_000.0, 440.0);
        let tuning = bounded(ins[2].at(i), 20.0, 10_000.0, 440.0);
        let keytrack = bounded(ins[3].at(i), -2.0, 2.0, 0.0);
        let ratio = bounded(ins[4].at(i), 0.5, 20.0, 1.0);
        let index = bounded(ins[5].at(i), 0.0, 50.0, 0.0);
        let attack = bounded(ins[6].at(i), 0.001, 3.0, 0.002);
        let hold = bounded(ins[7].at(i), 0.0, 3.0, 0.0);
        let decay = bounded(ins[8].at(i), 0.001, 5.0, 1.5);
        let env = envelope(st, i < kx.gate, attack, hold, decay, sr);
        let cents = bounded(ins[9].at(i), -4800.0, 4800.0, 0.0);
        let base = tuning * (note_freq / 440.0).powf(keytrack);
        let freq = (base * 2.0_f32.powf(cents * env / 1200.0)).clamp(0.0, 0.45 * sr);
        let mod_freq = (freq * ratio).min(0.45 * sr);
        let instantaneous = (freq * (1.0 + index * mod_phase.sin())).clamp(-0.45 * sr, 0.45 * sr);
        let gain = velocity(ins[1].at(i), ins[10].at(i)) * db_gain(ins[11].at(i));
        *sample = carrier_phase.sin() * env * gain;
        carrier_phase = (carrier_phase + TAU * instantaneous / sr).rem_euclid(TAU);
        mod_phase = (mod_phase + TAU * mod_freq / sr).rem_euclid(TAU);
    }
    st.s[0] = carrier_phase;
    st.s[1] = mod_phase;
}
