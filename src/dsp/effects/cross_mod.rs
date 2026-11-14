//! Analytic two-input/two-output modulation effect. The mode vocabulary and
//! main/aux routing are informed by Emilie Gillet's MIT-licensed Warps DSP
//! (revision 08460a69). The 20-band vocoder uses translated 96-kHz source
//! filter, follower, formant and SRC stages with an authored host-rate FIR
//! boundary. Cleared analytic XMOD equations are translated, while the carrier
//! uses analytic sine and table-free BLEP paths. XMOD has a cleared 6× FIR
//! converter. Neither path is a complete firmware port. See
//! THIRD_PARTY_NOTICES.md.

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};

use super::{FxCtx, FxState, ParamDef};

#[path = "cross_mod_vocoder_boundary.rs"]
mod boundary;
#[path = "cross_mod_limiter.rs"]
mod limiter;
#[path = "cross_mod_osc.rs"]
mod osc;
#[path = "cross_mod_src.rs"]
mod src;
#[path = "cross_mod_vocoder.rs"]
mod vocoder;
#[path = "cross_mod_vocoder_bank.rs"]
mod vocoder_bank;
#[path = "cross_mod_vocoder_fir.rs"]
mod vocoder_fir;

/// Required effect memory, including the host-rate FIR boundary if needed.
pub fn mem_len(sr: f32) -> usize {
    src::MEM_LEN + vocoder::MEM_LEN + boundary::mem_len(sr)
}

#[cfg(test)]
pub(crate) fn vocoder_envelope_energy(mem: &[f32]) -> f32 {
    vocoder::envelope_energy(&mem[src::MEM_LEN..src::MEM_LEN + vocoder::MEM_LEN])
}

pub const PARAMS: &[ParamDef] = &[
    ParamDef::new("algorithm", 0.0, 0.0, 8.0),
    ParamDef::new("timbre", 0.5, 0.0, 1.0),
    ParamDef::new("drive", 0.2, 0.0, 1.0),
    ParamDef::new("carrier-wave", 0.0, 0.0, 6.0),
    ParamDef::unit("carrier-frequency", 220.0, 20.0, 10_000.0, "Hz"),
    ParamDef::new("carrier-drive", 1.0, 0.0, 1.0),
    ParamDef::new("modulator-drive", 1.0, 0.0, 1.0),
];

/// A fresh effect state and zeroed, fixed-size filter-bank memory.
pub fn init(st: &mut FxState, mem: &mut [f32], sr: f32) {
    st.s.fill(0.0);
    st.s[limiter::PEAK_SLOT] = limiter::INITIAL_PEAK;
    st.s[27] = vocoder_bank::BLOCK as f32;
    mem.fill(0.0);
    if mem.len() < mem_len(sr) {
        return;
    }
    let (_, remaining) = mem.split_at_mut(src::MEM_LEN);
    let (_, boundary_mem) = remaining.split_at_mut(vocoder::MEM_LEN);
    boundary::init(boundary_mem, sr);
}

// FxState.s: 0 oscillator phase, 1 carrier-noise filter, 2..6 carrier amp,
// 6..10 modulator amp, 24 downsampling ring cursor, 25 vocoder limiter peak.
// Each amplifier stores
// level, drive, pre-gain, post-gain.
// Source gain equations are MIT Warps/stmlib; the smoothing is host-rate,
// sample-by-sample instead of source block interpolation.
fn source_soft_clip(x: f32) -> f32 {
    if x <= -3.0 {
        -1.0
    } else if x >= 3.0 {
        1.0
    } else {
        let square = x * x;
        x * (27.0 + square) / (27.0 + 9.0 * square)
    }
}

fn amplifier_gains(drive: f32) -> (f32, f32) {
    let square = drive * drive;
    let pre_a = drive * 0.5;
    let pre_b = square * square * drive * 24.0;
    let pre = pre_a + (pre_b - pre_a) * square;
    let squished = drive * (2.0 - drive);
    let post = 1.0 / source_soft_clip(0.33 + squished * (pre - 0.33));
    (pre, post)
}

