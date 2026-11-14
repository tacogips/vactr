//! Raw Vactr texture-kernel probe for the pinned Clouds mode comparison.
//!
//! Read whitespace-separated stereo float pairs from stdin, one frame per
//! line, and write the processed pairs to stdout. The shared Python harness
//! generates the source signal once and feeds these exact frames to both
//! probes. Input length must be a whole number of 32-frame blocks.
//!
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example clouds_texture_reference -- <mode> <scenario-index> <freeze 0|1> <quality 0..3>`.

use std::io::{self, BufRead, BufWriter, Write};

use vactr::dsp::arena::{SampleStore, StoreKind};
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::{self, FxCtx, FxState, FxStats};
use vactr::dsp::fft::{Fft, FFT_SIZE};

const SAMPLE_RATE: f32 = 32_000.0;
const BLOCK_FRAMES: usize = 32;
const PARAM_COUNT: usize = 13;

#[derive(Clone, Copy)]
struct Args {
    mode: Mode,
    scenario: usize,
    freeze: bool,
    quality: usize,
}

#[derive(Clone, Copy)]
enum Mode {
    Grain,
    Stretch,
    Loop,
    Spectral,
}

fn parse_args() -> io::Result<Args> {
    let values: Vec<String> = std::env::args().skip(1).collect();
    if values.len() != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected <mode> <scenario-index 0..2> <freeze 0|1> <quality 0..3>",
        ));
    }
    let mode = match values[0].as_str() {
        "grain" | "texture-grain" => Mode::Grain,
        "stretch" | "texture-stretch" => Mode::Stretch,
        "loop" | "texture-loop" => Mode::Loop,
        "spectral" | "texture-spectral" => Mode::Spectral,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mode must be grain, stretch, loop, or spectral",
            ));
        }
    };
    let scenario = parse_integer(&values[1], "scenario-index")?;
    if scenario > 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scenario-index must be 0, 1, or 2",
        ));
    }
    let freeze = match values[2].as_str() {
        "0" => false,
        "1" => true,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "freeze must be 0 or 1",
            ));
        }
    };
    let quality = parse_integer(&values[3], "quality")?;
    if quality > 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "quality must be between 0 and 3",
        ));
    }
    Ok(Args {
        mode,
        scenario,
        freeze,
        quality,
    })
}

fn parse_integer(value: &str, name: &str) -> io::Result<usize> {
    value.parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be an unsigned integer"),
        )
    })
}

// Common bus control order is position, size, pitch semitones, density,
// texture, stereo spread, feedback, reverb, mix, freeze, trigger, gate,
// quality. These deliberately span low, middle and high settings; every
// mode receives the same values so the upstream comparator can report its
// mode-specific control adaptation separately.
//
// Scenario 0: low settings; scenario 1: middle settings; scenario 2: high
// settings. Keep these tuples in sync with the reference wrapper's table.
fn parameters(args: Args) -> [f32; PARAM_COUNT] {
    let (position, size, pitch, density, texture, spread, feedback, reverb, mix) =
        match args.scenario {
            0 => (0.15, 0.18, -7.0, 0.25, 0.22, 0.00, 0.15, 0.10, 0.70),
            1 => (0.50, 0.52, 0.0, 0.50, 0.50, 0.50, 0.45, 0.35, 0.80),
            _ => (0.82, 0.86, 7.0, 0.78, 0.82, 1.00, 0.72, 0.65, 0.90),
        };
    [
        position,
        size,
        pitch,
        density,
        texture,
        spread,
        feedback,
        reverb,
        mix,
        0.0, // Freeze is applied at the requested input-frame boundary below.
        0.0, // Trigger is pulsed at the first frame of each 32-block phrase.
        1.0, // Gate remains open.
        match args.quality {
            0 => 0.0,
            1 => 1.0,
            2 => 2.0,
            _ => 3.0,
        },
    ]
}

fn read_input() -> io::Result<Vec<[f32; 2]>> {
    let stdin = io::stdin();
    let mut frames = Vec::new();
    for (line_index, line) in stdin.lock().lines().enumerate() {
        let line = line?;
        let mut fields = line.split_whitespace();
        let left = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {} must contain two samples", line_index + 1),
            )
        })?;
        let right = fields.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {} must contain two samples", line_index + 1),
            )
        })?;
        if fields.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {} has extra fields", line_index + 1),
            ));
        }
        let left = parse_sample(left, line_index + 1)?;
        let right = parse_sample(right, line_index + 1)?;
        frames.push([left, right]);
    }
    if frames.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "stdin contained no stereo frames",
        ));
    }
    if frames.len() % BLOCK_FRAMES != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input frame count must be a multiple of 32",
        ));
    }
    Ok(frames)
}

fn parse_sample(value: &str, line: usize) -> io::Result<f32> {
    let sample = value.parse::<f32>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("line {line} contains an invalid float"),
        )
    })?;
    if !sample.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("line {line} contains a nonfinite sample"),
        ));
    }
    Ok(sample)
}

fn kernel_mem_len(mode: Mode) -> usize {
    match mode {
        Mode::Grain => effects::texture::mem_len(SAMPLE_RATE),
        Mode::Stretch => effects::texture_stretch::mem_len(SAMPLE_RATE),
        Mode::Loop => effects::texture_loop::mem_len(SAMPLE_RATE),
        Mode::Spectral => effects::texture_spectral::mem_len(SAMPLE_RATE),
    }
}

fn kernel_init(mode: Mode, state: &mut FxState, mem: &mut [f32]) {
    match mode {
        Mode::Grain => effects::texture::init(state, mem),
        Mode::Stretch => effects::texture_stretch::init(state, mem),
        Mode::Loop => effects::texture_loop::init(state, mem),
        Mode::Spectral => effects::texture_spectral::init(state, mem),
    }
}

fn kernel_process(
    mode: Mode,
    params: &[f32],
    state: &mut FxState,
    mem: &mut [f32],
    left: &mut [f32],
    right: &mut [f32],
    ctx: &mut FxCtx<'_>,
) {
    match mode {
        Mode::Grain => {
            effects::texture::process(params, state, mem, left, right, ctx);
        }
        Mode::Stretch => {
            effects::texture_stretch::process(params, state, mem, left, right, ctx);
        }
        Mode::Loop => {
            effects::texture_loop::process(params, state, mem, left, right, ctx);
        }
        Mode::Spectral => {
            effects::texture_spectral::process(params, state, mem, left, right, ctx);
        }
    }
}

fn run(mut output: impl Write, args: Args, frames: &[[f32; 2]]) -> io::Result<()> {
    let mut params = parameters(args);
    let caps = CapabilitySet::native();
    let mem_len = kernel_mem_len(args.mode);
    let mut mem = vec![0.0; mem_len];
    let mut state = FxState::default();
    kernel_init(args.mode, &mut state, &mut mem);
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; FFT_SIZE * 2];
    let mut stats = FxStats::default();
    let mut left = [0.0; BLOCK_FRAMES];
    let mut right = [0.0; BLOCK_FRAMES];

    for (block_index, block) in frames.chunks_exact(BLOCK_FRAMES).enumerate() {
        // Quality bit zero makes the input dual-mono before the dry copy,
        // matching FxUnit::run. The kernel's quality adapter also converts
        // its wet path. Bit one selects Vactr's documented host-rate hold and
        // 8-bit quantization approximation.
        for (index, frame) in block.iter().enumerate() {
            let (mut input_l, mut input_r) = (frame[0], frame[1]);
            if args.quality & 1 != 0 {
                let mid = (input_l + input_r) * 0.5;
                input_l = mid;
                input_r = mid;
            }
            left[index] = input_l;
            right[index] = input_r;
        }
        let dry_left = left;
        let dry_right = right;
        let first_frame = block_index * BLOCK_FRAMES;
        params[9] = if args.freeze && first_frame >= 32_000 {
            1.0
        } else {
            0.0
        };
        // Emit one trigger block per 1024-frame phrase, with a low block
        // between pulses so every effect observes a rising edge.
        params[10] = if block_index % 32 == 0 { 1.0 } else { 0.0 };
        {
            let mut ctx = FxCtx {
                sr: SAMPLE_RATE,
                store: &store,
                fft: &fft,
                caps: &caps,
                scratch: &mut scratch,
                analysis: &mut analysis,
                stats: &mut stats,
            };
            kernel_process(
                args.mode, &params, &mut state, &mut mem, &mut left, &mut right, &mut ctx,
            );
        }
        // FxUnit::run blends linearly: dry + (wet - dry) * mix.
        // The raw kernels are fully wet and leave bus mix to their caller.
        for index in 0..BLOCK_FRAMES {
            left[index] = dry_left[index] + (left[index] - dry_left[index]) * params[8];
            right[index] = dry_right[index] + (right[index] - dry_right[index]) * params[8];
            if !left[index].is_finite() || !right[index].is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "nonfinite texture output at frame {}",
                        block_index * BLOCK_FRAMES + index
                    ),
                ));
            }
            writeln!(output, "{:.9} {:.9}", left[index], right[index])?;
        }
    }
    output.flush()
}

fn main() -> io::Result<()> {
    let args = parse_args()?;
    let frames = read_input()?;
    run(BufWriter::new(io::stdout().lock()), args, &frames)
}
