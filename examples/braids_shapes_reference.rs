//! Raw Vactr Braids macro-oscillator kernel probe for a separate source
//! comparison, generic across all 47 accessible source positions.
//!
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example
//! braids_shapes_reference -- <shape 0..46> <timbre 0..1> <color 0..1>`.
//!
//! Each source position dispatches to the family kernel that
//! `src/dsp/ported/braids.rs` maps it to (`braids_five`, `braids_subsync`,
//! `braids_triple`, `braids_digital`, `braids_filter`, `braids_formant`,
//! `braids_fm`, `braids_physical`, `braids_struck`, `braids_percussion`,
//! `braids_wave_bank`, `braids_wave_line`, `braids_noise`, `braids_cloud`),
//! passing the absolute source position as the family's own shape-select
//! input. Ports follow every kernel's uniform layout: 0 pitch (Hz), 1 shape
//! (absolute source position), 2 color, 3 timbre, 4 strike, 5 sync.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{
    braids_cloud, braids_digital, braids_filter, braids_five, braids_fm, braids_formant,
    braids_noise, braids_percussion, braids_physical, braids_struck, braids_subsync, braids_triple,
    braids_wave_bank, braids_wave_line, Inp, Kx, NodeState, MAX_PORTS,
};

const SAMPLE_RATE: f32 = 96_000.0;
const BLOCK_FRAMES: usize = 24;
const BLOCKS: usize = 200;
// MIDI note 60 (middle C) in standard equal temperament, matching the
// pinned firmware's own default quantizer target of "(60 << 7)" in its
// 128-units-per-semitone pitch code (see verification/braids_shapes_reference.cc).
// Vactr kernels take pitch directly in hertz.
const MIDI_NOTE_60_HZ: f32 = 261.625_58;

struct Args {
    position: u8,
    timbre: f32,
    color: f32,
}

fn parse_unit(text: &str) -> io::Result<f32> {
    let value: f32 = text
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "control must be a number"))?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "control must be finite and between 0 and 1",
        ));
    }
    Ok(value)
}

fn args() -> io::Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: shape(0..46) timbre(0..1) color(0..1)",
        ));
    }
    let position: u8 = raw[0]
        .parse()
        .ok()
        .filter(|value| *value <= 46)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "shape must be 0..=46"))?;
    Ok(Args {
        position,
        timbre: parse_unit(&raw[1])?,
        color: parse_unit(&raw[2])?,
    })
}

/// Render `BLOCKS` blocks of `BLOCK_FRAMES` samples for one source position,
/// dispatching to the family kernel that owns it. Strike fires once at the
/// first block (matching the reference driver's single upstream `Strike()`
/// call); sync stays at zero (matching the reference driver's zeroed sync
/// buffer).
fn render(position: u8, timbre: f32, color: f32, mut output: impl Write) -> io::Result<()> {
    #[allow(clippy::cast_precision_loss)]
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(MIDI_NOTE_60_HZ);
    ins[1] = Inp::Val(f32::from(position));
    ins[2] = Inp::Val(color);
    ins[3] = Inp::Val(timbre);
    ins[4] = Inp::Val(1.0);
    ins[5] = Inp::Val(0.0);

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
    let mut state = NodeState::default();
    let mut block = [0.0_f32; BLOCK_FRAMES];

    macro_rules! run_fixed {
        ($module:ident, $state_floats:expr) => {{
            let mut mem = vec![0.0_f32; $state_floats];
            for _ in 0..BLOCKS {
                $module::render(&ins, &mut state, &mut mem, &mut block, &kx);
                write_block(&mut output, &block)?;
            }
        }};
    }
    macro_rules! run_dynamic {
        ($module:ident) => {{
            let mut mem = vec![0.0_f32; $module::mem_len(SAMPLE_RATE)];
            for _ in 0..BLOCKS {
                $module::render(&ins, &mut state, &mut mem, &mut block, &kx);
                write_block(&mut output, &block)?;
            }
        }};
    }

    match position {
        0..=4 => run_fixed!(braids_five, braids_five::STATE_FLOATS),
        5..=8 => run_fixed!(braids_subsync, braids_subsync::STATE_FLOATS),
        9..=12 => run_fixed!(braids_triple, braids_triple::STATE_FLOATS),
        13..=16 => run_dynamic!(braids_digital),
        17..=20 => run_fixed!(braids_filter, braids_filter::STATE_FLOATS),
        21..=24 => run_fixed!(braids_formant, braids_formant::STATE_FLOATS),
        25..=27 => run_fixed!(braids_fm, braids_fm::STATE_FLOATS),
        28..=31 => run_dynamic!(braids_physical),
        32..=33 => run_fixed!(braids_struck, braids_struck::STATE_FLOATS),
        34..=36 => run_fixed!(braids_percussion, braids_percussion::STATE_FLOATS),
        37..=38 => run_fixed!(braids_wave_bank, braids_wave_bank::STATE_FLOATS),
        39..=40 => run_fixed!(braids_wave_line, braids_wave_line::STATE_FLOATS),
        41..=43 => run_fixed!(braids_noise, braids_noise::STATE_FLOATS),
        44..=46 => run_fixed!(braids_cloud, braids_cloud::STATE_FLOATS),
        _ => unreachable!("shape validated to 0..=46 in args()"),
    }
    output.flush()
}

fn write_block(mut output: impl Write, block: &[f32; BLOCK_FRAMES]) -> io::Result<()> {
    for sample in block {
        if !sample.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "nonfinite Braids kernel output",
            ));
        }
        writeln!(output, "{sample:.9}")?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let parsed = args()?;
    render(
        parsed.position,
        parsed.timbre,
        parsed.color,
        BufWriter::new(io::stdout().lock()),
    )
}