fn amplify(
    state: &mut [f32; 4],
    input: f32,
    target_drive: f32,
    limit: f32,
    smooth: f32,
    attack: f32,
    decay: f32,
) -> (f32, f32) {
    // The source receives signed 16-bit ADC samples. Bound direct host floats
    // to that normalized domain and keep invalid input out of the gate state.
    let mut sample = if input.is_finite() {
        input.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let error = sample * sample - state[0];
    state[0] += error * if error > 0.0 { attack } else { decay };
    sample *= (state[0] / 0.0001).min(1.0);

    let (pre_target, post_target) = amplifier_gains(target_drive);
    state[1] += smooth * (target_drive - state[1]);
    state[2] += smooth * (pre_target - state[2]);
    state[3] += smooth * (post_target - state[3]);
    let raw = sample * state[1];
    let pre = sample * state[2];
    let post = source_soft_clip(pre) * state[3];
    (pre + (post - pre) * limit, raw)
}

fn fold(x: f32) -> f32 {
    let phase = (x + 1.0).rem_euclid(4.0);
    if phase < 2.0 {
        phase - 1.0
    } else {
        3.0 - phase
    }
}

fn xor_sample(a: f32, b: f32) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let x = (a * 32768.0).clamp(-32768.0, 32767.0) as i16;
    #[allow(clippy::cast_possible_truncation)]
    let y = (b * 32768.0).clamp(-32768.0, 32767.0) as i16;
    f32::from(x ^ y) / 32768.0
}

fn crossfade(modulator: f32, carrier: f32, timbre: f32) -> f32 {
    let angle = (1.04 * timbre - 0.02).clamp(0.0, 1.0) * FRAC_PI_2;
    (modulator * angle.sin() + carrier * angle.cos()) * FRAC_1_SQRT_2
}

fn auxiliary_output(wave: f32, carrier_raw: f32, raw_modulator: f32) -> f32 {
    if wave < 0.5 {
        (carrier_raw + raw_modulator) * 0.5
    } else if wave < 3.5 {
        carrier_raw * 0.5
    } else {
        carrier_raw
    }
}

// The MIT Warps diode approximation cites Julian Parker's DAFx-11 model.
fn diode(x: f32) -> f32 {
    let dead = (x.abs() - 0.667).max(0.0);
    x.signum() * 0.172_990_64 * dead * dead
}

fn soft_limit(x: f32) -> f32 {
    let square = x * x;
    x * (27.0 + square) / (27.0 + 9.0 * square)
}

fn comparator(modulator: f32, carrier: f32, timbre: f32) -> f32 {
    let options = [
        modulator.min(carrier),
        if carrier > 0.05 { carrier } else { modulator },
        if modulator.abs() > carrier.abs() {
            modulator
        } else {
            carrier
        },
        if modulator.abs() > carrier.abs() {
            modulator.abs()
        } else {
            -carrier.abs()
        },
    ];
    let position = (timbre * 2.995).clamp(0.0, 2.995);
    let index = position.floor() as usize;
    options[index] + (options[index + 1] - options[index]) * position.fract()
}

// Source ProcessXmod passes modulator first, carrier second. Fold and
// vocoder remain Vactr adaptations; timbre is ignored in source modes
// where it has no role only after their own source parameter mapping.
fn mode(n: usize, carrier: f32, modulator: f32, timbre: f32) -> f32 {
    match n {
        0 => crossfade(modulator, carrier, timbre),
        1 => fold((carrier + modulator + carrier * modulator * 0.25) * (0.5 + timbre * 2.5)),
        2 => {
            let ring = (diode(modulator + 2.0 * carrier) + diode(modulator - 2.0 * carrier))
                * (4.0 + 24.0 * timbre);
            soft_limit(ring)
        }
        3 => {
            let ring = 4.0 * modulator * carrier * (1.0 + 8.0 * timbre);
            ring / (1.0 + ring.abs())
        }
        4 => {
            let sum = 0.7 * (modulator + carrier);
            sum + (xor_sample(modulator, carrier) - sum) * timbre
        }
        5 => comparator(modulator, carrier, timbre),
        // The final source XMOD slot transitions comparator to NOP.
        _ => modulator,
    }
}

