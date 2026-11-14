//! Direct Vactr cross-modulation kernel probe for the Warps XMOD comparison.
//!
//! Reads little-endian interleaved stereo `f32` input, with L as external
//! carrier and R as modulator, then writes one `main aux` pair per frame. The
//! algorithm argument is Vactr's public 0..6 position. The comparison harness
//! maps it to the source's normalized `modulation_algorithm` by dividing by 8.
//!
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example warps_xmod_reference
//! -- <algorithm-position> <timbre> <carrier-drive> <modulator-drive>
//! <carrier-wave> <carrier-frequency-hz> <input-f32-stereo-path> <frames>`.

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::{cross_mod, FxCtx, FxState, FxStats};
use vactr::dsp::fft::{Fft, FFT_SIZE};

const SAMPLE_RATE: f32 = 96_000.0;
const BLOCK_FRAMES: usize = 60;
const SCRATCH_FLOATS: usize = 4 * FFT_SIZE;
const ANALYSIS_FLOATS: usize = 1024;

#[derive(Clone, Copy)]
struct Params {
    algorithm_position: f32,
    timbre: f32,
    carrier_drive: f32,
    modulator_drive: f32,
    carrier_shape: f32,
    carrier_frequency: f32,
}

fn finite_arg(text: &str, label: &str) -> io::Result<f32> {
    let value = text.parse::<f32>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be a number"),
        )
    })?;
    if !value.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be finite"),
        ));
    }
    Ok(value)
}

fn ranged_arg(text: &str, label: &str, min: f32, max: f32) -> io::Result<f32> {
    let value = finite_arg(text, label)?;
    if !(min..=max).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be between {min} and {max}"),
        ));
    }
    Ok(value)
}

fn parse_params() -> io::Result<(Params, PathBuf, usize)> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: algorithm-position timbre carrier-drive modulator-drive carrier-wave carrier-frequency-hz input-f32-stereo-path frames",
        ));
    }
    let algorithm_position = ranged_arg(&args[0], "algorithm-position", 0.0, 6.0)?;
    let carrier_frequency = finite_arg(&args[5], "carrier-frequency-hz")?;
    if !(20.0..=SAMPLE_RATE * 0.45).contains(&carrier_frequency) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "carrier-frequency-hz must be between 20 and 43200",
        ));
    }
    let frames = args[7].parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "frames must be a positive integer",
        )
    })?;
    if frames == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frames must be a positive integer",
        ));
    }
    Ok((
        Params {
            algorithm_position,
            timbre: ranged_arg(&args[1], "timbre", 0.0, 1.0)?,
            carrier_drive: ranged_arg(&args[2], "carrier-drive", 0.0, 1.0)?,
            modulator_drive: ranged_arg(&args[3], "modulator-drive", 0.0, 1.0)?,
            carrier_shape: ranged_arg(&args[4], "carrier-wave", 0.0, 3.0)?,
            carrier_frequency,
        },
        PathBuf::from(&args[6]),
        frames,
    ))
}

#[allow(clippy::too_many_arguments)]
fn process_block(
    output: &mut impl Write,
    params: Params,
    frames: usize,
    carrier: &mut [f32; BLOCK_FRAMES],
    modulator: &mut [f32; BLOCK_FRAMES],
    state: &mut FxState,
    mem: &mut [f32],
    store: &SampleStore,
    fft: &Fft,
    caps: &CapabilitySet,
    scratch: &mut [f32],
    analysis: &mut [f32],
    stats: &mut FxStats,
) -> io::Result<()> {
    // Keep the shared drive at unity so the two public channel controls map
    // directly to the source's independent carrier and modulator drives.
    let effect_params = [
        params.algorithm_position,
        params.timbre,
        1.0,
        params.carrier_shape,
        params.carrier_frequency,
        params.carrier_drive,
        params.modulator_drive,
    ];
    {
        let mut ctx = FxCtx {
            sr: SAMPLE_RATE,
            store,
            fft,
            caps,
            scratch,
            analysis,
            stats,
        };
        cross_mod::process(
            &effect_params,
            state,
            mem,
            &mut carrier[..frames],
            &mut modulator[..frames],
            &mut ctx,
        );
    }
    for (&main, &aux) in carrier[..frames].iter().zip(&modulator[..frames]) {
        if !main.is_finite() || !aux.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "nonfinite Vactr XMOD output",
            ));
        }
        writeln!(output, "{main:.9} {aux:.9}")?;
    }
    Ok(())
}

fn run(params: Params, input_path: PathBuf, frame_count: usize) -> io::Result<()> {
    let input = fs::read(input_path)?;
    let expected_bytes = frame_count
        .checked_mul(2)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "frame count overflows input size",
            )
        })?;
    if input.len() != expected_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "input contains {} bytes; expected {expected_bytes}",
                input.len()
            ),
        ));
    }
    let mut output = BufWriter::new(io::stdout().lock());
    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let fft = Fft::new(FFT_SIZE);
    let mut stats = FxStats::default();
    let mut state = FxState::default();
    let mut mem = vec![0.0; cross_mod::mem_len(SAMPLE_RATE)];
    cross_mod::init(&mut state, &mut mem, SAMPLE_RATE);
    let mut scratch = vec![0.0; SCRATCH_FLOATS];
    let mut analysis = vec![0.0; ANALYSIS_FLOATS];
    let mut carrier = [0.0; BLOCK_FRAMES];
    let mut modulator = [0.0; BLOCK_FRAMES];
    let mut frames = 0;
    for (frame, samples) in input.chunks_exact(8).enumerate() {
        let left = f32::from_le_bytes([samples[0], samples[1], samples[2], samples[3]]);
        let right = f32::from_le_bytes([samples[4], samples[5], samples[6], samples[7]]);
        if !left.is_finite()
            || !right.is_finite()
            || !(-1.0..=1.0).contains(&left)
            || !(-1.0..=1.0).contains(&right)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("input frame {frame} must contain finite samples between -1 and 1"),
            ));
        }
        carrier[frames] = left;
        modulator[frames] = right;
        frames += 1;
        if frames == BLOCK_FRAMES {
            process_block(
                &mut output,
                params,
                frames,
                &mut carrier,
                &mut modulator,
                &mut state,
                &mut mem,
                &store,
                &fft,
                &caps,
                &mut scratch,
                &mut analysis,
                &mut stats,
            )?;
            frames = 0;
        }
    }
    if frames > 0 {
        process_block(
            &mut output,
            params,
            frames,
            &mut carrier,
            &mut modulator,
            &mut state,
            &mut mem,
            &store,
            &fft,
            &caps,
            &mut scratch,
            &mut analysis,
            &mut stats,
        )?;
    }
    output.flush()
}

fn main() -> io::Result<()> {
    let (params, input_path, frames) = parse_params()?;
    run(params, input_path, frames)
}
