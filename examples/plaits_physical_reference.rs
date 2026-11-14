//! Raw Vactr probe for Plaits positions 16–20.
//!
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example plaits_physical_reference -- \
//! <engine(swarm|noise|particle|string|modal)> <note> <harmonics> <timbre> <morph> <accent>`.
//! It writes 3,000 blocks of paired main/aux samples at 48 kHz. Main and aux
//! use separate, identically seeded kernel states and each receive one initial
//! event. Plaits' calibrated note conversion is retained for the raw Vactr Hz
//! input. Swarm and clocked noise have no accent port, so accent is ignored for
//! those two kernels.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{
    clock_noise_pair, modal_pair, particle_pair, string_pair, swarm_pair, Inp, Kx, NodeState,
    MAX_PORTS,
};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK_FRAMES: usize = 24;
const BLOCKS: usize = 3_000;

#[derive(Clone, Copy)]
enum Engine {
    Swarm,
    Noise,
    Particle,
    String,
    Modal,
}

struct Args {
    engine: Engine,
    note: f32,
    harmonics: f32,
    timbre: f32,
    morph: f32,
    accent: f32,
}

fn parse_finite(text: &str, field: &str) -> io::Result<f32> {
    let value = text.parse::<f32>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be a finite number"),
        )
    })?;
    if !value.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be a finite number"),
        ));
    }
    Ok(value)
}

fn parse_unit(text: &str, field: &str) -> io::Result<f32> {
    let value = parse_finite(text, field)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be between 0 and 1"),
        ));
    }
    Ok(value)
}

fn args() -> io::Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.len() != 6 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: engine(swarm|noise|particle|string|modal) note harmonics timbre morph accent",
        ));
    }
    let engine = match raw[0].as_str() {
        "swarm" => Engine::Swarm,
        "noise" => Engine::Noise,
        "particle" => Engine::Particle,
        "string" => Engine::String,
        "modal" => Engine::Modal,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "engine must be swarm, noise, particle, string or modal",
            ));
        }
    };
    let note = parse_finite(&raw[1], "note")?;
    if !(0.0..=127.0).contains(&note) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "note must be between 0 and 127",
        ));
    }
    Ok(Args {
        engine,
        note,
        harmonics: parse_unit(&raw[2], "harmonics")?,
        timbre: parse_unit(&raw[3], "timbre")?,
        morph: parse_unit(&raw[4], "morph")?,
        accent: parse_unit(&raw[5], "accent")?,
    })
}

/// Convert the pinned source's normalized NoteToFrequency value to the Hz
/// input used by the independent Vactr kernels at the same 48 kHz rate.
fn note_hz(note: f32) -> f32 {
    440.0 * 2.0_f32.powf((note - 69.0) / 12.0) * (SAMPLE_RATE / 47_872.34)
}

fn inputs(args: &Args, auxiliary: bool, frequency: f32) -> [Inp<'static>; MAX_PORTS] {
    let mut ins = [Inp::Val(0.0); MAX_PORTS];
    ins[0] = Inp::Val(frequency);
    match args.engine {
        Engine::Swarm => {
            // Ports: frequency, authored harmonics/spread, density, grain size,
            // sine-output selector, continuous-trigger selector.
            ins[1] = Inp::Val(args.harmonics);
            ins[2] = Inp::Val(args.timbre);
            ins[3] = Inp::Val(args.morph);
            ins[4] = Inp::Val(f32::from(auxiliary));
            ins[5] = Inp::Val(0.0);
        }
        Engine::Noise => {
            // Ports: note, harmonics, timbre, morph, aux band-output selector.
            ins[1] = Inp::Val(args.harmonics);
            ins[2] = Inp::Val(args.timbre);
            ins[3] = Inp::Val(args.morph);
            ins[4] = Inp::Val(f32::from(auxiliary));
        }
        Engine::Particle => {
            // Ports: note, authored structure/spread, density, diffusion/Q,
            // accent, impulse-output selector, sustain. Sustain is off so the
            // initialized event creates one triggered burst.
            ins[1] = Inp::Val(args.harmonics);
            ins[2] = Inp::Val(args.timbre);
            ins[3] = Inp::Val(args.morph);
            ins[4] = Inp::Val(args.accent);
            ins[5] = Inp::Val(f32::from(auxiliary));
            ins[6] = Inp::Val(0.0);
        }
        Engine::String => {
            // Ports: note, authored structure, brightness, damping, accent,
            // excitation-output selector, sustain.
            ins[1] = Inp::Val(args.harmonics);
            ins[2] = Inp::Val(args.timbre);
            ins[3] = Inp::Val(args.morph);
            ins[4] = Inp::Val(args.accent);
            ins[5] = Inp::Val(f32::from(auxiliary));
            ins[6] = Inp::Val(0.0);
        }
        Engine::Modal => {
            // Ports: note, authored structure, brightness, damping, accent,
            // excitation-output selector, sustain.
            ins[1] = Inp::Val(args.harmonics);
            ins[2] = Inp::Val(args.timbre);
            ins[3] = Inp::Val(args.morph);
            ins[4] = Inp::Val(args.accent);
            ins[5] = Inp::Val(f32::from(auxiliary));
            ins[6] = Inp::Val(0.0);
        }
    }
    ins
}

fn render_block(
    engine: Engine,
    ins: &[Inp<'_>; MAX_PORTS],
    state: &mut NodeState,
    memory: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
) {
    match engine {
        Engine::Swarm => swarm_pair::render(ins, state, memory, out, kx),
        Engine::Noise => clock_noise_pair::render(ins, state, out, kx),
        Engine::Particle => particle_pair::render(ins, state, memory, out, kx),
        Engine::String => string_pair::render(ins, state, memory, out, kx),
        Engine::Modal => modal_pair::render(ins, state, memory, out, kx),
    }
}

fn state_floats(engine: Engine) -> usize {
    match engine {
        Engine::Swarm => swarm_pair::STATE_FLOATS,
        Engine::Noise => 0,
        Engine::Particle => particle_pair::STATE_FLOATS,
        Engine::String => string_pair::STATE_FLOATS,
        Engine::Modal => modal_pair::STATE_FLOATS,
    }
}

fn run(mut output: impl Write, args: Args) -> io::Result<()> {
    let frequency = note_hz(args.note);
    if !frequency.is_finite() || frequency <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid comparison frequency",
        ));
    }
    let main_inputs = inputs(&args, false, frequency);
    let aux_inputs = inputs(&args, true, frequency);
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
    let state_size = state_floats(args.engine);
    let mut main_state = NodeState::default();
    let mut aux_state = NodeState::default();
    let mut main_memory = vec![0.0; state_size];
    let mut aux_memory = vec![0.0; state_size];
    let mut main = [0.0; BLOCK_FRAMES];
    let mut aux = [0.0; BLOCK_FRAMES];
    for _ in 0..BLOCKS {
        render_block(
            args.engine,
            &main_inputs,
            &mut main_state,
            &mut main_memory,
            &mut main,
            &kx,
        );
        render_block(
            args.engine,
            &aux_inputs,
            &mut aux_state,
            &mut aux_memory,
            &mut aux,
            &kx,
        );
        for (&main_sample, &aux_sample) in main.iter().zip(&aux) {
            if !main_sample.is_finite() || !aux_sample.is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "nonfinite physical-engine kernel output",
                ));
            }
            writeln!(output, "{main_sample:.9} {aux_sample:.9}")?;
        }
    }
    output.flush()
}

fn main() -> io::Result<()> {
    run(BufWriter::new(io::stdout().lock()), args()?)
}