fn skewed_timbre(algorithm: f32, timbre: f32) -> f32 {
    let source_position = algorithm.clamp(0.0, 6.0) / 8.0;
    timbre * (1.0 + source_position * (timbre - 1.0))
}

fn vocoder_amount(algorithm: f32) -> f32 {
    ((algorithm.clamp(0.0, 8.0) / 8.0 - 0.7) * 20.0 + 0.5).clamp(0.0, 1.0)
}

fn vocoder_release(algorithm: f32) -> f32 {
    let position = algorithm.clamp(0.0, 8.0) / 8.0;
    let linear = (4.0 * (position - 0.75)).clamp(0.0, 1.0);
    linear * (2.0 - linear)
}

fn xmod_output(algorithm: f32, carrier: f32, modulator: f32, timbre: f32) -> f32 {
    let position = algorithm.clamp(0.0, 5.999);
    let base = position.floor() as usize;
    let blend = position.fract();
    let parameter = skewed_timbre(algorithm, timbre);
    let first = mode(base, carrier, modulator, parameter);
    let second = mode(base + 1, carrier, modulator, parameter);
    first + (second - first) * blend
}

fn bridge(xmod: f32, vocoder: f32, modulator: f32, amount: f32) -> f32 {
    let selected = if amount < 0.5 { xmod } else { vocoder };
    let transition = 2.0 * amount.min(1.0 - amount);
    selected + transition * (modulator - selected)
}

#[derive(Clone, Copy)]
struct CarrierControl {
    wave: u8,
    base: f32,
    external: f32,
    sr: f32,
    amount: f32,
}

fn internal_pair(
    xmod: &mut osc::Oscillator,
    vocoder: &mut osc::Oscillator,
    rng: &mut super::prim::Rng,
    control: CarrierControl,
) -> (f32, f32) {
    let xmod_sample = if control.amount < 0.5 {
        match control.wave {
            1 => xmod.sine(control.base, control.external, control.sr),
            2 => {
                xmod.polyblep(1, control.base, control.external, control.sr)
                    .0
            }
            _ => {
                xmod.polyblep(2, control.base, control.external, control.sr)
                    .0
            }
        }
    } else {
        0.0
    };
    let (vocoder_sample, vocoder_gain) = if control.amount > 0.0 {
        match control.wave {
            1 => vocoder.polyblep(2, control.base, control.external, control.sr),
            2 => vocoder.polyblep(3, control.base, control.external, control.sr),
            _ => (
                vocoder.noise(control.base, control.external, control.sr, rng),
                1.0,
            ),
        }
    } else {
        (0.0, 1.0)
    };
    if control.amount == 0.0 {
        (xmod_sample * 0.5, xmod_sample)
    } else if control.amount >= 0.5 {
        (vocoder_sample * vocoder_gain, vocoder_sample)
    } else {
        let balance = 2.0 * control.amount;
        let raw = xmod_sample + (vocoder_sample - xmod_sample) * balance;
        let processed =
            xmod_sample * 0.5 + (vocoder_sample * vocoder_gain - xmod_sample * 0.5) * balance;
        (processed, raw)
    }
}

