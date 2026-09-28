//! Fixed 60-frame, 96 kHz Warps vocoder signal stages.
//!
//! The bank, follower, gain-ramp, and formant roles translate the MIT Warps
//! DSP. Analytic ratio conversion and the surrounding carrier/amplifier remain
//! numerically different from the original firmware.

use super::vocoder_bank::{self as bank, BANDS, BLOCK};

const BANKS_LEN: usize = 2 * bank::BANK_LEN;
const INPUTS_LEN: usize = 2 * BLOCK;
const OUTPUT_LEN: usize = BLOCK;
const FOLLOWERS_LEN: usize = 2 * BANDS;
const GAINS_LEN: usize = 2 * BANDS;
pub(super) const MEM_LEN: usize = BANKS_LEN + INPUTS_LEN + OUTPUT_LEN + FOLLOWERS_LEN + GAINS_LEN;

#[cfg(test)]
pub(super) fn envelope_energy(mem: &[f32]) -> f32 {
    let offset = BANKS_LEN + INPUTS_LEN + OUTPUT_LEN;
    mem[offset..offset + FOLLOWERS_LEN]
        .chunks_exact(2)
        .map(|band| band[0])
        .sum()
}

fn follower_rates(release: f32, band: usize) -> (f32, f32) {
    if release > 0.995 {
        return (0.0, 0.0);
    }
    #[allow(clippy::cast_precision_loss)]
    let frequency = 80.0 * 2.0f32.powf(-6.0 * release) * 1.2599f32.powi(band as i32);
    let normalized = frequency * bank::factor(band) as f32 / 96_000.0;
    ((2.0 * normalized).min(1.0), (0.5 * normalized).min(1.0))
}

fn target_gains(followers: &[f32], formant: f32, target: &mut [[f32; 2]; BANDS]) {
    let mut amount = 2.0 * (formant - 0.5).abs();
    amount *= 2.0 - amount;
    amount *= 2.0 - amount;
    let increment = 4.0 * 2.0f32.powf(-4.0 * formant);
    let mut envelope = 0.0f32;
    for gain in target.iter_mut() {
        let clamped = envelope.clamp(0.0f32, 18.9999);
        let first = clamped.floor() as usize;
        let blend = clamped.fract();
        let a = followers[first * 2 + 1];
        let b = followers[(first + 1) * 2 + 1];
        let mut band_gain = a + (b - a) * blend;
        if envelope >= 18.9999 {
            band_gain /= 1.0 + envelope - 18.9999;
        }
        gain[0] = band_gain * amount;
        gain[1] = 1.0 - amount;
        envelope += increment;
    }
}

fn process_block(mem: &mut [f32], formant: f32, release: f32) {
    let (mod_bank, rest) = mem.split_at_mut(bank::BANK_LEN);
    let (carrier_bank, rest) = rest.split_at_mut(bank::BANK_LEN);
    let (inputs, rest) = rest.split_at_mut(INPUTS_LEN);
    let (output, rest) = rest.split_at_mut(OUTPUT_LEN);
    let (followers, gains) = rest.split_at_mut(FOLLOWERS_LEN);
    let mod_input: &[f32; BLOCK] = (&inputs[BLOCK..]).try_into().expect("fixed block");
    let carrier_input: &[f32; BLOCK] = (&inputs[..BLOCK]).try_into().expect("fixed block");
    bank::analyze(mod_bank, mod_input);
    bank::analyze(carrier_bank, carrier_input);

    // Source computes formant targets from the *previous* block's peaks.
    let mut targets = [[0.0; 2]; BANDS];
    target_gains(followers, formant, &mut targets);
    for band_index in 0..BANDS {
        let size = BLOCK / bank::factor(band_index);
        let (attack, decay) = follower_rates(release, band_index);
        let follower = &mut followers[2 * band_index..2 * band_index + 2];
        let previous = &mut gains[2 * band_index..2 * band_index + 2];
        let target = targets[band_index];
        let mut carrier_gain = previous[0];
        let mut vocoder_gain = previous[1];
        let carrier_step = (target[0] - carrier_gain) / size as f32;
        let vocoder_step = (target[1] - vocoder_gain) / size as f32;
        let mod_samples = bank::band_samples(mod_bank, band_index);
        let carrier_samples = bank::band_samples_mut(carrier_bank, band_index);
        let mut block_peak = 0.0f32;
        for (&mod_sample, carrier_sample) in mod_samples.iter().zip(carrier_samples.iter_mut()) {
            let energy = mod_sample.abs() * (BANDS as f32).sqrt();
            let delta = energy - follower[0];
            follower[0] += delta * if delta > 0.0 { attack } else { decay };
            block_peak = block_peak.max(follower[0]);
            *carrier_sample *= carrier_gain + vocoder_gain * follower[0];
            carrier_gain += carrier_step;
            vocoder_gain += vocoder_step;
        }
        let peak_delta = block_peak - follower[1];
        follower[1] += peak_delta * if peak_delta > 0.0 { 0.5 } else { 0.1 };
        previous.copy_from_slice(&target);
    }
    let out: &mut [f32; BLOCK] = output.try_into().expect("fixed block");
    bank::synthesize(carrier_bank, out);
}

