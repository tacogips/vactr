//! Standalone reference driver for the shared Plaits voice-layer DSP.
//!
//! This example emits line-oriented rows consumed by
//! `verification/compare_plaits_voice.py`; it does not write audio or other
//! artifacts into the repository.

use std::fs;
use std::io::{self, BufWriter, Write};

use vactr::dsp::ported::plaits_voice;
use vactr::dsp::ugen::voice_layer::{
    clip_unit, compress_level, decay_terms, ping_attack, post_gain, ControlClock, DecayEnvelope,
    LowPassGate, LpgMode, PostLimiter, VactrolEnvelope, REFERENCE_RATE,
};

const BLOCK_FRAMES: usize = 12;

fn parse_f32(text: &str, label: &str) -> io::Result<f32> {
    let value = text.parse::<f32>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be numeric"),
        )
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be finite"),
        ))
    }
}

fn parse_unit(text: &str, label: &str) -> io::Result<f32> {
    let value = parse_f32(text, label)?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be in [0, 1]"),
        ))
    }
}

fn parse_mode(text: &str) -> io::Result<LpgMode> {
    match text {
        "off" => Ok(LpgMode::Off),
        "ping" => Ok(LpgMode::Ping),
        "level" => Ok(LpgMode::Level),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mode must be off, ping or level",
        )),
    }
}

fn read_samples(path: &str) -> io::Result<Vec<f32>> {
    let text = fs::read_to_string(path)?;
    let mut samples = Vec::new();
    for (index, field) in text.split_whitespace().enumerate() {
        let sample = parse_f32(field, "input sample")?;
        samples.push(sample);
        if samples.len() > 2_000_000 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("input has too many samples (first excess at {})", index + 1),
            ));
        }
    }
    Ok(samples)
}

fn read_trajectory(path: &str) -> io::Result<Vec<(f32, f32, f32)>> {
    let text = fs::read_to_string(path)?;
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            let values = line
                .split_whitespace()
                .map(|field| parse_f32(field, "trajectory value"))
                .collect::<io::Result<Vec<_>>>()?;
            if values.len() != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("trajectory row {} must contain three values", index + 1),
                ));
            }
            Ok((values[0], values[1], values[2]))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn trajectory(
    mode: LpgMode,
    decay: f32,
    color: f32,
    frequency_hz: f32,
    velocity: f32,
    gate_blocks: usize,
    blocks: usize,
    sample_rate: f32,
) -> Vec<(f32, f32, f32, f32, usize)> {
    let (short_decay, decay_tail) = decay_terms(decay, color);
    let mut clock = ControlClock { carry: 0.0 };
    let mut decay_envelope = DecayEnvelope { value: 0.0 };
    let mut vactrol = VactrolEnvelope::new();
    let mut rows = Vec::with_capacity(blocks);
    let mut start_sample = 0;
    for block in 0..blocks {
        let length = clock.next_len(sample_rate);
        if block == 0 {
            decay_envelope.trigger();
            if mode == LpgMode::Ping {
                vactrol.trigger();
            }
        }
        decay_envelope.process(short_decay);
        let gate_level = if block < gate_blocks {
            compress_level(velocity)
        } else {
            0.0
        };
        match mode {
            LpgMode::Ping => {
                vactrol.process_ping(ping_attack(frequency_hz), short_decay, decay_tail, color)
            }
            LpgMode::Level => {
                vactrol.process_lp(gate_level, short_decay, decay_tail, color);
            }
            LpgMode::Off => {}
        }
        rows.push((
            vactrol.gain(),
            vactrol.frequency(),
            vactrol.hf_bleed(),
            decay_envelope.value(),
            start_sample,
        ));
        start_sample += length;
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn emit_trajectory(
    mode: LpgMode,
    decay: f32,
    color: f32,
    frequency_hz: f32,
    velocity: f32,
    gate_blocks: usize,
    blocks: usize,
    sample_rate: f32,
) -> io::Result<()> {
    let rows = trajectory(
        mode,
        decay,
        color,
        frequency_hz,
        velocity,
        gate_blocks,
        blocks,
        sample_rate,
    );
    let mut output = BufWriter::new(io::stdout().lock());
    for (block, (gain, frequency, bleed, decay_value, start_sample)) in rows.iter().enumerate() {
        writeln!(
            output,
            "T {block} {gain:.9e} {frequency:.9e} {bleed:.9e} {decay_value:.9e} {start_sample}"
        )?;
    }
    output.flush()
}

fn apply_audio(
    input: &[f32],
    gains: &[(f32, f32, f32)],
    registered_gain: f32,
    sample_rate: f32,
    bypass: bool,
) -> io::Result<()> {
    if !bypass && gains.len() * BLOCK_FRAMES < input.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trajectory does not cover all input samples",
        ));
    }
    let mut limiter = PostLimiter::new();
    let mut gate = LowPassGate::default();
    let mut output = BufWriter::new(io::stdout().lock());
    let post = post_gain(registered_gain);
    for (block_index, chunk) in input.chunks(BLOCK_FRAMES).enumerate() {
        let (gain, frequency, bleed) = if bypass {
            (1.0, 0.5, 0.0)
        } else {
            gains[block_index]
        };
        let mut processed = [0.0_f32; BLOCK_FRAMES];
        for (index, sample) in chunk.iter().enumerate() {
            processed[index] = if registered_gain < 0.0 {
                limiter.process(-registered_gain, sample_rate, *sample)
            } else {
                *sample
            };
        }
        if !bypass {
            gate.begin(gain * post, frequency, bleed, chunk.len(), sample_rate);
        }
        for sample in &processed[..chunk.len()] {
            let value = if bypass {
                clip_unit(*sample * post)
            } else {
                clip_unit(gate.tick(*sample))
            };
            writeln!(output, "M {value:.9e}")?;
        }
    }
    output.flush()
}

