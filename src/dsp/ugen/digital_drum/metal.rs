//! `digital-metal-core` and `digital-hat-core`: the metal/cymbal and hat
//! kernel (design-music 4.1, DDRUM-004), built on the shared chain in
//! `chain.rs` (transient, filter/drive/decimation, amplitude envelope +
//! velocity, per-voice LFO, repeat/voice-end — DDRUM-003's shared core
//! contract).
//!
//! The source is an inharmonic oscillator bank: a fixed set of partials at
//! `freq * ratio[k]`, one original ratio set per family (the square roots
//! of the first few primes, `1, sqrt(2), sqrt(3), sqrt(5), ...`, not any
//! third-party table), each rendered with the shared `wave` shape. Metal
//! spreads its ratios away from unison by `metal-spread` (0 clustered, 1
//! the full authored spread); the hat's ratio set is fixed (no spread
//! control, per DDRUM-001's family-addition table). The bank is FM'd by
//! two independent modulators (`mod*`, `mod2*`) sharing one modulation
//! envelope (`mod-decay`, and `mod-slope` for metal only: the hat has no
//! `mod-slope` header, so its envelope is fixed fully exponential). A
//! small, fixed proportion of high-passed noise (no exposed level control)
//! is mixed into the bank output for the metallic/sizzling body.
//!
//! `hat-open`/`open-decay`/`closed-decay` (hat only): `hat-open` selects
//! which of `open-decay` (a long, ringing tail) or `closed-decay` (a
//! short, tight tail) is the voice's *base* decay; the shared `amp-decay`
//! control then scales that base relative to its own authored default
//! (`amp-decay / 0.3`), so `amp-decay`, `open-decay`, `closed-decay` and
//! `hat-open` are all independently audible: toggling `hat-open` swaps the
//! base tail, moving either `open-decay` or `closed-decay` rescales it
//! (whichever `hat-open` currently selects), and `amp-decay` scales
//! whichever tail is active. Metal has no `hat-open`/`open-decay`/
//! `closed-decay` header; its `amp-decay` behaves exactly like the tonal
//! family's.
//!
//! Hat choke (open/closed hi-hat exclusivity) uses the existing `cut`
//! voice-group control, not a mechanism added here (design-music 4.1:
//! "Open/closed hat choke uses the existing `cut` group control. It is not
//! a new knob.").
//!
//! State is fixed and preallocated (`STATE_FLOATS`, shared by both cores:
//! up to 8 partial phases, 2 modulator phases, the hit/voice clocks, the
//! LFO's phase and sample-and-hold value, the decimator's hold sample and
//! counter, one one-pole noise-coloring state and the repeat counter), so
//! neither core allocates.

use crate::dsp::effects::prim::{Rng, Shape};

use super::super::{Inp, Kx, NodeState, MAX_PORTS};
use super::chain::{
    self, amp_velocity_stage, apply_lfo_target, apply_velocity_target, bounded, decay_env,
    enum_index, filter_drive_decimate, lfo_init_phase, lfo_step, transient_stage, unit,
    wave_sample, Mods, RepeatAction,
};

/// The most partials either family's oscillator bank carries.
const MAX_PARTIALS: usize = 8;

/// Fixed preallocated per-voice state (see the module doc comment).
pub const STATE_FLOATS: usize = 18;

const PARTIAL_PHASE_BASE: usize = 0;
const MOD_PHASE: usize = 8;
const MOD2_PHASE: usize = 9;
const HIT_ELAPSED: usize = 10;
const VOICE_ELAPSED: usize = 11;
const LFO_PHASE: usize = 12;
const DECIM_HOLD: usize = 13;
const DECIM_COUNTER: usize = 14;
const LFO_RAND: usize = 15;
const TRANSIENT_COLOR: usize = 16;
const REPEATS_FIRED: usize = 17;

/// `amp-decay`'s authored default (control table row 119), the reference
/// `hat-open`'s base decay is scaled against.
const DEFAULT_AMP_DECAY: f32 = 0.3;