/// L is the external carrier, R the modulator. L becomes the main output;
/// R becomes the auxiliary sum or the internal carrier when enabled.
pub fn process(
    p: &[f32],
    st: &mut FxState,
    mem: &mut [f32],
    l: &mut [f32],
    r: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    let sr = ctx.sr.max(1.0);
    if mem.len() < mem_len(sr) {
        l.fill(0.0);
        r.fill(0.0);
        return;
    }
    let (src_mem, remaining) = mem.split_at_mut(src::MEM_LEN);
    let (source_vocoder_mem, boundary_mem) = remaining.split_at_mut(vocoder::MEM_LEN);
    let algorithm = p[0].clamp(0.0, 8.0);
    let timbre = p[1].clamp(0.0, 1.0);
    let drive = p[2].clamp(0.0, 1.0);
    let wave = p[3].clamp(0.0, 6.0);
    let carrier_drive = drive * p[5].clamp(0.0, 1.0);
    let modulator_drive = drive * p[6].clamp(0.0, 1.0);
    let freq = p[4].clamp(20.0, (sr * 0.45).max(20.0));
    let mut phase = st.s[0];
    let mut noise_lp = st.s[1];
    let mut carrier_amp = [st.s[2], st.s[3], st.s[4], st.s[5]];
    let mut modulator_amp = [st.s[6], st.s[7], st.s[8], st.s[9]];
    let mut xmod_osc = osc::Oscillator::load(&st.s, osc::XMOD_OFFSET, osc::XMOD_STARTUP_OFFSET);
    let mut vocoder_osc =
        osc::Oscillator::load(&st.s, osc::VOCODER_OFFSET, osc::VOCODER_STARTUP_OFFSET);
    let mut src_cursor = st.s[24] as usize;
    let mut source_write = st.s[26] as usize;
    let mut source_read = st.s[27] as usize;
    let mut vocoder_limiter = limiter::Limiter::new(st.s[limiter::PEAK_SLOT], 96_000.0);
    let vocoder_amount = vocoder_amount(algorithm);
    // Reference amp coefficients apply at its 96 kHz processing rate. The
    // authored 0.75 ms gain smoothing avoids callback-size-dependent ramps.
    let smooth = 1.0 - (-1.0 / (0.00075 * sr)).exp();
    let attack = 1.0 - 0.9f32.powf(96_000.0 / sr);
    let decay = 1.0 - 0.9999f32.powf(96_000.0 / sr);
    let source_release = vocoder_release(algorithm);
    for (main, aux) in l.iter_mut().zip(r.iter_mut()) {
        let external = *main;
        let (modulator, raw_modulator) = amplify(
            &mut modulator_amp,
            *aux,
            modulator_drive,
            1.0 - vocoder_amount,
            smooth,
            attack,
            decay,
        );
        phase = (phase + freq / sr).fract();
        let (carrier, carrier_raw) = if wave < 0.5 {
            amplify(
                &mut carrier_amp,
                external,
                carrier_drive,
                1.0 - vocoder_amount,
                smooth,
                attack,
                decay,
            )
        } else if wave < 3.5 {
            internal_pair(
                &mut xmod_osc,
                &mut vocoder_osc,
                &mut st.rng,
                CarrierControl {
                    wave: wave.round() as u8,
                    base: freq / sr,
                    external: external.clamp(-1.0, 1.0),
                    sr,
                    amount: vocoder_amount,
                },
            )
        } else if wave < 4.5 {
            // The public pulse selector remains a Vactr extension.
            let raw = if phase < 0.5 { 1.0 } else { -1.0 };
            (raw, raw)
        } else if wave < 5.5 {
            // A Vactr-generated low-passed noise carrier.
            let a = 1.0 - (-TAU * freq / sr).exp();
            noise_lp += a * (st.rng.bipolar() - noise_lp);
            (noise_lp, noise_lp)
        } else {
            // Carrier-input phase modulation of an internal sine.
            let raw = (TAU * (phase + external * timbre * 0.45)).sin();
            (raw, raw)
        };
        let voc = if vocoder_amount >= 0.5 {
            if sr == 96_000.0 {
                vocoder_limiter.process(vocoder::sample(
                    source_vocoder_mem,
                    &mut source_write,
                    &mut source_read,
                    carrier,
                    modulator,
                    timbre,
                    source_release,
                ))
            } else {
                boundary::sample(boundary_mem, sr, carrier, modulator, |c, m| {
                    vocoder_limiter.process(vocoder::sample(
                        source_vocoder_mem,
                        &mut source_write,
                        &mut source_read,
                        c,
                        m,
                        timbre,
                        source_release,
                    ))
                })
            }
        } else {
            0.0
        };
        let xmod = if vocoder_amount < 0.5 {
            src::process(
                src_mem,
                &mut src_cursor,
                carrier,
                modulator,
                |up_carrier, up_modulator| xmod_output(algorithm, up_carrier, up_modulator, timbre),
            )
        } else {
            0.0
        };
        *main = bridge(xmod, voc, modulator, vocoder_amount).clamp(-1.0, 1.0);
        *aux = auxiliary_output(wave, carrier_raw, raw_modulator);
    }
    st.s[0] = phase;
    st.s[1] = noise_lp;
    st.s[2..6].copy_from_slice(&carrier_amp);
    st.s[6..10].copy_from_slice(&modulator_amp);
    xmod_osc.save(&mut st.s, osc::XMOD_OFFSET, osc::XMOD_STARTUP_OFFSET);
    vocoder_osc.save(&mut st.s, osc::VOCODER_OFFSET, osc::VOCODER_STARTUP_OFFSET);
    st.s[24] = src_cursor as f32;
    st.s[26] = source_write as f32;
    st.s[27] = source_read as f32;
    st.s[limiter::PEAK_SLOT] = vocoder_limiter.peak;
}

