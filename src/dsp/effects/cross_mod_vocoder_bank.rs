//! Two-pass, 20-band source-rate Warps filter-bank stages.
//!
//! The CrossoverSvf update order and analysis/synthesis topology translate
//! Emilie Gillet's MIT `warps/dsp/filter_bank.{h,cc}` and MIT stmlib
//! `dsp/filter.h`. Only the 20 filter rows in the sibling coefficient file
//! are imported from `warps/resources.cc`; no other resource is read.

#[path = "cross_mod_vocoder_coeffs.rs"]
mod coeffs;

use super::vocoder_fir as fir;

pub(super) const BANDS: usize = 20;
pub(super) const BLOCK: usize = 60;
const FIR_LEN: usize = fir::FIR_STATE_LEN;
const SVF_LEN: usize = BANDS * 2 * 6;
const DELAY_LEN: usize = 456 + BANDS;
const SAMPLES_LEN: usize = 13 * 5 + 6 * 20 + BLOCK;
const SVF_START: usize = FIR_LEN;
const DELAY_START: usize = SVF_START + SVF_LEN;
const SAMPLES_START: usize = DELAY_START + DELAY_LEN;
pub(super) const BANK_LEN: usize = SAMPLES_START + SAMPLES_LEN;

pub(super) fn factor(band: usize) -> usize {
    coeffs::BANDS[band][0] as usize
}

fn samples_offset(band: usize) -> usize {
    if band < 13 {
        band * 5
    } else if band < 19 {
        65 + (band - 13) * 20
    } else {
        185
    }
}

pub(super) fn band_samples(mem: &[f32], band: usize) -> &[f32] {
    let offset = SAMPLES_START + samples_offset(band);
    &mem[offset..offset + BLOCK / factor(band)]
}

pub(super) fn band_samples_mut(mem: &mut [f32], band: usize) -> &mut [f32] {
    let offset = SAMPLES_START + samples_offset(band);
    &mut mem[offset..offset + BLOCK / factor(band)]
}

fn filter(state: &mut [f32], input: f32, f: f32, fq: f32, mode: usize) -> f32 {
    // One CrossoverSvf contains two state-variable integration stages.
    state[0] += f * state[1];
    state[1] += -fq * state[1] - f * state[0] + input;
    if mode == 1 {
        state[1] += state[2];
    }
    state[2] = input;
    let y = match mode {
        0 => state[0] * f,
        1 => state[1] * fq,
        _ => state[2] - state[0] * f - state[1] * fq,
    };
    state[3] += f * state[4];
    state[4] += -fq * state[4] - f * state[3] + y;
    if mode == 1 {
        state[4] += state[5];
    }
    state[5] = y;
    match mode {
        0 => state[3] * f,
        1 => state[4] * fq,
        _ => state[5] - state[3] * f - state[4] * fq,
    }
}

pub(super) fn analyze(mem: &mut [f32], input: &[f32; BLOCK]) {
    let (fir_mem, rest) = mem.split_at_mut(FIR_LEN);
    let (svf_mem, rest) = rest.split_at_mut(SVF_LEN);
    let samples = &mut rest[DELAY_LEN..DELAY_LEN + SAMPLES_LEN];
    let mut mid = [0.0; 20];
    let mut low = [0.0; 5];
    fir::down3(&mut fir_mem[fir::MID_DOWN], input, &mut mid);
    fir::down4(&mut fir_mem[fir::LOW_DOWN], &mid, &mut low);
    for (band, row) in coeffs::BANDS.iter().enumerate() {
        let factor = factor(band);
        let source: &[f32] = if factor == 12 {
            &low
        } else if factor == 3 {
            &mid
        } else {
            input
        };
        let mode = if band == 0 {
            0
        } else if band == 19 {
            2
        } else {
            1
        };
        let offset = samples_offset(band);
        let state = &mut svf_mem[band * 12..(band + 1) * 12];
        for (frame, &sample) in source.iter().enumerate() {
            let first = filter(&mut state[..6], sample, row[3], row[4], mode);
            let second = filter(&mut state[6..], first, row[5], row[6], mode);
            samples[offset + frame] = second * row[2];
        }
    }
}