/// A one-block source pipeline, independent of host callback partitioning.
pub(super) fn sample(
    mem: &mut [f32],
    write: &mut usize,
    read: &mut usize,
    carrier: f32,
    modulator: f32,
    formant: f32,
    release: f32,
) -> f32 {
    debug_assert!(mem.len() >= MEM_LEN);
    let output = if *read < BLOCK {
        let value = mem[BANKS_LEN + INPUTS_LEN + *read];
        *read += 1;
        value
    } else {
        0.0
    };
    mem[BANKS_LEN + *write] = carrier;
    mem[BANKS_LEN + BLOCK + *write] = modulator;
    *write += 1;
    if *write == BLOCK {
        process_block(mem, formant, release);
        *write = 0;
        *read = 0;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::super::limiter::{Limiter, INITIAL_PEAK};
    use super::{sample, MEM_LEN};

    #[test]
    fn pipeline_has_one_block_latency_and_finite_stereo_source_response() {
        let mut mem = vec![0.0; MEM_LEN];
        let mut write = 0;
        let mut read = 60;
        let mut energy = 0.0;
        for frame in 0..6000 {
            let t = frame as f32 / 96_000.0;
            let carrier = (std::f32::consts::TAU * 230.0 * t).sin() * 0.8;
            let modulator = (std::f32::consts::TAU * 230.0 * t).sin() * 0.8;
            let out = sample(
                &mut mem, &mut write, &mut read, carrier, modulator, 0.5, 0.5,
            );
            assert!(out.is_finite());
            if frame < 60 {
                assert_eq!(out, 0.0);
            }
            energy += out.abs();
        }
        assert!(energy > 1.0, "vocoder should synthesize audio");
    }

    #[test]
    #[allow(clippy::excessive_precision)] // Pinned C++ probe values are retained verbatim.
    fn pinned_source_probe_scenarios() {
        // Independently compiled pinned-source probe, 100 × 60 frames at
        // 96 kHz. Its values include the source output limiter. The pipeline
        // adds exactly one block of latency, which is removed before metrics.
        let scenarios = [
            (0.1, 0.25, 100, 0.1, 0.108_982_816_7, 0.099_737_541_4),
            (0.5, 0.5, 100, 0.5, 0.014_511_668_5, 0.021_231_203_5),
            (0.9, 0.75, 100, 0.9, 0.010_526_650_9, 0.015_288_648_0),
            (0.1, 0.5, 50, 0.1, 0.034_655_204_9, 0.044_810_643_7),
            (0.9, 0.5, 50, 0.9, 0.004_340_591_8, 0.006_701_626_4),
            (0.5, 0.5, 50, 1.0, 0.009_826_680_6, 0.009_238_651_7),
        ];
        for (release, formant, gate_block, after_release, source_rms, source_tail) in scenarios {
            let mut mem = vec![0.0; MEM_LEN];
            let mut limiter = Limiter::new(INITIAL_PEAK, 96_000.0);
            let (mut write, mut read) = (0, 60);
            let mut output = [0.0f32; 6060];
            for (frame, result) in output.iter_mut().enumerate() {
                let n = frame.min(5999);
                let t = n as f32 / 96_000.0;
                let modulator = if n / 60 >= gate_block {
                    0.0
                } else {
                    0.65 * (std::f32::consts::TAU * 220.0 * t).sin()
                        + 0.20 * (std::f32::consts::TAU * 330.0 * t).sin()
                };
                let carrier = 0.55 * (std::f32::consts::TAU * 110.0 * t).sin()
                    + 0.25 * (std::f32::consts::TAU * 880.0 * t).sin();
                let current_release = if n / 60 >= gate_block {
                    after_release
                } else {
                    release
                };
                *result = limiter.process(sample(
                    &mut mem,
                    &mut write,
                    &mut read,
                    carrier,
                    modulator,
                    formant,
                    current_release,
                ));
            }
            let aligned = &output[60..];
            let rms = (aligned.iter().map(|x| x * x).sum::<f32>() / 6000.0).sqrt();
            let tail = (aligned[4500..].iter().map(|x| x * x).sum::<f32>() / 1500.0).sqrt();
            assert!(
                (rms - source_rms).abs() < 5.0e-6,
                "probe RMS: {rms} vs {source_rms}"
            );
            assert!(
                (tail - source_tail).abs() < 5.0e-6,
                "probe tail RMS: {tail} vs {source_tail}"
            );
            if release == 0.5 && formant == 0.5 && gate_block == 100 {
                // Independent source sample positions guard FIR phase and
                // band delay ordering, which aggregate RMS cannot identify.
                let reference = [
                    (200, -0.000_067_262_42),
                    (300, -0.000_210_953_63),
                    (600, -0.001_029_505_9),
                    (1200, 0.003_002_920_4),
                    (3000, -0.007_872_957_7),
                    (4500, -0.005_118_472_5),
                    (5999, -0.031_668_074),
                ];
                for (index, expected) in reference {
                    assert!(
                        (aligned[index] - expected).abs() < 1.0e-5,
                        "sample {index}: {} vs {expected}",
                        aligned[index]
                    );
                }
            }
        }
    }
}