#[cfg(test)]
mod tests {
    use super::{
        amplifier_gains, amplify, auxiliary_output, bridge, comparator, crossfade, diode,
        internal_pair, mode, osc, skewed_timbre, soft_limit, source_soft_clip, vocoder_amount,
        vocoder_release, xmod_output, xor_sample, CarrierControl,
    };
    use std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn source_internal_carrier_pair_routes_raw_aux_and_gain_at_transition() {
        let mut x = osc::Oscillator::load(&[0.0; 32], 0, 28);
        let mut v = osc::Oscillator::load(&[0.0; 32], 0, 28);
        let mut rng = super::super::prim::Rng::new(7);
        let (carrier, raw) = internal_pair(
            &mut x,
            &mut v,
            &mut rng,
            CarrierControl {
                wave: 1,
                base: 0.01,
                external: 0.2,
                sr: 48_000.0,
                amount: 0.25,
            },
        );
        let mut x_ref = osc::Oscillator::load(&[0.0; 32], 0, 28);
        let mut v_ref = osc::Oscillator::load(&[0.0; 32], 0, 28);
        let sine = x_ref.sine(0.01, 0.2, 48_000.0);
        let saw = v_ref.polyblep(2, 0.01, 0.2, 48_000.0).0;
        assert!((raw - 0.5 * (sine + saw)).abs() < 1.0e-6);
        assert!((carrier - (0.25 * sine + 0.5 * saw)).abs() < 1.0e-6);

        for wave in 1..=3 {
            let mut x = osc::Oscillator::load(&[0.0; 32], 0, 28);
            let mut v = osc::Oscillator::load(&[0.0; 32], 0, 28);
            let mut rng = super::super::prim::Rng::new(7);
            let mut xmod_energy = 0.0;
            let mut vocoder_energy = 0.0;
            for _ in 0..256 {
                let (main, aux) = internal_pair(
                    &mut x,
                    &mut v,
                    &mut rng,
                    CarrierControl {
                        wave,
                        base: 0.02,
                        external: 0.0,
                        sr: 48_000.0,
                        amount: 0.0,
                    },
                );
                assert!((main - 0.5 * aux).abs() < 1.0e-6);
                xmod_energy += aux.abs();
            }
            for _ in 0..256 {
                let (main, aux) = internal_pair(
                    &mut x,
                    &mut v,
                    &mut rng,
                    CarrierControl {
                        wave,
                        base: 0.02,
                        external: 0.0,
                        sr: 48_000.0,
                        amount: 1.0,
                    },
                );
                assert!(main.is_finite() && aux.is_finite());
                vocoder_energy += aux.abs();
            }
            assert!(xmod_energy > 1.0 && vocoder_energy > 1.0, "wave {wave}");
        }
    }

