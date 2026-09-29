//! Mono nodes for the Plaits voice-level gate and decay modulation.

use super::{Inp, Kx, NodeState, MAX_PORTS};
use crate::dsp::ported::plaits_voice;
use crate::dsp::ugen::voice_layer::{
    clip_unit, compress_level, decay_terms, ping_attack, post_gain, shaped_amount, ControlClock,
    DecayEnvelope, LowPassGate, LpgMode, PostLimiter, VactrolEnvelope,
};

/// Persistent float state used by [`render_gate`].
pub const GATE_STATE_FLOATS: usize = 16;

/// Persistent float state used by [`render_decay_mod`].
pub const DECAY_MOD_STATE_FLOATS: usize = 4;

const CLOCK: usize = 0;
const VACTROL: usize = CLOCK + ControlClock::FLOATS;
const LPG: usize = VACTROL + VactrolEnvelope::FLOATS;
const LIMITER: usize = LPG + LowPassGate::FLOATS;
const GATE_REMAINING: usize = LIMITER + PostLimiter::FLOATS;

const DECAY_CLOCK: usize = 0;
const DECAY_ENVELOPE: usize = DECAY_CLOCK + ControlClock::FLOATS;
const DECAY_REMAINING: usize = DECAY_ENVELOPE + DecayEnvelope::FLOATS;
const DECAY_HELD: usize = DECAY_REMAINING + 1;

const LATCHED_MODE: usize = 0;
const LATCHED_SLOT: usize = 1;
const LATCHED_LANE: usize = 2;
const LATCHED_CLOCKED: usize = 3;
const LATCHED_VALID: usize = 4;
const LATCHED_GAIN: usize = 5;
const LATCHED_BYPASS: usize = 6;

#[inline]
fn mode_from_state(st: &NodeState) -> LpgMode {
    if st.s[LATCHED_MODE] == 1.0 {
        LpgMode::Ping
    } else if st.s[LATCHED_MODE] == 2.0 {
        LpgMode::Level
    } else {
        LpgMode::Off
    }
}

#[inline]
fn valid_sample_rate(sr: f32) -> f32 {
    if sr.is_finite() && sr > 0.0 {
        sr
    } else {
        48_000.0
    }
}

#[inline]
fn copy_input(input: &Inp<'_>, out: &mut [f32]) {
    match input {
        Inp::Val(value) => out.fill(*value),
        Inp::Buf(buffer) if buffer.len() >= out.len() => {
            out.copy_from_slice(&buffer[..out.len()]);
        }
        Inp::Buf(buffer) => {
            let copied = buffer.len().min(out.len());
            out[..copied].copy_from_slice(&buffer[..copied]);
            out[copied..].fill(0.0);
        }
    }
}

#[inline]
fn rounded_slot(value: f32) -> Option<usize> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let rounded = value.round();
    if !(0.0..24.0).contains(&rounded) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(rounded as usize)
}

#[inline]
fn remaining_samples(value: f32) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value as usize
    }
}

