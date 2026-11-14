//! Raw Vactr kernel probe for Plaits oscillator positions 0, 1, 7, 8, 9, 11, 12.
//!
//! Usage: `CARGO_TERM_QUIET=true cargo run -q --example plaits_osc_reference -- \
//! <position> <note> <harmonics> <timbre> <morph> <accent> <trigger>`.
//! Prints one `main aux` pair per sample. Output is finite-checked and uses
//! 48 kHz, 24-frame blocks, with both paths rendered from fresh fixed state.
//!
//! Note conversion is `440 * 2^((note - 69) / 12) * (48_000 / 47_872.34)` Hz,
//! accounting for Plaits' corrected `NoteToFrequency` constant at this host
//! rate. Controls are normalized to [0, 1]. `accent` is accepted for a common
//! EngineParameters-shaped interface, but these oscillator kernels expose no
//! accent input. `trigger` selects the chip kernel's internal clocked mode at
//! position 7; other kernels have no trigger input and begin with reset state.
//!
//! Vactr mappings (the raw port order includes its output selector):
//! - 0: note -> `freq`; morph -> `va-source` morph; timbre -> filter cutoff;
//!   harmonics -> `filter-harmonics`. Render `va-source` once, then `va-filter`
//!   mode 0 (main low-pass) and mode 1 (aux high-pass). Accent/trigger unused.
//! - 1: note -> `freq`; harmonics -> `phase-harmonics`; timbre/morph direct;
//!   mode 0 synchronized main, mode 1 free-running aux. Accent/trigger unused.
//! - 7: note -> `freq`; harmonics -> `chip-chord`; timbre/morph direct;
//!   `chip-clocked` <- trigger, `chip-rate` fixed at 8; mode 0 selects the
//!   chord/arpeggio main and mode 1 the stepped-triangle aux. Accent unused.
//! - 8: note -> `freq`; harmonics -> `analog-detune`; timbre/morph direct;
//!   mode 0 variable square/saw main, mode 1 synchronized-shape difference
//!   aux. Accent/trigger unused.
//! - 9: note -> `freq`; harmonics -> `shape-harmonics`; timbre/morph direct;
//!   mode 0 folded slope main, mode 1 overtone aux. Accent/trigger unused.
//! - 11: note -> `freq`; harmonics/timbre/morph direct; mode 0 grainlet main,
//!   mode 1 Z-oscillator aux. Accent/trigger unused.
//! - 12: note -> `freq`; timbre -> source centroid, morph direct, harmonics ->
//!   `spectrum-bumps`; mode 0 integer-series main, mode 1 organ-series aux.
//!   Accent/trigger unused.
//!
//! All mappings follow the per-position notices and implementation plans.
//! They compare raw kernels only, not Plaits' outer voice, LPG, or event path.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{
    analog_pair, chip_pair, grain_pair, phase_pair, shape_pair, spectrum_pair, va_filter, Inp, Kx,
    NodeState, MAX_PORTS,
};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK_FRAMES: usize = 24;
const BLOCKS: usize = 3_000;

#[derive(Clone, Copy)]
struct Scenario {
    position: u8,
    note: f32,
    harmonics: f32,
    timbre: f32,
    morph: f32,
    accent: f32,
    trigger: f32,
}

fn parse_float(text: &str, label: &str, min: f32, max: f32) -> io::Result<f32> {
    let value = text.parse::<f32>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be numeric"),
        )
    })?;
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be finite and in [{min}, {max}]"),
        ));
    }
    Ok(value)
}

fn parse_args() -> io::Result<Scenario> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 7 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: position(0|1|7|8|9|11|12) note(0..127) harmonics timbre morph accent trigger(0|1)",
        ));
    }
    let position = args[0]
        .parse::<u8>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "position must be an integer"))?;
    if ![0, 1, 7, 8, 9, 11, 12].contains(&position) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "position must be one of 0, 1, 7, 8, 9, 11, 12",
        ));
    }
    let unit = |index: usize, label: &str| parse_float(&args[index], label, 0.0, 1.0);
    Ok(Scenario {
        position,
        note: parse_float(&args[1], "note", 0.0, 127.0)?,
        harmonics: unit(2, "harmonics")?,
        timbre: unit(3, "timbre")?,
        morph: unit(4, "morph")?,
        accent: unit(5, "accent")?,
        trigger: parse_float(&args[6], "trigger", 0.0, 1.0)?,
    })
}

fn note_hz(note: f32) -> f32 {
    440.0 * 2.0_f32.powf((note - 69.0) / 12.0) * (48_000.0 / 47_872.34)
}