    #[test]
    fn internal_carrier_auxiliary_is_half_gain_while_external_routing_is_unchanged() {
        let carrier = 0.75;
        let modulator = -0.25;
        for wave in 1..=3 {
            assert_eq!(
                auxiliary_output(wave as f32, carrier, modulator),
                carrier * 0.5
            );
        }
        assert_eq!(
            auxiliary_output(0.0, carrier, modulator),
            (carrier + modulator) * 0.5
        );
        assert_eq!(auxiliary_output(4.0, carrier, modulator), carrier);
    }

    #[test]
    fn source_amplifier_gain_clip_gate_and_vocoder_limit() {
        assert_eq!(source_soft_clip(3.0), 1.0);
        assert_eq!(source_soft_clip(-3.0), -1.0);
        assert!((source_soft_clip(0.5) - 0.5 * 27.25 / 29.25).abs() < 1.0e-6);
        let (pre, post) = amplifier_gains(1.0);
        assert_eq!(pre, 24.0);
        assert_eq!(post, 1.0);
        let (pre, _) = amplifier_gains(0.5);
        let expected = 0.25 + (0.75 - 0.25) * 0.25;
        assert!((pre - expected).abs() < 1.0e-6);

        let mut state = [0.0; 4];
        let (gated, raw) = amplify(&mut state, 0.002, 1.0, 1.0, 1.0, 0.1, 0.0001);
        assert!(gated.abs() < 0.002 && raw.abs() < 0.002);
        for _ in 0..100 {
            let _ = amplify(&mut state, 0.8, 1.0, 1.0, 1.0, 0.1, 0.0001);
        }
        let mut saturated = state;
        let mut linear = state;
        let (post, raw_post) = amplify(&mut saturated, 0.8, 1.0, 1.0, 1.0, 0.1, 0.0001);
        let (pre, raw_pre) = amplify(&mut linear, 0.8, 1.0, 0.0, 1.0, 0.1, 0.0001);
        assert!((post - source_soft_clip(pre)).abs() < 1.0e-6);
        assert!((raw_post - raw_pre).abs() < 1.0e-6);
        assert!(pre > post);
    }

    #[test]
    fn source_shaped_crossfade_has_clamped_scaled_endpoints_and_midpoint() {
        assert!((crossfade(0.8, -0.4, 0.0) + 0.4 * FRAC_1_SQRT_2).abs() < 1.0e-6);
        assert!((crossfade(0.8, -0.4, 1.0) - 0.8 * FRAC_1_SQRT_2).abs() < 1.0e-6);
        assert!((crossfade(0.8, -0.4, 0.5) - 0.2).abs() < 1.0e-6);
        assert_eq!(crossfade(0.8, -0.4, -2.0), crossfade(0.8, -0.4, 0.0));
    }