fn delay_len(band: usize) -> usize {
    let factor = factor(band) as i32;
    let source_delay = coeffs::BANDS[band][1] as i32 * factor;
    let mut compensation = 256 - source_delay;
    if band < 13 {
        compensation -= 144; // 4×(24+6) + (18+6) source SRC delays.
    } else if band < 19 {
        compensation -= 24; // 3×/36 down and up source SRC delays.
    }
    ((compensation - factor / 2).max(0) / factor + 1) as usize
}

fn delay_offset(band: usize) -> usize {
    (0..band).map(|index| delay_len(index) + 1).sum()
}

fn delay_sample(state: &mut [f32], sample: f32) -> f32 {
    let size = state.len() - 1;
    let mut head = state[0] as usize;
    state[1 + head] = sample;
    head = (head + 1) % size;
    state[0] = head as f32;
    state[1 + head]
}

fn add_group(mem: &mut [f32], start: usize, end: usize, destination: &mut [f32]) {
    let (_, rest) = mem.split_at_mut(DELAY_START);
    let (delays, samples) = rest.split_at_mut(DELAY_LEN);
    for band in start..end {
        let len = delay_len(band);
        let delay_start = delay_offset(band);
        let sample_start = samples_offset(band);
        let delay = &mut delays[delay_start..delay_start + len + 1];
        for (frame, output) in destination.iter_mut().enumerate() {
            *output += delay_sample(delay, samples[sample_start + frame]);
        }
    }
}

pub(super) fn synthesize(mem: &mut [f32], out: &mut [f32; BLOCK]) {
    let mut low = [0.0; 5];
    let mut mid = [0.0; 20];
    add_group(mem, 0, 13, &mut low);
    fir::up4(&mut mem[fir::LOW_UP], &low, &mut mid);
    add_group(mem, 13, 19, &mut mid);
    fir::up3(&mut mem[fir::MID_UP], &mid, out);
    add_group(mem, 19, 20, out);
}

#[cfg(test)]
mod tests {
    use super::{analyze, band_samples, delay_len, factor, synthesize, BANK_LEN, BLOCK};

    #[test]
    fn twenty_bands_have_source_decimation_and_exact_delay_pool() {
        assert_eq!(
            (0..20).map(factor).collect::<Vec<_>>(),
            [12; 13]
                .into_iter()
                .chain([3; 6])
                .chain([1])
                .collect::<Vec<_>>()
        );
        assert_eq!((0..20).map(delay_len).sum::<usize>(), 456);
        assert_eq!(BANK_LEN, 1071);
    }

    #[test]
    fn analysis_and_synthesis_are_finite_with_distinct_low_and_high_bands() {
        let mut mem = vec![0.0; BANK_LEN];
        let mut low_energy = 0.0;
        let mut high_energy = 0.0;
        let mut output_energy = 0.0;
        for block in 0..100 {
            let mut input = [0.0; BLOCK];
            for (frame, sample) in input.iter_mut().enumerate() {
                let n = (block * BLOCK + frame) as f32;
                *sample = (n * 87.0 * std::f32::consts::TAU / 96_000.0).sin() * 0.5;
            }
            analyze(&mut mem, &input);
            low_energy += band_samples(&mem, 0).iter().map(|x| x * x).sum::<f32>();
            high_energy += band_samples(&mem, 19).iter().map(|x| x * x).sum::<f32>();
            let mut out = [0.0; BLOCK];
            synthesize(&mut mem, &mut out);
            assert!(out.iter().all(|x| x.is_finite()));
            output_energy += out.iter().map(|x| x * x).sum::<f32>();
        }
        assert!(low_energy > high_energy);
        assert!(output_energy > 0.0);
    }
}
