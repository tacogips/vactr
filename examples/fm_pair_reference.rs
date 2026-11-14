//! Raw Vactr position-10 FM kernel probe for a separate source comparison.
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example fm_pair_reference`.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{fm_pair, Inp, Kx, NodeState, MAX_PORTS};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK_FRAMES: usize = 24;
const BLOCKS: usize = 100;

fn controls() -> io::Result<[f32; 3]> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Ok([0.5; 3]);
    }
    if args.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected three controls: harmonics timbre morph",
        ));
    }
    let mut values = [0.0; 3];
    for (value, arg) in values.iter_mut().zip(args) {
        *value = arg.parse::<f32>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "control must be a finite number",
            )
        })?;
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "control must be finite and between 0 and 1",
            ));
        }
    }
    Ok(values)
}

fn run(mut output: impl Write, controls: [f32; 3]) -> io::Result<()> {
    // Match the pinned source's audible note-69 carrier after its calibrated
    // NoteToFrequency conversion and fourfold phase advance. The raw Vactr
    // kernel takes hertz.
    let carrier_hz: f32 = 440.0 * 48_000.0 / 47_872.34;
    if !carrier_hz.is_finite() || carrier_hz <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid comparison frequency",
        ));
    }
    let mut main_inputs = [Inp::Val(0.0); MAX_PORTS];
    main_inputs[0] = Inp::Val(carrier_hz);
    main_inputs[1] = Inp::Val(controls[0]);
    main_inputs[2] = Inp::Val(controls[1]);
    main_inputs[3] = Inp::Val(controls[2]);
    let mut aux_inputs = main_inputs;
    aux_inputs[4] = Inp::Val(1.0);

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
    let mut main_state = NodeState::default();
    let mut aux_state = NodeState::default();
    let mut main = [0.0; BLOCK_FRAMES];
    let mut aux = [0.0; BLOCK_FRAMES];
    for _ in 0..BLOCKS {
        fm_pair::render(&main_inputs, &mut main_state, &mut main, &kx);
        fm_pair::render(&aux_inputs, &mut aux_state, &mut aux, &kx);
        for (&carrier, &sub) in main.iter().zip(&aux) {
            if !carrier.is_finite() || !sub.is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "nonfinite FM kernel output",
                ));
            }
            writeln!(output, "{carrier:.9} {sub:.9}")?;
        }
    }
    output.flush()
}

fn main() -> io::Result<()> {
    run(BufWriter::new(io::stdout().lock()), controls()?)
}
