//! Raw Vactr `resonator-part-core` / `string-choir-core` kernel probe for a
//! separate source comparison (RNG-006). Neither kernel is a source port; see
//! `src/dsp/ported/rings.rs` and `THIRD_PARTY_NOTICES.md`.
//!
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example rings_part_reference
//! -- <target> <structure> <brightness> <damping> <position>`, where `target`
//! is `0`..`5` (the `rings::ResonatorModel` ordinal) or `stringsynth`.
//! Structure/brightness/damping/position are each `0..1`. Polyphony, strum,
//! the internal exciter and tonic/note/fm are fixed to match the pinned
//! `verification/rings_part_reference.cc` probe: an internal strum/exciter
//! trigger fires once at the start of the render, and the note is fixed so
//! that the event frequency (440 Hz) passes through unchanged.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{rings_part, string_choir, Inp, Kx, NodeState, MAX_PORTS};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK_FRAMES: usize = 24; // rings::kMaxBlockSize
const BLOCKS: usize = 4_000; // 2.0 s at 48 kHz, matching the pinned probe.
const TOTAL_FRAMES: usize = BLOCK_FRAMES * BLOCKS;
/// A4, matching the pinned probe's fixed note 69 with tonic/fm at zero
/// (`SemitonesToRatio(69 - 69) * a3` is exactly 440 Hz).
const FIXED_HZ: f32 = 440.0;
/// `rings::StringSynthPart::Init` defaults `fx_type_` to `FX_ENSEMBLE` (4).
const STRING_SYNTH_FX: f32 = 4.0;

enum Target {
    Model(u8),
    StringSynth,
}

struct Controls {
    target: Target,
    structure: f32,
    brightness: f32,
    damping: f32,
    position: f32,
}

fn unit_arg(text: &str) -> io::Result<f32> {
    let value: f32 = text.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "control must be a finite number",
        )
    })?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "control must be finite and between 0 and 1",
        ));
    }
    Ok(value)
}

fn controls() -> io::Result<Controls> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.len() != 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: target structure brightness damping position",
        ));
    }
    let target = if raw[0] == "stringsynth" {
        Target::StringSynth
    } else {
        let model: u8 = raw[0].parse().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "target must be 0..5 or stringsynth",
            )
        })?;
        if model > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "target must be 0..5 or stringsynth",
            ));
        }
        Target::Model(model)
    };
    Ok(Controls {
        target,
        structure: unit_arg(&raw[1])?,
        brightness: unit_arg(&raw[2])?,
        damping: unit_arg(&raw[3])?,
        position: unit_arg(&raw[4])?,
    })
}

/// Renders the six-model `resonator-part-core` kernel: two independent node
/// instances (one per channel), matching `resonator-voice`'s pairing of a
/// `mode: 0` and a `mode: 1` node in `src/prelude/templates.vact`.
fn render_rings_part(cfg: &Controls, model: u8, kx: &Kx<'_>) -> (Vec<f32>, Vec<f32>) {
    let mut base = [Inp::Val(0.0); MAX_PORTS];
    base[0] = Inp::Val(FIXED_HZ);
    base[1] = Inp::Val(f32::from(model));
    base[2] = Inp::Val(cfg.structure);
    base[3] = Inp::Val(cfg.brightness);
    base[4] = Inp::Val(cfg.damping);
    base[5] = Inp::Val(cfg.position);
    base[6] = Inp::Val(1.0); // strum amount
    base[7] = Inp::Val(1.0); // internal_exciter
    base[8] = Inp::Val(1.0); // internal_strum
    base[9] = Inp::Val(1.0); // internal_note
    base[10] = Inp::Val(0.0); // tonic
    base[11] = Inp::Val(0.0); // note
    base[12] = Inp::Val(0.0); // fm
    base[13] = Inp::Val(0.0); // chord
    base[14] = Inp::Val(1.0); // polyphony
    base[16] = Inp::Val(0.0); // external excitation: unused (internal only)

    let mut main_inputs = base;
    main_inputs[15] = Inp::Val(0.0);
    let mut aux_inputs = base;
    aux_inputs[15] = Inp::Val(1.0);

    let mut main_mem = vec![0.0_f32; rings_part::mem_len(SAMPLE_RATE)];
    let mut aux_mem = vec![0.0_f32; rings_part::mem_len(SAMPLE_RATE)];
    let mut main_state = NodeState::default();
    let mut aux_state = NodeState::default();
    let mut main = vec![0.0_f32; TOTAL_FRAMES];
    let mut aux = vec![0.0_f32; TOTAL_FRAMES];
    rings_part::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, kx);
    rings_part::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, kx);
    (main, aux)
}

/// Renders the separate `string-choir-core` path the same way.
fn render_string_choir(cfg: &Controls, kx: &Kx<'_>) -> (Vec<f32>, Vec<f32>) {
    let mut base = [Inp::Val(0.0); MAX_PORTS];
    base[0] = Inp::Val(FIXED_HZ);
    base[1] = Inp::Val(cfg.structure);
    base[2] = Inp::Val(cfg.brightness);
    base[3] = Inp::Val(cfg.damping);
    base[4] = Inp::Val(cfg.position);
    base[5] = Inp::Val(1.0); // strum amount
    base[6] = Inp::Val(1.0); // exciter
    base[7] = Inp::Val(1.0); // internal_strum
    base[8] = Inp::Val(1.0); // internal_note
    base[9] = Inp::Val(0.0); // tonic
    base[10] = Inp::Val(0.0); // note
    base[11] = Inp::Val(0.0); // fm
    base[12] = Inp::Val(0.0); // chord
    base[13] = Inp::Val(1.0); // polyphony
    base[14] = Inp::Val(STRING_SYNTH_FX);

    let mut main_inputs = base;
    main_inputs[15] = Inp::Val(0.0);
    let mut aux_inputs = base;
    aux_inputs[15] = Inp::Val(1.0);

    let mut main_mem = vec![0.0_f32; string_choir::mem_len(SAMPLE_RATE)];
    let mut aux_mem = vec![0.0_f32; string_choir::mem_len(SAMPLE_RATE)];
    let mut main_state = NodeState::default();
    let mut aux_state = NodeState::default();
    let mut main = vec![0.0_f32; TOTAL_FRAMES];
    let mut aux = vec![0.0_f32; TOTAL_FRAMES];
    string_choir::render(&main_inputs, &mut main_state, &mut main_mem, &mut main, kx);
    string_choir::render(&aux_inputs, &mut aux_state, &mut aux_mem, &mut aux, kx);
    (main, aux)
}

fn run(mut output: impl Write, cfg: Controls) -> io::Result<()> {
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

    let (main, aux) = match cfg.target {
        Target::Model(model) => render_rings_part(&cfg, model, &kx),
        Target::StringSynth => render_string_choir(&cfg, &kx),
    };

    for (&main, &aux) in main.iter().zip(&aux) {
        if !main.is_finite() || !aux.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "nonfinite Vactr resonator output",
            ));
        }
        writeln!(output, "{main:.9} {aux:.9}")?;
    }
    output.flush()
}

fn main() -> io::Result<()> {
    run(BufWriter::new(io::stdout().lock()), controls()?)
}