fn initialize_gate(st: &mut NodeState, mem: &mut [f32], ins: &[Inp<'_>; MAX_PORTS]) {
    let mode = LpgMode::from_control(ins[3].first());
    let slot = rounded_slot(ins[6].first());
    let lane = ins[7].first() >= 0.5;
    let clocked = ins[8].first() >= 0.5;
    let registration = slot.and_then(plaits_voice);
    let valid = registration.is_some();
    let gain = registration.map_or(1.0, |entry| entry.gain(u8::from(lane)));
    let bypass = registration.is_some_and(|entry| entry.is_enveloped(clocked));

    st.s[LATCHED_MODE] = match mode {
        LpgMode::Off => 0.0,
        LpgMode::Ping => 1.0,
        LpgMode::Level => 2.0,
    };
    #[allow(clippy::cast_precision_loss)]
    {
        st.s[LATCHED_SLOT] = slot.map_or(-1.0, |value| value as f32);
    }
    st.s[LATCHED_LANE] = if lane { 1.0 } else { 0.0 };
    st.s[LATCHED_CLOCKED] = if clocked { 1.0 } else { 0.0 };
    st.s[LATCHED_VALID] = if valid { 1.0 } else { 0.0 };
    st.s[LATCHED_GAIN] = gain;
    st.s[LATCHED_BYPASS] = if bypass { 1.0 } else { 0.0 };

    mem[..GATE_STATE_FLOATS].fill(0.0);
    ControlClock { carry: 0.0 }.store(&mut mem[CLOCK..CLOCK + ControlClock::FLOATS]);
    let mut envelope = VactrolEnvelope::new();
    if mode == LpgMode::Ping {
        envelope.trigger();
    }
    envelope.store(&mut mem[VACTROL..VACTROL + VactrolEnvelope::FLOATS]);
    LowPassGate::default().store(&mut mem[LPG..LPG + LowPassGate::FLOATS]);
    PostLimiter::new().store(&mut mem[LIMITER..LIMITER + PostLimiter::FLOATS]);
    mem[GATE_REMAINING] = 0.0;
    st.u[1] = 1;
    st.u[2] = u32::from(mode != LpgMode::Off && valid && !bypass);
}

/// Renders a mono voice-level low-pass gate and registered output stage.
pub fn render_gate(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < GATE_STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    if st.u[1] == 0 {
        initialize_gate(st, mem, ins);
    }

    if kx.gate < out.len() {
        st.u[0] = 1;
    }

    let mode = mode_from_state(st);
    let shaping = st.u[2] != 0;
    let input = &ins[0];
    if mode == LpgMode::Off || st.s[LATCHED_VALID] == 0.0 {
        copy_input(input, out);
    } else {
        let gain = st.s[LATCHED_GAIN];
        let limiter_needed = gain < 0.0;
        let post = post_gain(gain);
        let sr = valid_sample_rate(kx.sr);
        let mut limiter = PostLimiter::load(&mem[LIMITER..LIMITER + PostLimiter::FLOATS]);

        if !shaping {
            for (i, sample) in out.iter_mut().enumerate() {
                let x = input.at(i);
                let x1 = if limiter_needed {
                    limiter.process(-gain, sr, x)
                } else {
                    x
                };
                *sample = clip_unit(x1 * post);
            }
        } else {
            let mut clock = ControlClock::load(&mem[CLOCK..CLOCK + ControlClock::FLOATS]);
            let mut envelope =
                VactrolEnvelope::load(&mem[VACTROL..VACTROL + VactrolEnvelope::FLOATS]);
            let mut lpg = LowPassGate::load(&mem[LPG..LPG + LowPassGate::FLOATS]);
            let mut remaining = remaining_samples(mem[GATE_REMAINING]);

            for (i, sample) in out.iter_mut().enumerate() {
                if remaining == 0 {
                    let len = clock.next_len(sr);
                    let open = i < kx.gate;
                    let (short, tail) = decay_terms(ins[4].at(i), ins[5].at(i));
                    match mode {
                        LpgMode::Ping => {
                            envelope.process_ping(
                                ping_attack(ins[1].at(i)),
                                short,
                                tail,
                                ins[5].at(i),
                            );
                        }
                        LpgMode::Level => {
                            let level = if open {
                                compress_level(ins[2].at(i))
                            } else {
                                0.0
                            };
                            envelope.process_lp(level, short, tail, ins[5].at(i));
                        }
                        LpgMode::Off => {}
                    }
                    lpg.begin(
                        envelope.gain() * post,
                        envelope.frequency(),
                        envelope.hf_bleed(),
                        len,
                        sr,
                    );
                    remaining = len;
                }

                let x = input.at(i);
                let x1 = if limiter_needed {
                    limiter.process(-gain, sr, x)
                } else {
                    x
                };
                *sample = clip_unit(lpg.tick(x1));
                remaining -= 1;
            }

            clock.store(&mut mem[CLOCK..CLOCK + ControlClock::FLOATS]);
            envelope.store(&mut mem[VACTROL..VACTROL + VactrolEnvelope::FLOATS]);
            lpg.store(&mut mem[LPG..LPG + LowPassGate::FLOATS]);
            #[allow(clippy::cast_precision_loss)]
            {
                mem[GATE_REMAINING] = remaining as f32;
            }
        }
        limiter.store(&mut mem[LIMITER..LIMITER + PostLimiter::FLOATS]);
    }

    if shaping && st.u[0] == 1 {
        let envelope = VactrolEnvelope::load(&mem[VACTROL..VACTROL + VactrolEnvelope::FLOATS]);
        if envelope.is_done() {
            st.finish();
        }
    }
}

fn initialize_decay_mod(st: &mut NodeState, mem: &mut [f32]) {
    mem[..DECAY_MOD_STATE_FLOATS].fill(0.0);
    ControlClock { carry: 0.0 }.store(&mut mem[DECAY_CLOCK..DECAY_CLOCK + ControlClock::FLOATS]);
    let mut envelope = DecayEnvelope { value: 0.0 };
    envelope.trigger();
    envelope.store(&mut mem[DECAY_ENVELOPE..DECAY_ENVELOPE + DecayEnvelope::FLOATS]);
    mem[DECAY_REMAINING] = 0.0;
    mem[DECAY_HELD] = 0.0;
    st.u[1] = 1;
}

/// Renders the trigger-patched decay modulation control as a held mono signal.
pub fn render_decay_mod(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    if mem.len() < DECAY_MOD_STATE_FLOATS {
        out.fill(0.0);
        return;
    }
    if st.u[1] == 0 {
        initialize_decay_mod(st, mem);
    }

    let sr = valid_sample_rate(kx.sr);
    let mut clock = ControlClock::load(&mem[DECAY_CLOCK..DECAY_CLOCK + ControlClock::FLOATS]);
    let mut envelope =
        DecayEnvelope::load(&mem[DECAY_ENVELOPE..DECAY_ENVELOPE + DecayEnvelope::FLOATS]);
    let mut remaining = remaining_samples(mem[DECAY_REMAINING]);
    let mut held = mem[DECAY_HELD];

    for (i, sample) in out.iter_mut().enumerate() {
        if remaining == 0 {
            let len = clock.next_len(sr);
            let (short, _) = decay_terms(ins[0].at(i), 0.5);
            envelope.process(short);
            let amount = shaped_amount(ins[1].at(i));
            let modulated = amount * envelope.value();
            held = if ins[2].at(i) < 0.5 {
                modulated
            } else {
                2.0_f32.powf(modulated * envelope.value() * 4.0)
            };
            remaining = len;
        }
        *sample = held;
        remaining -= 1;
    }

    clock.store(&mut mem[DECAY_CLOCK..DECAY_CLOCK + ControlClock::FLOATS]);
    envelope.store(&mut mem[DECAY_ENVELOPE..DECAY_ENVELOPE + DecayEnvelope::FLOATS]);
    #[allow(clippy::cast_precision_loss)]
    {
        mem[DECAY_REMAINING] = remaining as f32;
    }
    mem[DECAY_HELD] = held;
}
