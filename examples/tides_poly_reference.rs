//! Raw four-lane Vactr Tides2 ramp probe for the TID-006 source comparison.
//!
//! Usage: `CARGO_TERM_QUIET=true cargo run -q --example tides_poly_reference --
//! <mode 0..2> <output 0..3> <range 0..1> <frequency-hz> <pw> <shape>
//! <smoothness> <shift> [frames]`
//!
//! Output is headerless whitespace-separated rows with lanes 0 through 3.
//! Modes are AD=0, looping=1, AR=2; output roles are
//! GATES=0, AMPLITUDE=1, PHASE=2, FREQUENCY=3. Range 0 applies the kernel's
//! low-range 0.02 frequency multiplier, while range 1 uses frequency-hz
//! directly. `frequency-hz` means effective frequency after Vactr's range
//! scaling; the probe compensates the raw kernel input for that multiplier.
//! The four shape controls are raw kernel controls in [0, 1]; PW is mapped
//! internally to 0.04..0.96. The gate is high for the first 24 frames
//! and then low. Clock remains low. Each lane has separate persistent kernel
//! state, and output is ordered by sample then lane.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{tidal_function, tidal_poly, Inp, Kx, NodeState, MAX_PORTS};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK_FRAMES: usize = 24;
const DEFAULT_FRAMES: usize = 96_000;

#[derive(Clone, Copy)]
struct Args {
    mode: u8,
    output: u8,
    range: u8,
    frequency_hz: f32,
    pw: f32,
    shape: f32,
    smoothness: f32,
    shift: f32,
    frames: usize,
}

#[derive(Clone, Copy)]
struct Tides1Args {
    mode: u8,
    frequency_hz: f32,
    shape: f32,
    slope: f32,
    smoothness: f32,
    frames: usize,
}

#[derive(Clone, Copy)]
enum ProbeArgs {
    Tides1(Tides1Args),
    Tides2(Args),
}

fn parse_index(text: &str, name: &str, max: u8) -> io::Result<u8> {
    let value = text.parse::<u8>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be an integer"),
        )
    })?;
    if value > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be between 0 and {max}"),
        ));
    }
    Ok(value)
}

fn parse_finite(text: &str, name: &str) -> io::Result<f32> {
    let value = text.parse::<f32>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be a number"),
        )
    })?;
    if !value.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be finite"),
        ));
    }
    Ok(value)
}

fn parse_unit(text: &str, name: &str) -> io::Result<f32> {
    let value = parse_finite(text, name)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be between 0 and 1"),
        ));
    }
    Ok(value)
}

fn args() -> io::Result<ProbeArgs> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().is_some_and(|arg| arg == "tides1") {
        if raw.len() != 7 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected: tides1 mode(0..2) frequency-hz shape slope smoothness frames",
            ));
        }
        let frequency_hz = parse_finite(&raw[2], "frequency-hz")?;
        if frequency_hz <= 0.0 || frequency_hz > SAMPLE_RATE * 0.4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "frequency-hz must be positive and no greater than 19200",
            ));
        }
        let frames = raw[6].parse::<usize>().map_err(|_| {
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
        return Ok(ProbeArgs::Tides1(Tides1Args {
            mode: parse_index(&raw[1], "mode", 2)?,
            frequency_hz,
            shape: parse_unit(&raw[3], "shape")?,
            slope: parse_unit(&raw[4], "slope")?,
            smoothness: parse_unit(&raw[5], "smoothness")?,
            frames,
        }));
    }
    if !(8..=9).contains(&raw.len()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: mode(0..2) output(0..3) range(0..1) frequency-hz pw shape smoothness shift [frames]",
        ));
    }
    let frequency_hz = parse_finite(&raw[3], "frequency-hz")?;
    let range = parse_index(&raw[2], "range", 1)?;
    let max_frequency = SAMPLE_RATE * 0.24 * if range == 0 { 0.02 } else { 1.0 };
    if frequency_hz <= 0.0 || frequency_hz > max_frequency {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("frequency-hz must be positive and no greater than {max_frequency}"),
        ));
    }
    let frames = if let Some(text) = raw.get(8) {
        text.parse::<usize>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "frames must be a positive integer",
            )
        })?
    } else {
        DEFAULT_FRAMES
    };
    if frames == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frames must be a positive integer",
        ));
    }
    Ok(ProbeArgs::Tides2(Args {
        mode: parse_index(&raw[0], "mode", 2)?,
        output: parse_index(&raw[1], "output", 3)?,
        range,
        frequency_hz,
        pw: parse_unit(&raw[4], "pw")?,
        shape: parse_unit(&raw[5], "shape")?,
        smoothness: parse_unit(&raw[6], "smoothness")?,
        shift: parse_unit(&raw[7], "shift")?,
        frames,
    }))
}

