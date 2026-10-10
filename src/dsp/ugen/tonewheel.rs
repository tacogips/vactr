//! Original drawbar tonewheel organ synthesis from the accepted FM1 voice design.
//! This kernel uses analytic partials and a delay-line scanner; it does not use emulation source.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use crate::dsp::effects::prim::{Biquad, DelayLine, Rng, Shape};
use std::f32::consts::TAU;

/// Number of ports in the pinned contract.
pub const PORT_COUNT: usize = 20;
/// Stable port names and defaults; order is part of the template contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 440.0),
    ("cps", 0.5),
    ("onset-time", 0.0),
    ("drawbar1", 8.0),
    ("drawbar2", 8.0),
    ("drawbar3", 8.0),
    ("drawbar4", 0.0),
    ("drawbar5", 0.0),
    ("drawbar6", 0.0),
    ("drawbar7", 0.0),
    ("drawbar8", 0.0),
    ("drawbar9", 0.0),
    ("organ-click", 0.3),
    ("organ-perc", 0.0),
    ("organ-perc-slow", 0.0),
    ("organ-perc-soft", 0.0),
    ("organ-perc-trigger", 1.0),
    ("organ-vibrato", 0.0),
    ("gate-length", 4.0),
    ("velocity", 1.0),
];

/// Stable indices for the named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const CPS: usize = 1;
    pub const ONSET_TIME: usize = 2;
    pub const DRAWBAR1: usize = 3;
    pub const DRAWBAR2: usize = 4;
    pub const DRAWBAR3: usize = 5;
    pub const DRAWBAR4: usize = 6;
    pub const DRAWBAR5: usize = 7;
    pub const DRAWBAR6: usize = 8;
    pub const DRAWBAR7: usize = 9;
    pub const DRAWBAR8: usize = 10;
    pub const DRAWBAR9: usize = 11;
    pub const ORGAN_CLICK: usize = 12;
    pub const ORGAN_PERC: usize = 13;
    pub const ORGAN_PERC_SLOW: usize = 14;
    pub const ORGAN_PERC_SOFT: usize = 15;
    pub const ORGAN_PERC_TRIGGER: usize = 16;
    pub const ORGAN_VIBRATO: usize = 17;
    pub const GATE_LENGTH: usize = 18;
    pub const VELOCITY: usize = 19;
}

const PHASES: usize = 9;
const MAX_DELAY_FLOATS: usize = 292;
const DELAY_BASE: usize = PHASES;
const LN_100: f32 = 4.605_170_2;
const RATIOS: [f32; 9] = [
    0.5,
    1.498_307_1,
    1.0,
    2.0,
    2.996_614_3,
    4.0,
    5.039_684_3,
    5.993_228_4,
    8.0,
];

/// Fixed per-voice arena, including the scanner delay at 96 kHz.
pub const STATE_FLOATS: usize = PHASES + MAX_DELAY_FLOATS;