/// Original inharmonic ratio sets: the square roots of the first few
/// primes (metal: 2, 3, 5, 7, 11; hat: 3, 7, 13, 19 past the unison
/// fundamental), a simple, defensible, wholly original basis distinct from
/// any third-party synthesizer's authored table.
#[allow(clippy::approx_constant)]
const METAL_RATIOS: [f32; 6] = [
    1.0,
    1.414_213_6,
    1.732_050_8,
    2.236_068,
    2.645_751_3,
    3.316_624_8,
];
const HAT_RATIOS: [f32; 5] = [1.0, 1.732_050_8, 2.645_751_3, 3.605_551_3, 4.358_899];

/// The port layout of one family. Metal has `mod-slope`, `metal-spread`
/// and `repeat-count`/`repeat-time`; the hat has none of those but adds
/// `open-decay`/`closed-decay`/`hat-open` (DDRUM-001's family-addition
/// table).
struct Layout {
    freq: usize,
    velocity: usize,
    wave: usize,
    coarse: usize,
    fine: usize,
    mod_wave: usize,
    mod_freq: usize,
    mod_level: usize,
    mod2_wave: usize,
    mod2_freq: usize,
    mod2_level: usize,
    fm_depth: usize,
    mod_decay: usize,
    mod_slope: Option<usize>,
    metal_spread: Option<usize>,
    transient_wave: usize,
    transient_freq: usize,
    transient_level: usize,
    filter_type: usize,
    cutoff: usize,
    res: usize,
    filter_drive: usize,
    drive: usize,
    decimation: usize,
    amp_attack: usize,
    amp_decay: usize,
    amp_slope: usize,
    volume_velocity: usize,
    velocity_depth: usize,
    velocity_target: usize,
    lfo_wave: usize,
    lfo_rate: usize,
    lfo_depth: usize,
    lfo_offset: usize,
    lfo_target: usize,
    lfo_retrigger: usize,
    lfo_sync: usize,
    repeat_count: Option<usize>,
    repeat_time: Option<usize>,
    open_decay: Option<usize>,
    closed_decay: Option<usize>,
    hat_open: Option<usize>,
    cps: usize,
    onset_time: usize,
}

const METAL_LAYOUT: Layout = Layout {
    freq: 0,
    velocity: 1,
    wave: 2,
    coarse: 3,
    fine: 4,
    mod_wave: 5,
    mod_freq: 6,
    mod_level: 7,
    mod2_wave: 8,
    mod2_freq: 9,
    mod2_level: 10,
    fm_depth: 11,
    mod_decay: 12,
    mod_slope: Some(13),
    metal_spread: Some(14),
    transient_wave: 15,
    transient_freq: 16,
    transient_level: 17,
    filter_type: 18,
    cutoff: 19,
    res: 20,
    filter_drive: 21,
    drive: 22,
    decimation: 23,
    amp_attack: 24,
    amp_decay: 25,
    amp_slope: 26,
    volume_velocity: 27,
    velocity_depth: 28,
    velocity_target: 29,
    lfo_wave: 30,
    lfo_rate: 31,
    lfo_depth: 32,
    lfo_offset: 33,
    lfo_target: 34,
    lfo_retrigger: 35,
    lfo_sync: 36,
    repeat_count: Some(37),
    repeat_time: Some(38),
    open_decay: None,
    closed_decay: None,
    hat_open: None,
    cps: 39,
    onset_time: 40,
};

const HAT_LAYOUT: Layout = Layout {
    freq: 0,
    velocity: 1,
    wave: 2,
    coarse: 3,
    fine: 4,
    mod_wave: 5,
    mod_freq: 6,
    mod_level: 7,
    mod2_wave: 8,
    mod2_freq: 9,
    mod2_level: 10,
    fm_depth: 11,
    mod_decay: 12,
    mod_slope: None,
    metal_spread: None,
    transient_wave: 13,
    transient_freq: 14,
    transient_level: 15,
    filter_type: 16,
    cutoff: 17,
    res: 18,
    filter_drive: 19,
    drive: 20,
    decimation: 21,
    amp_attack: 22,
    amp_decay: 23,
    amp_slope: 24,
    volume_velocity: 25,
    velocity_depth: 26,
    velocity_target: 27,
    lfo_wave: 28,
    lfo_rate: 29,
    lfo_depth: 30,
    lfo_offset: 31,
    lfo_target: 32,
    lfo_retrigger: 33,
    lfo_sync: 34,
    repeat_count: None,
    repeat_time: None,
    open_decay: Some(35),
    closed_decay: Some(36),
    hat_open: Some(37),
    cps: 38,
    onset_time: 39,
};