fn inputs<'a>(scenario: Scenario, mode: f32) -> [Inp<'a>; MAX_PORTS] {
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    let hz = note_hz(scenario.note);
    match scenario.position {
        0 => {
            ins[0] = Inp::Val(hz);
            ins[1] = Inp::Val(scenario.morph);
            ins[2] = Inp::Val(scenario.timbre);
            ins[3] = Inp::Val(scenario.harmonics);
            ins[4] = Inp::Val(mode);
        }
        1 => {
            ins[0] = Inp::Val(hz);
            ins[1] = Inp::Val(scenario.harmonics);
            ins[2] = Inp::Val(scenario.timbre);
            ins[3] = Inp::Val(scenario.morph);
            ins[4] = Inp::Val(mode);
        }
        7 => {
            ins[0] = Inp::Val(hz);
            ins[1] = Inp::Val(scenario.harmonics);
            ins[2] = Inp::Val(scenario.timbre);
            ins[3] = Inp::Val(scenario.morph);
            ins[4] = Inp::Val(mode); // source template's mode selects main or bass.
            ins[5] = Inp::Val(scenario.trigger);
            ins[6] = Inp::Val(8.0); // chip-rate Vactr extension, not source control.
        }
        8 => {
            ins[0] = Inp::Val(hz);
            ins[1] = Inp::Val(scenario.harmonics);
            ins[2] = Inp::Val(scenario.timbre);
            ins[3] = Inp::Val(scenario.morph);
            ins[4] = Inp::Val(mode);
        }
        9 | 11 => {
            ins[0] = Inp::Val(hz);
            ins[1] = Inp::Val(scenario.harmonics);
            ins[2] = Inp::Val(scenario.timbre);
            ins[3] = Inp::Val(scenario.morph);
            ins[4] = Inp::Val(mode);
        }
        12 => {
            ins[0] = Inp::Val(hz);
            ins[1] = Inp::Val(scenario.timbre); // source centroid role
            ins[2] = Inp::Val(scenario.morph);
            ins[3] = Inp::Val(scenario.harmonics); // spectrum-bumps
            ins[4] = Inp::Val(mode);
        }
        _ => unreachable!("validated position"),
    }
    ins
}

fn run(mut output: impl Write, scenario: Scenario) -> io::Result<()> {
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
    let mut main_mem = vec![0.0; memory_size(scenario.position)];
    let mut aux_mem = vec![0.0; memory_size(scenario.position)];
    let mut source_state = NodeState::default();
    let mut source = [0.0; BLOCK_FRAMES];
    let mut main = [0.0; BLOCK_FRAMES];
    let mut aux = [0.0; BLOCK_FRAMES];
    let mut all_main = Vec::with_capacity(BLOCKS * BLOCK_FRAMES);
    let mut all_aux = Vec::with_capacity(BLOCKS * BLOCK_FRAMES);
    let source_inputs = inputs(scenario, 0.0);
    let main_inputs = inputs(scenario, 0.0);
    let aux_inputs = inputs(scenario, 1.0);

    for _ in 0..BLOCKS {
        match scenario.position {
            0 => {
                va_filter::source(&source_inputs, &mut source_state, &mut source, &kx);
                let mut lp_inputs = main_inputs;
                // The filter consumes the oscillator buffer through its first
                // input; Inp::Buf is the zero-copy block connection.
                lp_inputs[0] = Inp::Buf(&source);
                let mut hp_inputs = lp_inputs;
                hp_inputs[4] = Inp::Val(1.0);
                va_filter::filter(&lp_inputs, &mut main_state, &mut main, &kx);
                va_filter::filter(&hp_inputs, &mut aux_state, &mut aux, &kx);
            }
            1 => {
                phase_pair::render(&main_inputs, &mut main_state, &mut main, &kx);
                phase_pair::render(&aux_inputs, &mut aux_state, &mut aux, &kx);
            }
            7 => {
                chip_pair::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, &kx);
                chip_pair::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, &kx);
            }
            8 => {
                analog_pair::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, &kx);
                analog_pair::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, &kx);
            }
            9 => {
                shape_pair::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, &kx);
                shape_pair::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, &kx);
            }
            11 => {
                grain_pair::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, &kx);
                grain_pair::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, &kx);
            }
            12 => {
                spectrum_pair::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, &kx);
                spectrum_pair::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, &kx);
            }
            _ => unreachable!("validated position"),
        }
        if main.iter().chain(&aux).any(|sample| !sample.is_finite()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "nonfinite oscillator kernel output",
            ));
        }
        all_main.extend_from_slice(&main);
        all_aux.extend_from_slice(&aux);
    }
    for (main_sample, aux_sample) in all_main.iter().zip(&all_aux) {
        writeln!(output, "{main_sample:.9} {aux_sample:.9}")?;
    }
    output.flush()
}

fn memory_size(position: u8) -> usize {
    match position {
        7 => chip_pair::STATE_FLOATS,
        8 => analog_pair::STATE_FLOATS,
        9 => shape_pair::STATE_FLOATS,
        11 => grain_pair::STATE_FLOATS,
        12 => 48,
        _ => 0,
    }
}

fn main() -> io::Result<()> {
    let scenario = parse_args()?;
    // Accent is intentionally parsed and validated, although no listed raw
    // oscillator kernel or paired Vactr template exposes an accent port.
    let _unused_accent = scenario.accent;
    run(BufWriter::new(io::stdout().lock()), scenario)
}
