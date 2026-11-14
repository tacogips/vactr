//! `digital-drum-core` and `digital-snare-core`: the tonal drum/snare
//! kernel (design-music 4.1, DDRUM-001/003), built on the shared chain in
//! `chain.rs` (transient, filter/drive/decimation, amplitude envelope +
//! velocity, per-voice LFO, repeat/voice-end).
//!
//! `digital-drum-core` and `digital-snare-core` share one `render`
//! implementation, parameterized by a compile-time [`Layout`] (the two
//! cores differ only in whether a band-passed noise stage sits between the
//! FM section and the transient, and in the resulting port offsets): a
//! phase-modulated carrier with its own pitch envelope, an FM modulator
//! with its own envelope, an onset transient, an optional noise band (the
//! snare only), then the shared chain. `repeat-count`/`repeat-time`
//! retrigger the pitch/mod/transient/amplitude envelopes (not the
//! filter/LFO state) inside one voice.

use crate::dsp::effects::prim::{Rng, Shape};

use super::super::{Inp, Kx, NodeState, MAX_PORTS};
use super::chain::{
    self, amp_velocity_stage, apply_lfo_target, apply_velocity_target, bounded, decay_env,
    enum_index, filter_drive_decimate, lfo_init_phase, lfo_step, transient_stage, unit,
    wave_sample, Mods, RepeatAction,
};

/// Fixed preallocated per-voice state: oscillator phases, envelope clocks,
/// the LFO's phase and sample-and-hold value, the decimator's hold sample
/// and counter, one one-pole noise-coloring state and the repeat counter.
pub const STATE_FLOATS: usize = 12;

const CARRIER_PHASE: usize = 0;
const MOD_PHASE: usize = 1;
const HIT_ELAPSED: usize = 2;
const VOICE_ELAPSED: usize = 3;
const LFO_PHASE: usize = 4;
const DECIM_HOLD: usize = 5;
const DECIM_COUNTER: usize = 6;
const LFO_RAND: usize = 7;
const TRANSIENT_COLOR: usize = 8;
const REPEATS_FIRED: usize = 10;

/// The port layout of one family: the two cores share every index except
/// the snare's two extra noise ports, which shift everything after them.
struct Layout {
    freq: usize,
    velocity: usize,
    wave: usize,
    coarse: usize,
    fine: usize,
    mod_wave: usize,
    mod_freq: usize,
    mod_level: usize,
    fm_depth: usize,
    pitch_decay: usize,
    pitch_depth: usize,
    pitch_slope: usize,
    osc_mix: usize,
    mod_decay: usize,
    mod_slope: usize,
    noise_freq: Option<usize>,
    noise_mix: Option<usize>,
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
    repeat_count: usize,
    repeat_time: usize,
    cps: usize,
    onset_time: usize,
}

const DRUM_LAYOUT: Layout = Layout {
    freq: 0,
    velocity: 1,
    wave: 2,
    coarse: 3,
    fine: 4,
    mod_wave: 5,
    mod_freq: 6,
    mod_level: 7,
    fm_depth: 8,
    pitch_decay: 9,
    pitch_depth: 10,
    pitch_slope: 11,
    osc_mix: 12,
    mod_decay: 13,
    mod_slope: 14,
    noise_freq: None,
    noise_mix: None,
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
    repeat_count: 37,
    repeat_time: 38,
    cps: 39,
    onset_time: 40,
};

const SNARE_LAYOUT: Layout = Layout {
    freq: 0,
    velocity: 1,
    wave: 2,
    coarse: 3,
    fine: 4,
    mod_wave: 5,
    mod_freq: 6,
    mod_level: 7,
    fm_depth: 8,
    pitch_decay: 9,
    pitch_depth: 10,
    pitch_slope: 11,
    osc_mix: 12,
    mod_decay: 13,
    mod_slope: 14,
    noise_freq: Some(15),
    noise_mix: Some(16),
    transient_wave: 17,
    transient_freq: 18,
    transient_level: 19,
    filter_type: 20,
    cutoff: 21,
    res: 22,
    filter_drive: 23,
    drive: 24,
    decimation: 25,
    amp_attack: 26,
    amp_decay: 27,
    amp_slope: 28,
    volume_velocity: 29,
    velocity_depth: 30,
    velocity_target: 31,
    lfo_wave: 32,
    lfo_rate: 33,
    lfo_depth: 34,
    lfo_offset: 35,
    lfo_target: 36,
    lfo_retrigger: 37,
    lfo_sync: 38,
    repeat_count: 39,
    repeat_time: 40,
    cps: 41,
    onset_time: 42,
};