/// Renders one block of the shared metal/hat chain.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn render(
    layout: &Layout,
    ratios: &'static [f32],
    noise_freq: f32,
    noise_amt: f32,
    default_freq: f32,
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
    let n_partials = ratios.len().min(MAX_PARTIALS);
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0x6D65_7461).s;
        st.u[1] = 1;
        let sync = ins[layout.lfo_sync].first() >= 0.5;
        let cps = bounded(ins[layout.cps].first(), 0.01, 100.0, 0.5);
        let rate = bounded(ins[layout.lfo_rate].first(), 0.0, 1000.0, 4.0);
        let onset = bounded(ins[layout.onset_time].first(), 0.0, 1.0e9, 0.0);
        let retrigger = ins[layout.lfo_retrigger].first() >= 0.5;
        let offset = unit(ins[layout.lfo_offset].first(), 0.0);
        mem[LFO_PHASE] = lfo_init_phase(retrigger, sync, cps, rate, onset, offset);
    }
    let mut rng = Rng::new(st.u[0]);

    for (i, sample) in out.iter_mut().enumerate() {
        if st.done() {
            *sample = 0.0;
            continue;
        }
        let freq = bounded(ins[layout.freq].at(i), 20.0, sr * 0.45, default_freq);
        let velocity = unit(ins[layout.velocity].at(i), 1.0);
        let wave = enum_index(ins[layout.wave].at(i), 4);
        let coarse = bounded(ins[layout.coarse].at(i), -24.0, 24.0, 0.0);
        let fine = bounded(ins[layout.fine].at(i), -100.0, 100.0, 0.0);
        let mod_wave = enum_index(ins[layout.mod_wave].at(i), 4);
        let mod_freq_ratio = bounded(ins[layout.mod_freq].at(i), 0.05, 32.0, 2.0);
        let mod_level = unit(ins[layout.mod_level].at(i), 0.5);
        let mod2_wave = enum_index(ins[layout.mod2_wave].at(i), 4);
        let mod2_freq_ratio = bounded(ins[layout.mod2_freq].at(i), 0.05, 32.0, 3.0);
        let mod2_level = unit(ins[layout.mod2_level].at(i), 0.3);
        let fm_depth = bounded(ins[layout.fm_depth].at(i), 0.0, 16.0, 1.0);
        let mod_decay = bounded(ins[layout.mod_decay].at(i), 0.001, 8.0, 0.15);
        let mod_slope = layout
            .mod_slope
            .map_or(1.0, |idx| unit(ins[idx].at(i), 0.5));
        let transient_wave = enum_index(ins[layout.transient_wave].at(i), 3);
        let transient_freq = bounded(ins[layout.transient_freq].at(i), 20.0, sr * 0.45, 4000.0);
        let transient_level = bounded(ins[layout.transient_level].at(i), 0.0, 4.0, 0.3);
        let filter_type = enum_index(ins[layout.filter_type].at(i), 4);
        let cutoff = bounded(ins[layout.cutoff].at(i), 40.0, sr * 0.45, 1200.0);
        let res = bounded(ins[layout.res].at(i), 0.0, 4.0, 0.3);
        let filter_drive = unit(ins[layout.filter_drive].at(i), 0.0);
        let drive = unit(ins[layout.drive].at(i), 0.0);
        let decimation = unit(ins[layout.decimation].at(i), 0.0);
        let amp_attack = bounded(ins[layout.amp_attack].at(i), 0.0, 4.0, 0.001);
        let amp_decay_raw = bounded(ins[layout.amp_decay].at(i), 0.005, 16.0, 0.3);
        let amp_slope = unit(ins[layout.amp_slope].at(i), 0.6);
        let volume_velocity = unit(ins[layout.volume_velocity].at(i), 1.0);
        let velocity_depth = bounded(ins[layout.velocity_depth].at(i), -4.0, 4.0, 0.0);
        let velocity_target = enum_index(ins[layout.velocity_target].at(i), 6);
        let lfo_wave = enum_index(ins[layout.lfo_wave].at(i), 5);
        let lfo_rate = bounded(ins[layout.lfo_rate].at(i), 0.0, 1000.0, 4.0);
        let lfo_depth = bounded(ins[layout.lfo_depth].at(i), -4.0, 4.0, 0.0);
        let lfo_target = enum_index(ins[layout.lfo_target].at(i), 9);
        let lfo_sync = ins[layout.lfo_sync].at(i) >= 0.5;
        let cps = bounded(ins[layout.cps].at(i), 0.01, 100.0, 0.5);
        let repeat_count = layout
            .repeat_count
            .map_or(0, |idx| enum_index(ins[idx].at(i), 8).max(0));
        let repeat_time = layout
            .repeat_time
            .map_or(0.1, |idx| bounded(ins[idx].at(i), 0.005, 8.0, 0.1));

        // `hat-open`/`open-decay`/`closed-decay` (hat only): see the
        // module doc comment for the exact relationship to `amp-decay`.
        let amp_decay = if let (Some(oi), Some(ci), Some(hi)) =
            (layout.open_decay, layout.closed_decay, layout.hat_open)
        {
            let open_d = bounded(ins[oi].at(i), 0.05, 4.0, 0.6);
            let closed_d = bounded(ins[ci].at(i), 0.01, 1.0, 0.08);
            let hat_open = ins[hi].at(i) >= 0.5;
            let base = if hat_open { open_d } else { closed_d };
            (base * (amp_decay_raw / DEFAULT_AMP_DECAY)).clamp(0.005, 16.0)
        } else {
            amp_decay_raw
        };

        // The per-voice LFO and the typed velocity/LFO routes.
        let (lfo_val, next_lfo_phase, next_lfo_rand) = lfo_step(
            lfo_wave,
            lfo_rate,
            lfo_sync,
            cps,
            sr,
            mem[LFO_PHASE],
            mem[LFO_RAND],
            &mut rng,
        );
        mem[LFO_PHASE] = next_lfo_phase;
        mem[LFO_RAND] = next_lfo_rand;

        let mut mods = Mods::default();
        apply_velocity_target(velocity_target, velocity_depth * velocity, &mut mods);
        apply_lfo_target(lfo_target, lfo_depth * lfo_val, &mut mods);

        let te = mem[HIT_ELAPSED];
        let semis = coarse + fine / 100.0 + mods.pitch;
        let tuned = bounded(freq * 2f32.powf(semis / 12.0), 15.0, sr * 0.45, freq);
        let mod_hz = bounded(tuned * mod_freq_ratio, 0.5, sr * 0.45, tuned);
        let mod2_hz = bounded(tuned * mod2_freq_ratio, 0.5, sr * 0.45, tuned);

        let mod_sample = wave_sample(mod_wave, mem[MOD_PHASE]);
        let mod2_sample = wave_sample(mod2_wave, mem[MOD2_PHASE]);
        let mod_env = decay_env(te, mod_decay, mod_slope);
        let fm_depth_eff = (fm_depth * (1.0 + mods.mod_depth)).max(0.0);
        let fm_amount =
            fm_depth_eff * mod_env * 0.5 * (mod_level * mod_sample + mod2_level * mod2_sample);

        let mut bank_sum = 0.0f32;
        for k in 0..n_partials {
            let base_ratio = ratios[k];
            let eff_ratio = if let Some(idx) = layout.metal_spread {
                let spread = unit(ins[idx].at(i), 0.3);
                let scale = 0.35 + 0.9 * spread;
                1.0 + (base_ratio - 1.0) * scale
            } else {
                base_ratio
            };
            let partial_freq = bounded(tuned * eff_ratio, 15.0, sr * 0.45, tuned);
            let phase = mem[PARTIAL_PHASE_BASE + k];
            bank_sum += wave_sample(wave, phase + fm_amount);
            mem[PARTIAL_PHASE_BASE + k] = (phase + partial_freq / sr).rem_euclid(1.0);
        }
        #[allow(clippy::cast_precision_loss)]
        let bank_out = bank_sum / n_partials as f32;

        // High-passed noise, a small fixed proportion of the source (the
        // module doc comment): no exposed level control for this family.
        st.bq[1].set(Shape::Highpass, noise_freq, 0.7, 0.0, sr);
        let noise_sample = st.bq[1].run(rng.bipolar());
        let tone = bank_out * (1.0 - noise_amt) + noise_sample * noise_amt;

        let transient_level_eff = (transient_level + mods.transient).clamp(0.0, 4.0);
        let (transient_out, next_color) = transient_stage(
            transient_wave,
            transient_freq,
            transient_level_eff,
            te,
            mem[TRANSIENT_COLOR],
            &mut rng,
            sr,
        );
        mem[TRANSIENT_COLOR] = next_color;

        let raw = (tone + transient_out).clamp(-4.0, 4.0);

        let (decimated, next_hold, next_counter) = filter_drive_decimate(
            raw,
            filter_type,
            cutoff,
            res,
            filter_drive,
            drive,
            decimation,
            &mods,
            &mut st.bq[0],
            mem[DECIM_HOLD],
            mem[DECIM_COUNTER],
            sr,
        );
        mem[DECIM_HOLD] = next_hold;
        mem[DECIM_COUNTER] = next_counter;

        let (level, amp_decay_eff) = amp_velocity_stage(
            te,
            amp_attack,
            amp_decay,
            amp_slope,
            &mods,
            volume_velocity,
            velocity,
            decimated,
        );
        *sample = level;

        mem[MOD_PHASE] = (mem[MOD_PHASE] + mod_hz / sr).rem_euclid(1.0);
        mem[MOD2_PHASE] = (mem[MOD2_PHASE] + mod2_hz / sr).rem_euclid(1.0);
        mem[HIT_ELAPSED] += 1.0 / sr;
        mem[VOICE_ELAPSED] += 1.0 / sr;

        let longest_decay = amp_decay_eff.max(mod_decay);
        let (action, rf, he) = chain::repeat_or_finish(
            repeat_count,
            repeat_time,
            mem[REPEATS_FIRED],
            mem[HIT_ELAPSED],
            mem[VOICE_ELAPSED],
            longest_decay,
            st,
        );
        mem[REPEATS_FIRED] = rf;
        mem[HIT_ELAPSED] = he;
        if let RepeatAction::Retrigger = action {
            for k in 0..n_partials {
                mem[PARTIAL_PHASE_BASE + k] = 0.0;
            }
            mem[MOD_PHASE] = 0.0;
            mem[MOD2_PHASE] = 0.0;
            if ins[layout.lfo_retrigger].at(i) >= 0.5 {
                mem[LFO_PHASE] = unit(ins[layout.lfo_offset].at(i), 0.0);
            }
        }
    }
    st.u[0] = rng.s;
}

/// `digital-metal-core`: the cymbal voice (6-partial inharmonic bank,
/// spread by `metal-spread`, FM'd by two modulators).
pub fn render_metal(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render(
        &METAL_LAYOUT,
        &METAL_RATIOS,
        6000.0,
        0.12,
        300.0,
        ins,
        st,
        mem,
        out,
        kx,
    );
}

/// `digital-hat-core`: the hat voice (5-partial fixed inharmonic bank, FM'd
/// by two modulators, `hat-open`-selected decay).
pub fn render_hat(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render(
        &HAT_LAYOUT,
        &HAT_RATIOS,
        8000.0,
        0.22,
        400.0,
        ins,
        st,
        mem,
        out,
        kx,
    );
}