fn lane_inputs(args: Args, lane: usize, first_frame: usize) -> [Inp<'static>; MAX_PORTS] {
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    let range_scale = if args.range == 0 { 0.02 } else { 1.0 };
    inputs[0] = Inp::Val(args.frequency_hz / range_scale);
    inputs[1] = Inp::Val(args.pw);
    inputs[2] = Inp::Val(args.shape);
    inputs[3] = Inp::Val(args.smoothness);
    inputs[4] = Inp::Val(args.shift);
    inputs[5] = Inp::Val(f32::from(first_frame < BLOCK_FRAMES));
    inputs[6] = Inp::Val(0.0); // No external clock edges.
    inputs[7] = Inp::Val(f32::from(args.mode));
    inputs[8] = Inp::Val(f32::from(args.output));
    inputs[9] = Inp::Val(f32::from(args.range));
    inputs[10] = Inp::Val(lane as f32);
    inputs
}

fn run(mut output: impl Write, args: Args) -> io::Result<()> {
    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: SAMPLE_RATE,
        gate: BLOCK_FRAMES,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut states = [NodeState::default(); 4];
    let mut memories = [[0.0_f32; tidal_poly::STATE_FLOATS]; 4];
    let mut outputs = [[0.0_f32; BLOCK_FRAMES]; 4];

    for first_frame in (0..args.frames).step_by(BLOCK_FRAMES) {
        let block_len = (args.frames - first_frame).min(BLOCK_FRAMES);
        for lane in 0..4 {
            let inputs = lane_inputs(args, lane, first_frame);
            tidal_poly::render(
                &inputs,
                &mut states[lane],
                &mut memories[lane],
                &mut outputs[lane][..block_len],
                &kx,
            );
        }
        for offset in 0..block_len {
            let mut separator = "";
            for lane in &outputs {
                let sample = lane[offset];
                if !sample.is_finite() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "nonfinite Tides poly kernel output",
                    ));
                }
                write!(output, "{separator}{sample:.9}")?;
                separator = " ";
            }
            writeln!(output)?;
        }
    }
    output.flush()
}

fn run_tides1(mut output: impl Write, args: Tides1Args) -> io::Result<()> {
    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: SAMPLE_RATE,
        gate: BLOCK_FRAMES,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut states = [NodeState::default(); 2];
    let mut memories = [[0.0_f32; tidal_function::STATE_FLOATS]; 2];
    let mut outputs = [[0.0_f32; BLOCK_FRAMES]; 2];

    for first_frame in (0..args.frames).step_by(BLOCK_FRAMES) {
        let block_len = (args.frames - first_frame).min(BLOCK_FRAMES);
        for lane in 0..2 {
            let mut inputs = [Inp::Val(0.0); MAX_PORTS];
            inputs[0] = Inp::Val(args.frequency_hz);
            inputs[1] = Inp::Val(args.shape);
            inputs[2] = Inp::Val(args.slope);
            inputs[3] = Inp::Val(args.smoothness);
            inputs[4] = Inp::Val(1.0); // Ratio control.
            inputs[6] = Inp::Val(f32::from(first_frame < BLOCK_FRAMES));
            inputs[9] = Inp::Val(f32::from(args.mode));
            inputs[10] = Inp::Val(0.0); // High range.
            inputs[12] = Inp::Val(lane as f32); // Unipolar, then bipolar.
            tidal_function::render(
                &inputs,
                &mut states[lane],
                &mut memories[lane],
                &mut outputs[lane][..block_len],
                &kx,
            );
        }
        for offset in 0..block_len {
            for (lane_index, lane) in outputs.iter().enumerate() {
                let sample = lane[offset];
                if !sample.is_finite() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "nonfinite Tides function kernel output",
                    ));
                }
                if lane_index > 0 {
                    write!(output, " ")?;
                }
                write!(output, "{sample:.9}")?;
            }
            writeln!(output)?;
        }
    }
    output.flush()
}

fn main() -> io::Result<()> {
    let output = BufWriter::new(io::stdout().lock());
    match args()? {
        ProbeArgs::Tides1(args) => run_tides1(output, args),
        ProbeArgs::Tides2(args) => run(output, args),
    }
}