/// Renders one block of the shared drum/snare chain.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn render(
    layout: &Layout,
    has_noise: bool,
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
    if st.u[1] == 0 {
        st.u[0] = Rng::new(kx.seed ^ 0x6472_756D).s;
        st.u[1] = 1;
        // `lfo-retrigger: false`: anchor the free-running LFO phase to the
        // event's absolute host time (DDRUM-002A/003), so independently
        // triggered voices at the same rate share phase.
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
        let freq = bounded(ins[layout.freq].at(i), 20.0, sr * 0.45, 110.0);
        let velocity = unit(ins[layout.velocity].at(i), 1.0);
        let wave = enum_index(ins[layout.wave].at(i), 4);
        let coarse = bounded(ins[layout.coarse].at(i), -24.0, 24.0, 0.0);
        let fine = bounded(ins[layout.fine].at(i), -100.0, 100.0, 0.0);
        let mod_wave = enum_index(ins[layout.mod_wave].at(i), 4);
        let mod_freq_ratio = bounded(ins[layout.mod_freq].at(i), 0.05, 32.0, 2.0);
        let mod_level = unit(ins[layout.mod_level].at(i), 0.5);
        let fm_depth = bounded(ins[layout.fm_depth].at(i), 0.0, 16.0, 1.0);
        let pitch_decay = bounded(ins[layout.pitch_decay].at(i), 0.001, 8.0, 0.05);
        let pitch_depth = bounded(ins[layout.pitch_depth].at(i), -96.0, 96.0, 12.0);
        let pitch_slope = unit(ins[layout.pitch_slope].at(i), 0.5);
        let osc_mix = unit(ins[layout.osc_mix].at(i), 0.5);
        let mod_decay = bounded(ins[layout.mod_decay].at(i), 0.001, 8.0, 0.15);
        let mod_slope = unit(ins[layout.mod_slope].at(i), 0.5);
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
        let amp_decay = bounded(ins[layout.amp_decay].at(i), 0.005, 16.0, 0.3);
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
        let repeat_count = enum_index(ins[layout.repeat_count].at(i), 8).max(0);
        let repeat_time = bounded(ins[layout.repeat_time].at(i), 0.005, 8.0, 0.1);
        let (noise_freq, noise_mix) = if has_noise {
            (
                bounded(
                    ins[layout.noise_freq.unwrap()].at(i),
                    100.0,
                    sr * 0.45,
                    2500.0,
                ),
                unit(ins[layout.noise_mix.unwrap()].at(i), 0.3),
            )
        } else {
            (2500.0, 0.0)
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
        let pitch_env = decay_env(te, pitch_decay, pitch_slope) * pitch_depth;
        let semis = coarse + fine / 100.0 + pitch_env + mods.pitch;
        let tuned = bounded(freq * 2f32.powf(semis / 12.0), 15.0, sr * 0.45, freq);
        let mod_hz = bounded(tuned * mod_freq_ratio, 0.5, sr * 0.45, tuned);

        let mod_sample = wave_sample(mod_wave, mem[MOD_PHASE]);
        let mod_env = decay_env(te, mod_decay, mod_slope);
        let fm_depth_eff = (fm_depth * (1.0 + mods.mod_depth)).max(0.0);
        let fm_amount = fm_depth_eff * mod_level * mod_env * 0.5;
        let carrier_sample = wave_sample(wave, mem[CARRIER_PHASE] + fm_amount * mod_sample);
        let mut tone = (1.0 - osc_mix) * carrier_sample + osc_mix * mod_sample;

        if has_noise {
            let alpha = 1.0 - (-std::f32::consts::TAU * noise_freq / sr).exp();
            st.bq[1].set(Shape::Bandpass, noise_freq, 2.0, 0.0, sr);
            let filtered = st.bq[1].run(rng.bipolar() * alpha.sqrt().max(0.2));
            tone = tone * (1.0 - noise_mix) + filtered * noise_mix;
        }

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

        mem[CARRIER_PHASE] = (mem[CARRIER_PHASE] + tuned / sr).rem_euclid(1.0);
        mem[MOD_PHASE] = (mem[MOD_PHASE] + mod_hz / sr).rem_euclid(1.0);
        mem[HIT_ELAPSED] += 1.0 / sr;
        mem[VOICE_ELAPSED] += 1.0 / sr;

        let longest_decay = amp_decay_eff.max(mod_decay).max(pitch_decay);
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
            mem[CARRIER_PHASE] = 0.0;
            mem[MOD_PHASE] = 0.0;
            if ins[layout.lfo_retrigger].at(i) >= 0.5 {
                mem[LFO_PHASE] = unit(ins[layout.lfo_offset].at(i), 0.0);
            }
        }
    }
    st.u[0] = rng.s;
}

/// `digital-drum-core`: the tonal voice (carrier + FM modulator, no noise
/// band).
pub fn render_drum(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render(&DRUM_LAYOUT, false, ins, st, mem, out, kx);
}

/// `digital-snare-core`: the tonal voice plus a band-passed noise stage.
pub fn render_snare(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    render(&SNARE_LAYOUT, true, ins, st, mem, out, kx);
}