/// Renders one mono drawbar tonewheel voice.
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if st.done() || mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    let sr = if kx.sr.is_finite() {
        kx.sr.clamp(1.0, 96_000.0)
    } else {
        48_000.0
    };
    let mut delay = st.dl;
    if st.u[1] == 0 {
        mem[..STATE_FLOATS].fill(0.0);
        st.s = [0.0; 8];
        st.bq = [Biquad::default(); 2];
        st.u[0] = kx.seed ^ 0x544F_4E45;
        st.u[1] = 1;
        st.s[0] = gate_samples(value(ins, port::GATE_LENGTH), value(ins, port::CPS), sr);
        st.u[2] = percussion_mode(
            value(ins, port::ORGAN_PERC),
            value(ins, port::ORGAN_PERC_TRIGGER),
        );
        let mut cursor = DELAY_BASE;
        delay = DelayLine::carve(&mut cursor, STATE_FLOATS, delay_len(sr));
        st.bq[0].set(Shape::Bandpass, 3_000.0, 1.0, 0.0, sr);
    }
    let click = click_level(ins);
    let mode = st.u[2] & 3;
    let perc_enabled = mode != 0;
    let perc_ratio = if mode == 1 { 2.0 } else { RATIOS[4] };
    let perc_slow = value(ins, port::ORGAN_PERC_SLOW) >= 0.5;
    let perc_soft = value(ins, port::ORGAN_PERC_SOFT) >= 0.5;
    let t60 = if perc_slow { 1.0 } else { 0.25 };
    let perc_gain = if perc_soft {
        10.0_f32.powf(-10.0 / 20.0)
    } else {
        1.0
    };
    let vibrato = finite(value(ins, port::ORGAN_VIBRATO), 0.0)
        .round()
        .clamp(0.0, 6.0) as u8;
    let depth = match vibrato {
        1 | 4 => 0.000_25,
        2 | 5 => 0.000_5,
        3 | 6 => 0.001,
        _ => 0.0,
    };
    let velocity = finite(value(ins, port::VELOCITY), 1.0).clamp(0.0, 1.0);
    let mut gate_left = st.s[0].max(0.0);
    let mut release_left = st.s[1].max(0.0);
    let mut release_armed = st.s[5] >= 0.5;
    let release_samples = (sr * 0.005).max(1.0);
    let mut elapsed = st.s[2].max(0.0);
    let mut scan_phase = st.s[3];
    let mut perc_phase = st.s[4];
    let delay_samples = delay_len(sr) as f32;
    let mut rng = Rng::new(st.u[0]);
    for (i, sample) in out.iter_mut().enumerate() {
        let frequency = finite(ins[port::FREQ].at(i), 440.0).clamp(1.0, sr * 0.45);
        let mut dry = 0.0;
        for (index, ratio) in RATIOS.iter().copied().enumerate() {
            let partial = fold_frequency(frequency * ratio);
            let increment = partial / sr;
            let phase = &mut mem[index];
            let wave = if partial < sr * 0.45 {
                (TAU * *phase).sin()
            } else {
                0.0
            };
            *phase = (*phase + increment).fract();
            let drawbar = finite(ins[port::DRAWBAR1 + index].first(), 0.0)
                .round()
                .clamp(0.0, 8.0) as u8;
            if drawbar > 0 && !(index == 8 && perc_enabled) {
                dry += wave * drawbar_gain(drawbar);
            }
        }
        dry *= 1.0 / 9.0;
        if perc_enabled && !perc_soft {
            dry *= 10.0_f32.powf(-3.0 / 20.0);
        }
        if perc_enabled {
            let freq = fold_frequency(frequency * perc_ratio).min(sr * 0.45);
            let perc =
                (TAU * perc_phase).sin() * perc_gain * 10.0_f32.powf(-3.0 * elapsed / (t60 * sr));
            dry += perc / 9.0;
            perc_phase = (perc_phase + freq / sr).fract();
        }
        let click_sample = if click > 0.0 && elapsed < sr * 0.004 {
            let noise = rng.bipolar();
            st.bq[0].run(noise) * (-LN_100 * elapsed / (sr * 0.004)).exp() * click
        } else {
            0.0
        };
        let scanned_delay = 0.001 * sr + depth * sr * (TAU * scan_phase).sin();
        delay.write(mem, dry);
        let scanned = delay.read(
            mem,
            scanned_delay.clamp(1.0, (delay_samples - 2.0).max(1.0)),
        );
        let organ = match vibrato {
            1..=3 => scanned,
            4..=6 => 0.5 * (dry + scanned),
            _ => dry,
        };
        let mut level = velocity;
        if !release_armed && gate_left <= 0.0 {
            release_armed = true;
            release_left = release_samples;
        }
        if release_armed {
            level *= (release_left / release_samples).clamp(0.0, 1.0);
            release_left = (release_left - 1.0).max(0.0);
        } else {
            gate_left = (gate_left - 1.0).max(0.0);
        }
        *sample = finite((organ + click_sample) * level, 0.0).clamp(-1.0, 1.0);
        elapsed += 1.0;
        scan_phase = (scan_phase + 6.9 / sr).fract();
        if release_armed && release_left <= 0.0 {
            out[i + 1..].fill(0.0);
            st.finish();
            break;
        }
    }
    st.dl = delay;
    st.s[0] = gate_left;
    st.s[1] = release_left;
    st.s[2] = elapsed;
    st.s[3] = scan_phase;
    st.s[4] = perc_phase;
    st.s[5] = if release_armed { 1.0 } else { 0.0 };
    st.u[0] = rng.s;
}

#[inline]
fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
fn value(ins: &[Inp<'_>; MAX_PORTS], index: usize) -> f32 {
    ins[index].first()
}

#[inline]
fn click_level(ins: &[Inp<'_>; MAX_PORTS]) -> f32 {
    finite(value(ins, port::ORGAN_CLICK), 0.0).clamp(0.0, 1.0)
}

#[inline]
fn gate_samples(length: f32, cps: f32, sr: f32) -> f32 {
    let length = finite(length, 1.0).clamp(0.05, 64.0);
    let cps = finite(cps, 0.5).clamp(0.03, 50.0);
    (length / (16.0 * cps) * sr).round().max(1.0)
}

#[inline]
fn delay_len(sr: f32) -> usize {
    ((0.003 * sr).ceil() as usize + 4).min(MAX_DELAY_FLOATS)
}

#[inline]
fn percussion_mode(mode: f32, trigger: f32) -> u32 {
    if finite(trigger, 0.0) < 0.5 {
        0
    } else {
        finite(mode, 0.0).round().clamp(0.0, 2.0) as u32
    }
}

#[inline]
fn fold_frequency(mut frequency: f32) -> f32 {
    while frequency > 5_900.0 {
        frequency *= 0.5;
    }
    while frequency < 32.70 {
        frequency *= 2.0;
    }
    frequency
}

#[inline]
fn drawbar_gain(drawbar: u8) -> f32 {
    if drawbar == 0 {
        0.0
    } else {
        10.0_f32.powf(-3.0 * f32::from(8 - drawbar) / 20.0)
    }
}
