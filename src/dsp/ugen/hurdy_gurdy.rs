//! Hurdy-gurdy voice: four event-local bowed waveguides and a chien bridge.

mod bowed;
mod buzz;

use super::{Inp, Kx, NodeState, MAX_PORTS};
use bowed::BowedString;
use buzz::Rattle;

const STRING_COUNT: usize = 4;
const LINE_FLOATS: usize = 4_802;
const LINES_OFFSET: usize = 0;
const DATA_FLOATS: usize = STRING_COUNT * LINE_FLOATS;
const STRING_STATE_FLOATS: usize = 3;
const STRING_STATE_OFFSET: usize = DATA_FLOATS;
const RATTLE_STATE_OFFSET: usize = STRING_STATE_OFFSET + STRING_COUNT * STRING_STATE_FLOATS;
const RATTLE_STATE_FLOATS: usize = 3;
const PHASE: usize = 0;
const GATE_LEFT: usize = 1;
const TAIL_ENERGY: usize = 2;

/// Number of ports in the pinned contract.
pub const PORT_COUNT: usize = 16;
/// Stable port names and defaults; order is part of the template contract.
pub const PORTS: [(&str, f32); PORT_COUNT] = [
    ("freq", 440.0),
    ("cps", 0.5),
    ("onset-time", 0.0),
    ("gurdy-wheel", 0.5),
    ("gurdy-pressure", 0.5),
    ("gurdy-melody", 1.0),
    ("gurdy-bourdon", 0.6),
    ("gurdy-fifth", 0.4),
    ("gurdy-trompette", 0.5),
    ("gurdy-drone-key", 43.0),
    ("gurdy-buzz", 0.6),
    ("gurdy-buzz-threshold", 0.5),
    ("gurdy-strokes", 0.0),
    ("gurdy-stroke-depth", 0.5),
    ("gate-length", 8.0),
    ("velocity", 1.0),
];

/// Stable indices for the named ports.
pub mod port {
    pub const FREQ: usize = 0;
    pub const CPS: usize = 1;
    pub const ONSET_TIME: usize = 2;
    pub const GURDY_WHEEL: usize = 3;
    pub const GURDY_PRESSURE: usize = 4;
    pub const GURDY_MELODY: usize = 5;
    pub const GURDY_BOURDON: usize = 6;
    pub const GURDY_FIFTH: usize = 7;
    pub const GURDY_TROMPETTE: usize = 8;
    pub const GURDY_DRONE_KEY: usize = 9;
    pub const GURDY_BUZZ: usize = 10;
    pub const GURDY_BUZZ_THRESHOLD: usize = 11;
    pub const GURDY_STROKES: usize = 12;
    pub const GURDY_STROKE_DEPTH: usize = 13;
    pub const GATE_LENGTH: usize = 14;
    pub const VELOCITY: usize = 15;
}

/// Fixed per-voice arena: four 96 kHz/20 Hz lines, guards and scalar states.
/// Each line is 4,802 floats; the total is 19,223 floats.
pub const STATE_FLOATS: usize = RATTLE_STATE_OFFSET + RATTLE_STATE_FLOATS;