    #[test]
    fn asymmetric_diode_and_four_comparator_choices_preserve_source_roles() {
        assert_eq!(diode(0.5), 0.0);
        assert!((diode(1.0) + diode(-1.0)).abs() < 1.0e-6);
        let carrier = 0.25;
        let modulator = -0.8;
        let ring = (diode(modulator + 2.0 * carrier) + diode(modulator - 2.0 * carrier)) * 16.0;
        assert!((mode(2, carrier, modulator, 0.5) - soft_limit(ring)).abs() < 1.0e-6);
        assert_ne!(
            mode(2, carrier, modulator, 0.5),
            mode(2, modulator, carrier, 0.5)
        );
        let choices = [modulator, carrier, modulator, modulator.abs()];
        for (index, expected) in choices.into_iter().take(3).enumerate() {
            let timbre = index as f32 / 2.995;
            assert!((comparator(modulator, carrier, timbre) - expected).abs() < 1.0e-6);
        }
        // The pinned source's 2.995 scale approaches but does not reach
        // choice 3 exactly at the public timbre=1 endpoint.
        let endpoint = choices[2] + 0.995 * (choices[3] - choices[2]);
        assert!((comparator(modulator, carrier, 1.0) - endpoint).abs() < 1.0e-6);
        let between = comparator(modulator, carrier, 0.5 / 2.995);
        assert!((between - (choices[0] + choices[1]) * 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn digital_ring_xor_sum_and_comparator_endpoints_are_distinct() {
        let carrier = -0.35;
        let modulator = 0.7;
        let ring = 4.0 * carrier * modulator * 5.0;
        assert!((mode(3, carrier, modulator, 0.5) - ring / (1.0 + ring.abs())).abs() < 1.0e-6);
        assert!((mode(4, carrier, modulator, 0.0) - 0.7 * (carrier + modulator)).abs() < 1.0e-6);
        assert_eq!(
            mode(4, carrier, modulator, 1.0),
            xor_sample(modulator, carrier)
        );
        assert!(
            (mode(5, carrier, modulator, 1.0) - comparator(modulator, carrier, 1.0)).abs() < 1.0e-6
        );
    }

    #[test]
    fn public_position_skews_xmod_timbre_and_ends_comparator_at_raw_modulator() {
        assert!((skewed_timbre(0.0, 0.5) - 0.5).abs() < 1.0e-6);
        assert!((skewed_timbre(4.0, 0.5) - 0.375).abs() < 1.0e-6);
        assert_eq!(skewed_timbre(6.0, 0.0), 0.0);
        assert_eq!(skewed_timbre(6.0, 1.0), 1.0);
        let carrier = 0.25;
        let modulator = -0.8;
        let timbre = 0.7;
        let comparator = mode(5, carrier, modulator, skewed_timbre(5.4, timbre));
        let expected = comparator + 0.4 * (modulator - comparator);
        assert!((xmod_output(5.4, carrier, modulator, timbre) - expected).abs() < 1.0e-6);
        assert_eq!(mode(6, carrier, modulator, timbre), modulator);
    }

    #[test]
    fn vocoder_bridge_has_raw_modulator_center_and_continuous_sides() {
        assert!(vocoder_amount(5.4) < 1.0e-5);
        assert!((vocoder_amount(5.6) - 0.5).abs() < 1.0e-5);
        assert!((vocoder_amount(5.8) - 1.0).abs() < 1.0e-5);
        let xmod = -0.7;
        let vocoder = 0.6;
        let modulator = 0.2;
        assert!((bridge(xmod, vocoder, modulator, vocoder_amount(5.4)) - xmod).abs() < 1.0e-5);
        assert!((bridge(xmod, vocoder, modulator, vocoder_amount(5.6)) - modulator).abs() < 1.0e-6);
        assert!((bridge(xmod, vocoder, modulator, vocoder_amount(5.8)) - vocoder).abs() < 1.0e-5);
        let left = bridge(xmod, vocoder, modulator, vocoder_amount(5.599));
        let right = bridge(xmod, vocoder, modulator, vocoder_amount(5.601));
        assert!((left - right).abs() < 0.01, "transition around 5.6");
    }

    #[test]
    fn upper_public_algorithm_range_maps_vocoder_release_and_freeze() {
        assert_eq!(vocoder_release(6.0), 0.0);
        assert!((vocoder_release(7.0) - 0.75).abs() < 1.0e-6);
        assert_eq!(vocoder_release(8.0), 1.0);
        assert!(vocoder_release(7.9) > 0.995);
        assert!(vocoder_release(7.8) <= 0.995);
        assert_eq!(vocoder_amount(6.0), 1.0);
        assert_eq!(vocoder_amount(8.0), 1.0);
    }
}