fn main() -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("traj") if args.len() == 9 => {
            let mode = parse_mode(&args[1])?;
            let decay = parse_unit(&args[2], "decay")?;
            let color = parse_unit(&args[3], "color")?;
            let frequency = parse_f32(&args[4], "frequency")?;
            let velocity = parse_unit(&args[5], "velocity")?;
            let gate_blocks = args[6].parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "gate_blocks must be an integer")
            })?;
            let blocks = args[7].parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "blocks must be an integer")
            })?;
            let sample_rate = parse_f32(&args[8], "sample rate")?;
            if !(1_000.0..=192_000.0).contains(&sample_rate) || blocks == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "sample rate must be in [1000, 192000] and blocks must be positive",
                ));
            }
            emit_trajectory(
                mode,
                decay,
                color,
                frequency,
                velocity,
                gate_blocks,
                blocks,
                sample_rate,
            )
        }
        Some("audio") if args.len() == 4 => {
            let gain = parse_f32(&args[1], "gain")?;
            let input = read_samples(&args[2])?;
            let trajectory = read_trajectory(&args[3])?;
            apply_audio(&input, &trajectory, gain, REFERENCE_RATE, false)
        }
        Some("bypass") if args.len() == 3 => {
            let gain = parse_f32(&args[1], "gain")?;
            let input = read_samples(&args[2])?;
            apply_audio(&input, &[], gain, REFERENCE_RATE, true)
        }
        Some("lane") if args.len() == 10 => {
            let slot = args[1].parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "slot must be an integer")
            })?;
            let lane = args[2].parse::<u8>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "lane must be 0 or 1")
            })?;
            if lane > 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "lane must be 0 or 1"));
            }
            let mode = parse_mode(&args[3])?;
            let decay = parse_unit(&args[4], "decay")?;
            let color = parse_unit(&args[5], "color")?;
            let frequency = parse_f32(&args[6], "frequency")?;
            let velocity = parse_unit(&args[7], "velocity")?;
            let gate_blocks = args[8].parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "gate_blocks must be an integer")
            })?;
            let input = read_samples(&args[9])?;
            let registration = plaits_voice(slot).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "slot must be in 0..24")
            })?;
            let enveloped = registration.is_enveloped(false);
            let registered_gain = registration.gain(lane);
            let rows = trajectory(
                mode,
                decay,
                color,
                frequency,
                velocity,
                gate_blocks,
                input.len().div_ceil(BLOCK_FRAMES),
                REFERENCE_RATE,
            );
            let gains = rows
                .iter()
                .map(|row| (row.0, row.1, row.2))
                .collect::<Vec<_>>();
            apply_audio(
                &input,
                &gains,
                registered_gain,
                REFERENCE_RATE,
                mode == LpgMode::Off || enveloped,
            )
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected traj <mode> <decay> <color> <freq_hz> <velocity> <gate_blocks> <blocks> <sr>; audio <gain> <input_path> <trajectory_path>; bypass <gain> <input_path>; or lane <slot> <lane> <mode> <decay> <color> <freq_hz> <velocity> <gate_blocks> <input_path>",
        )),
    }
}