/// Renders a mono hurdy-gurdy voice with allocation-free fixed state.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if st.done() {
        out.fill(0.0);
        return;
    }
    if mem.len() < STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    debug_assert_eq!(BowedString::mem_floats(96_000.0), LINE_FLOATS);

    let sr = finite(kx.sr, 48_000.0).max(1.0);
    let cps = finite(ins[port::CPS].first(), 0.5).clamp(0.03, 50.0);
    let strokes = finite(ins[port::GURDY_STROKES].first(), 0.0).clamp(0.0, 16.0);
    let stroke_depth = unit(ins[port::GURDY_STROKE_DEPTH].first(), 0.5);
    let wheel_level = unit(ins[port::GURDY_WHEEL].first(), 0.5);
    let pressure = unit(ins[port::GURDY_PRESSURE].first(), 0.5);
    let buzz_gain = unit(ins[port::GURDY_BUZZ].first(), 0.6);
    let buzz_threshold = unit(ins[port::GURDY_BUZZ_THRESHOLD].first(), 0.5);

    if st.u[1] == 0 {
        let rate = strokes * cps;
        let onset = finite(ins[port::ONSET_TIME].first(), 0.0);
        st.s[PHASE] = (rate * onset).rem_euclid(1.0);
        let length = finite(ins[port::GATE_LENGTH].first(), 8.0).clamp(0.05, 64.0);
        st.s[GATE_LEFT] = (length / (16.0 * cps) * sr).max(1.0);
        st.s[TAIL_ENERGY] = 0.0;
        st.u[1] = 1;
    }

    let drone_key = finite(ins[port::GURDY_DRONE_KEY].first(), 43.0).clamp(24.0, 72.0);
    let frequencies = [
        finite(ins[port::FREQ].first(), 440.0).clamp(20.0, sr * 0.45),
        midi_hz(drone_key),
        midi_hz((drone_key + 7.0).min(127.0)),
        midi_hz((drone_key + 12.0).min(127.0)),
    ];
    let levels = [
        unit(ins[port::GURDY_MELODY].first(), 1.0),
        unit(ins[port::GURDY_BOURDON].first(), 0.6),
        unit(ins[port::GURDY_FIFTH].first(), 0.4),
        unit(ins[port::GURDY_TROMPETTE].first(), 0.5),
    ];

    let mut strings = [BowedString::default(); STRING_COUNT];
    for (index, string) in strings.iter_mut().enumerate() {
        let state = STRING_STATE_OFFSET + index * STRING_STATE_FLOATS;
        *string = BowedString::load(&mem[state..state + STRING_STATE_FLOATS]);
    }
    let mut rattle =
        Rattle::load(&mem[RATTLE_STATE_OFFSET..RATTLE_STATE_OFFSET + RATTLE_STATE_FLOATS]);
    let mut phase = finite(st.s[PHASE], 0.0).rem_euclid(1.0);
    let mut gate_left = finite(st.s[GATE_LEFT], 0.0);
    let mut tail_energy = finite(st.s[TAIL_ENERGY], 0.0).max(0.0);
    let mut should_finish = false;

    for frame in 0..out.len() {
        let ramp = if gate_left > 0.0 {
            1.0
        } else {
            (1.0 + gate_left / (0.01 * sr)).clamp(0.0, 1.0)
        };
        let stroke_value = if strokes > 0.0 {
            buzz::stroke(phase)
        } else {
            0.0
        };
        let effective_wheel = wheel_level * ramp * (1.0 + stroke_depth * stroke_value);
        let bow_velocity = effective_wheel * 0.5;
        let mut string_sum = 0.0;
        let mut trompette_bridge = 0.0;

        for index in 0..STRING_COUNT {
            if levels[index] == 0.0 {
                continue;
            }
            let line_start = LINES_OFFSET + index * LINE_FLOATS;
            let bridge = strings[index].tick(
                &mut mem[line_start..line_start + LINE_FLOATS],
                bow_velocity,
                pressure,
                frequencies[index],
                sr,
            );
            if index == 3 {
                trompette_bridge = bridge;
            }
            string_sum += bridge * levels[index];
        }

        let buzz = if buzz_gain == 0.0 {
            0.0
        } else {
            let bridge = if effective_wheel > buzz_threshold {
                trompette_bridge
            } else {
                0.0
            };
            rattle.tick(bridge * 6.0, effective_wheel, buzz_threshold, buzz_gain, sr)
        };
        let velocity = unit(ins[port::VELOCITY].at(frame), 1.0);
        out[frame] = finite((string_sum + buzz * 4.8) * velocity, 0.0).clamp(-1.0, 1.0);

        tail_energy = (tail_energy * 0.99).max(string_sum * string_sum);
        phase = (phase + strokes * cps / sr).rem_euclid(1.0);
        gate_left -= 1.0;
        if gate_left <= -(0.01 * sr) && tail_energy < 1.0e-8 {
            should_finish = true;
            out[frame + 1..].fill(0.0);
            break;
        }
    }

    for (index, string) in strings.iter().copied().enumerate() {
        let state = STRING_STATE_OFFSET + index * STRING_STATE_FLOATS;
        string.store(&mut mem[state..state + STRING_STATE_FLOATS]);
    }
    rattle.store(&mut mem[RATTLE_STATE_OFFSET..RATTLE_STATE_OFFSET + RATTLE_STATE_FLOATS]);
    st.s[PHASE] = finite(phase, 0.0);
    st.s[GATE_LEFT] = finite(gate_left, 0.0);
    st.s[TAIL_ENERGY] = finite(tail_energy, 0.0);
    if should_finish {
        st.finish();
    }
}

#[inline]
fn midi_hz(key: f32) -> f32 {
    440.0 * 2.0_f32.powf((key - 69.0) / 12.0)
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
fn unit(value: f32, fallback: f32) -> f32 {
    finite(value, fallback).clamp(0.0, 1.0)
}
