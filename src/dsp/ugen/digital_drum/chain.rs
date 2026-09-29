//! Shared digital-drum-family chain (design-music 4.1, DDRUM-001/003/004
//! shared core contract).
//!
//! Pure DSP primitives, the typed velocity/LFO target routers, and the
//! onset transient, filter/drive/decimation, amplitude-envelope + velocity,
//! per-voice LFO and repeat/voice-end stages every family (`tonal`,
//! `metal`) reuses without duplicating the math. Every function here is
//! either a pure value transform or takes the one small piece of state it
//! owns by value and returns the updated value, so a caller's `mem: &mut
//! [f32]` never needs two simultaneous mutable borrows of different
//! indices.

use std::f32::consts::TAU;

use crate::dsp::effects::prim::{Biquad, Rng, Shape};

use super::super::NodeState;

/// Clamps a possibly non-finite value; a non-finite input reads as
/// `fallback`.
pub(super) fn bounded(v: f32, lo: f32, hi: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        fallback
    }
}

pub(super) fn unit(v: f32, fallback: f32) -> f32 {
    bounded(v, 0.0, 1.0, fallback)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn enum_index(v: f32, max: i32) -> i32 {
    if v.is_finite() {
        v.round().clamp(0.0, max as f32) as i32
    } else {
        0
    }
}

/// A naive (non-band-limited) oscillator: 0 saw, 1 pulse (25% duty), 2
/// square (50% duty), 3 triangle, 4 sine (the shared `wave`/`mod-wave`/
/// `mod2-wave` domain order).
pub(super) fn wave_sample(kind: i32, phase: f32) -> f32 {
    let phase = phase.rem_euclid(1.0);
    match kind {
        0 => 2.0 * phase - 1.0,
        1 => {
            if phase < 0.25 {
                1.0
            } else {
                -1.0
            }
        }
        2 => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        3 => tri01(phase),
        _ => (TAU * phase).sin(),
    }
}

fn tri01(phase: f32) -> f32 {
    let t = phase * 4.0;
    if t < 1.0 {
        t
    } else if t < 3.0 {
        2.0 - t
    } else {
        t - 4.0
    }
}

/// An attack/decay envelope (0..1), blended linearly and exponentially by
/// `slope` (0 linear, 1 exponential decay curve).
fn attack_decay_env(t: f32, attack: f32, decay: f32, slope: f32) -> f32 {
    let attack = attack.max(1.0e-4);
    if t < attack {
        return (t / attack).clamp(0.0, 1.0);
    }
    let d = t - attack;
    let decay = decay.max(0.005);
    let lin = (1.0 - d / decay).max(0.0);
    let exp = (-d / decay).exp();
    ((1.0 - slope) * lin + slope * exp).clamp(0.0, 1.0)
}

/// A decay-only envelope from `t = 0` (the FM modulator envelope, and the
/// tonal family's pitch envelope, which have no attack stage).
pub(super) fn decay_env(t: f32, decay: f32, slope: f32) -> f32 {
    attack_decay_env(t, 1.0e-4, decay, slope)
}

/// A saturating soft-clip gain stage (a pre-filter drive and the post-
/// filter `drive` stage share this shape).
fn drive_stage(x: f32, amount: f32) -> f32 {
    let g = 1.0 + amount.max(0.0) * 8.0;
    let n = g.tanh();
    if n.abs() < 1.0e-6 {
        x
    } else {
        (x * g).tanh() / n
    }
}

fn filter_shape(kind: i32) -> Option<Shape> {
    match kind {
        1 => Some(Shape::Lowpass),
        2 => Some(Shape::Bandpass),
        3 => Some(Shape::Highpass),
        4 => Some(Shape::Notch),
        _ => None,
    }
}

/// The additive/multiplicative deltas the typed velocity/LFO routes
/// accumulate before the parameters they name are used (every family
/// implements every target in both target domains).
#[derive(Default, Clone, Copy)]
pub(super) struct Mods {
    pub pitch: f32,
    pub mod_depth: f32,
    pub cutoff: f32,
    pub res: f32,
    pub amp: f32,
    pub drive: f32,
    pub decimation: f32,
    pub transient: f32,
    pub decay: f32,
}

/// `velocity-target`'s domain: none, pitch, mod, cutoff, decay, transient,
/// drive.
pub(super) fn apply_velocity_target(target: i32, delta: f32, m: &mut Mods) {
    match target {
        1 => m.pitch += delta * 24.0,
        2 => m.mod_depth += delta,
        3 => m.cutoff += delta * 2.0,
        4 => m.decay += delta * 0.8,
        5 => m.transient += delta * 0.8,
        6 => m.drive += delta * 0.8,
        _ => {}
    }
}

/// `lfo-target`'s domain: none, pitch, mod, cutoff, res, amp, drive,
/// decimation, transient, decay (no `pan`: every core here is mono).
pub(super) fn apply_lfo_target(target: i32, delta: f32, m: &mut Mods) {
    match target {
        1 => m.pitch += delta * 12.0,
        2 => m.mod_depth += delta,
        3 => m.cutoff += delta * 1.5,
        4 => m.res += delta * 0.5,
        5 => m.amp += delta * 0.9,
        6 => m.drive += delta * 0.6,
        7 => m.decimation += delta.abs() * 0.6,
        8 => m.transient += delta * 0.6,
        9 => m.decay += delta * 0.6,
        _ => {}
    }
}

/// The LFO's own waveform, `lfo-wave`'s domain: sine, tri, saw, ramp,
/// square, random (sample-and-hold, refreshed once per cycle).
fn lfo_sample(kind: i32, phase: f32, rand_state: &mut f32, wrapped: bool, rng: &mut Rng) -> f32 {
    if wrapped && kind == 5 {
        *rand_state = rng.bipolar();
    }
    match kind {
        0 => (TAU * phase).sin(),
        1 => 2.0 * tri01(phase) - 1.0,
        2 => 1.0 - 2.0 * phase,
        3 => 2.0 * phase - 1.0,
        4 => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        _ => *rand_state,
    }
}

/// The per-voice LFO's start-of-voice phase: `lfo-retrigger` anchors it to
/// `lfo-offset`; otherwise it derives from the event's absolute host time
/// (`onset-time`), so independently triggered voices at the same rate
/// share phase (DDRUM-002A/003).
pub(super) fn lfo_init_phase(
    retrigger: bool,
    sync: bool,
    cps: f32,
    rate: f32,
    onset: f32,
    offset: f32,
) -> f32 {
    let rate_hz = if sync { rate * cps } else { rate };
    if retrigger {
        offset
    } else {
        (rate_hz * onset).rem_euclid(1.0)
    }
}

/// Advances the LFO phase by one sample; returns `(value, next_phase,
/// next_rand_state)`.
#[allow(clippy::too_many_arguments)]
pub(super) fn lfo_step(
    lfo_wave: i32,
    lfo_rate: f32,
    lfo_sync: bool,
    cps: f32,
    sr: f32,
    phase: f32,
    rand_state: f32,
    rng: &mut Rng,
) -> (f32, f32, f32) {
    let rate_hz = if lfo_sync { lfo_rate * cps } else { lfo_rate };
    let next_phase = (phase + rate_hz / sr).rem_euclid(1.0);
    let wrapped = next_phase < phase;
    let mut rand_state = rand_state;
    let value = lfo_sample(lfo_wave, phase, &mut rand_state, wrapped, rng);
    (value, next_phase, rand_state)
}

/// The onset transient generator (click/snap/noise/sweep): a short, fixed
/// ~20 ms decay independent of the amplitude envelope. Returns `(output,
/// next_color_state)`.
pub(super) fn transient_stage(
    transient_wave: i32,
    transient_freq: f32,
    transient_level_eff: f32,
    te: f32,
    color: f32,
    rng: &mut Rng,
    sr: f32,
) -> (f32, f32) {
    let t_env = decay_env(te, 0.02, 1.0);
    let alpha_t = 1.0 - (-TAU * transient_freq / sr).exp();
    let color = color + alpha_t * (rng.bipolar() - color);
    let raw = match transient_wave {
        1 => (TAU * transient_freq * te).sin(),
        2 => rng.bipolar(),
        3 => (TAU * transient_freq * (1.0 + 2.0 * t_env) * te).sin(),
        _ => color,
    };
    (raw * t_env * transient_level_eff, color)
}

/// The filter (off/lp/bp/hp/notch, driven by a pre-filter soft clip), the
/// post-filter `drive` stage, and sample-and-hold decimation. Returns
/// `(output, next_decim_hold, next_decim_counter)`.
#[allow(clippy::too_many_arguments)]
pub(super) fn filter_drive_decimate(
    raw: f32,
    filter_type: i32,
    cutoff: f32,
    res: f32,
    filter_drive: f32,
    drive: f32,
    decimation: f32,
    mods: &Mods,
    bq: &mut Biquad,
    decim_hold: f32,
    decim_counter: f32,
    sr: f32,
) -> (f32, f32, f32) {
    let cutoff_eff = (cutoff * (1.0 + mods.cutoff)).clamp(40.0, sr * 0.45);
    let res_eff = (res + mods.res).clamp(0.05, 4.0);
    let filtered = match filter_shape(filter_type) {
        Some(shape) => {
            let driven = drive_stage(raw, filter_drive);
            bq.set(shape, cutoff_eff, 0.4 + res_eff * 6.0, 0.0, sr);
            bq.run(driven)
        }
        None => raw,
    };
    let drive_eff = (drive + mods.drive).clamp(0.0, 1.0);
    let driven = drive_stage(filtered, drive_eff);
    let decimation_eff = (decimation + mods.decimation).clamp(0.0, 1.0);
    let hold_n = (1.0 + decimation_eff * 39.0).round();
    let (next_hold, next_counter) = if decimation_eff > 0.001 {
        if decim_counter >= hold_n {
            (driven, 1.0)
        } else {
            (decim_hold, decim_counter + 1.0)
        }
    } else {
        (driven, decim_counter)
    };
    (next_hold, next_hold, next_counter)
}

/// The amplitude envelope, the velocity-scaled output stage and the LFO's
/// amplitude route; returns `(sample, amp_decay_eff)` (the caller needs
/// the effective decay for the voice-end threshold).
#[allow(clippy::too_many_arguments)]
pub(super) fn amp_velocity_stage(
    te: f32,
    amp_attack: f32,
    amp_decay: f32,
    amp_slope: f32,
    mods: &Mods,
    volume_velocity: f32,
    velocity: f32,
    decimated: f32,
) -> (f32, f32) {
    let amp_decay_eff = (amp_decay * (1.0 + mods.decay)).clamp(0.005, 16.0);
    let amp_env = attack_decay_env(te, amp_attack, amp_decay_eff, amp_slope);
    let amp_extra = (1.0 + mods.amp).clamp(0.0, 3.0);
    let vol_vel = (1.0 - volume_velocity + volume_velocity * velocity).clamp(0.0, 2.0);
    let level = decimated * amp_env * amp_extra * vol_vel;
    let sample = if level.is_finite() {
        level.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    (sample, amp_decay_eff)
}

/// What happened to the voice's repeat/finish state this sample.
pub(super) enum RepeatAction {
    None,
    /// A repeat fired: the caller resets its own oscillator phase state.
    Retrigger,
    Finished,
}

/// `repeat-count`/`repeat-time` retriggering, and the voice-end decision
/// once every repeat's tail has decayed (`longest_decay` is the slowest of
/// the family's own decay controls; `* 10` past it is silence). Returns
/// `(action, next_repeats_fired, next_hit_elapsed)`.
#[allow(clippy::too_many_arguments)]
pub(super) fn repeat_or_finish(
    repeat_count: i32,
    repeat_time: f32,
    repeats_fired: f32,
    hit_elapsed: f32,
    voice_elapsed: f32,
    longest_decay: f32,
    st: &mut NodeState,
) -> (RepeatAction, f32, f32) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let repeats_fired_u = repeats_fired as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let repeat_count_u = repeat_count.max(0) as u32;
    #[allow(clippy::cast_precision_loss)]
    let next_repeat_at = repeat_time * (repeats_fired_u + 1) as f32;
    if repeats_fired_u < repeat_count_u && voice_elapsed >= next_repeat_at {
        (RepeatAction::Retrigger, repeats_fired + 1.0, 0.0)
    } else if repeats_fired_u >= repeat_count_u && hit_elapsed > longest_decay.max(0.05) * 10.0 {
        st.finish();
        (RepeatAction::Finished, repeats_fired, hit_elapsed)
    } else {
        (RepeatAction::None, repeats_fired, hit_elapsed)
    }
}
