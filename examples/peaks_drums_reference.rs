//! Raw Vactr drum-kernel probe for a separate Peaks source comparison.
//! Drives `analog_percussion::render` (low-drum/wire-drum/metal-hat, modes
//! 0/1/2) and `fm_drum::render` (phase-drum) directly, at the pinned
//! source's own 48 kHz rate, for one triggered hit with fixed control
//! values held for the whole render.
//!
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example peaks_drums_reference`.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{analog_percussion, fm_drum, Inp, Kx, NodeState, MAX_PORTS};

const SAMPLE_RATE: f32 = 48_000.0;
const FRAMES: usize = 120_000;

struct Args {
    drum: String,
    values: Vec<f32>,
}

fn parse_args() -> io::Result<Args> {
    let mut args = std::env::args().skip(1);
    let drum = args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a drum name: bass, snare, hat or fm",
        )
    })?;
    let mut values = Vec::new();
    for arg in args {
        let value: f32 = arg.parse().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "control must be a finite number",
            )
        })?;
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "control must be finite",
            ));
        }
        values.push(value);
    }
    Ok(Args { drum, values })
}

/// `analog-percussion` ports are `[freq, punch, tone, decay, snappy, metal,
/// mode]`; each drum's own controls are documented in
/// `verification/compare_peaks_drums.py`, and unused ports keep the
/// `src/prelude/templates.vact` defaults.
fn analog_percussion_inputs(
    mode: f32,
    freq: f32,
    punch: f32,
    tone: f32,
    decay: f32,
    snappy: f32,
    metal: f32,
) -> [Inp<'static>; MAX_PORTS] {
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(freq);
    ins[1] = Inp::Val(punch);
    ins[2] = Inp::Val(tone);
    ins[3] = Inp::Val(decay);
    ins[4] = Inp::Val(snappy);
    ins[5] = Inp::Val(metal);
    ins[6] = Inp::Val(mode);
    ins
}

/// `fm-drum` ports are `[freq, fm-amount, pitch-sweep, decay, drum-noise,
/// drive]`, matching `src/dsp/ugen/catalog.rs`'s `FM_DRUM` port list.
fn fm_drum_inputs(
    freq: f32,
    fm_amount: f32,
    pitch_sweep: f32,
    decay: f32,
    noise: f32,
    drive: f32,
) -> [Inp<'static>; MAX_PORTS] {
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(freq);
    ins[1] = Inp::Val(fm_amount);
    ins[2] = Inp::Val(pitch_sweep);
    ins[3] = Inp::Val(decay);
    ins[4] = Inp::Val(noise);
    ins[5] = Inp::Val(drive);
    ins
}

fn build_inputs(args: &Args) -> io::Result<[Inp<'static>; MAX_PORTS]> {
    let v = &args.values;
    match args.drum.as_str() {
        "bass" if v.len() == 4 => Ok(analog_percussion_inputs(
            0.0, v[0], v[1], v[2], v[3], 0.5, 0.5,
        )),
        "snare" if v.len() == 4 => Ok(analog_percussion_inputs(
            1.0, v[0], 0.5, v[1], v[3], v[2], 0.5,
        )),
        "hat" if v.len() == 4 => Ok(analog_percussion_inputs(
            2.0, v[0], 0.5, v[1], v[2], 0.5, v[3],
        )),
        "fm" if v.len() == 6 => Ok(fm_drum_inputs(v[0], v[1], v[2], v[3], v[4], v[5])),
        "bass" | "snare" | "hat" => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bass/snare/hat expect 4 controls",
        )),
        "fm" => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "fm expects 6 controls: freq fm-amount pitch-sweep decay noise drive",
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown drum: expected bass, snare, hat or fm",
        )),
    }
}

fn run(mut output: impl Write, args: &Args) -> io::Result<()> {
    let ins = build_inputs(args)?;
    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr: SAMPLE_RATE,
        gate: FRAMES,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed: 1,
    };
    let mut state = NodeState::default();
    let mut buffer = vec![0.0_f32; FRAMES];
    if args.drum == "fm" {
        fm_drum::render(&ins, &mut state, &mut buffer, &kx);
    } else {
        analog_percussion::render(&ins, &mut state, &mut buffer, &kx);
    }
    for sample in &buffer {
        if !sample.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "nonfinite Vactr drum output",
            ));
        }
        writeln!(output, "{sample:.9}")?;
    }
    output.flush()
}

fn main() -> io::Result<()> {
    let args = parse_args()?;
    run(BufWriter::new(io::stdout().lock()), &args)
}
