//! Raw Vactr position 21-23 drum kernel probe for a separate source comparison.
//! Run with `CARGO_TERM_QUIET=true cargo run -q --example plaits_drums_reference --
//! <engine(kick|snare|hihat)> <harmonics> <timbre> <morph> <accent>`.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{dual_kick, hat_pair, snare_pair, Inp, Kx, NodeState, MAX_PORTS};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK_FRAMES: usize = 24;
const BLOCKS: usize = 3_000;

// Fixed per-engine notes fed to the pinned source's own `NoteToFrequency`.
const KICK_NOTE: f32 = 36.0;
const SNARE_NOTE: f32 = 57.0;
const HAT_NOTE: f32 = 81.0;

/// The pinned source's `NoteToFrequency` is `a0 * SemitonesToRatio(note - 9)`
/// with `a0 = (440 / 8) / 47_872.34`, and the drum engines then use that
/// value directly as a normalized frequency at their fixed 48 kHz processing
/// rate. Multiplying back by 48 kHz gives the equivalent Hz value the raw
/// Vactr kernel takes: `440 * 2^((note - 69) / 12) * 48_000 / 47_872.34`.
/// This is the same corrected-sample-rate conversion documented for the
/// position-10 FM probe (`fm_pair_reference.rs`), generalized to any note.
fn note_hz(note: f32) -> f32 {
    440.0 * 2f32.powf((note - 69.0) / 12.0) * (48_000.0 / 47_872.34)
}

type RenderFn = fn(&[Inp<'_>; MAX_PORTS], &mut NodeState, &mut [f32], &mut [f32], &Kx<'_>);

enum Engine {
    Kick,
    Snare,
    Hat,
}

struct Args {
    engine: Engine,
    harmonics: f32,
    timbre: f32,
    morph: f32,
    accent: f32,
}

fn parse_unit(text: &str) -> io::Result<f32> {
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

fn args() -> io::Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.len() != 5 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected: engine(kick|snare|hihat) harmonics timbre morph accent",
        ));
    }
    let engine = match raw[0].as_str() {
        "kick" => Engine::Kick,
        "snare" => Engine::Snare,
        "hihat" => Engine::Hat,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "engine must be kick, snare or hihat",
            ))
        }
    };
    Ok(Args {
        engine,
        harmonics: parse_unit(&raw[1])?,
        timbre: parse_unit(&raw[2])?,
        morph: parse_unit(&raw[3])?,
        accent: parse_unit(&raw[4])?,
    })
}

/// Runs one output node (mode 0.0 = main/analog, 1.0 = aux/synthetic) for
/// `BLOCKS` blocks from a freshly reset `NodeState`, which retriggers both
/// paths exactly once at the first sample of the whole run.
#[allow(clippy::too_many_arguments)]
fn render_mode(
    render: RenderFn,
    state_floats: usize,
    freq_hz: f32,
    args: &Args,
    mode: f32,
    seed: u32,
    sr: f32,
) -> io::Result<Vec<f32>> {
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    inputs[0] = Inp::Val(freq_hz);
    inputs[1] = Inp::Val(args.harmonics);
    inputs[2] = Inp::Val(args.timbre);
    inputs[3] = Inp::Val(args.morph);
    inputs[4] = Inp::Val(args.accent);
    inputs[5] = Inp::Val(mode);
    inputs[6] = Inp::Val(0.0); // kick/snare/hat-sustain: one-shot trigger.

    let store = SampleStore::default();
    let caps = CapabilitySet::native();
    let mut stats = FxStats::default();
    let kx = Kx {
        sr,
        gate: BLOCK_FRAMES,
        bank: None,
        store: &store,
        caps: &caps,
        stats: &mut stats,
        seed,
    };
    let mut state = NodeState::default();
    let mut mem = vec![0.0f32; state_floats];
    let mut out = [0.0f32; BLOCK_FRAMES];
    let mut all = Vec::with_capacity(BLOCK_FRAMES * BLOCKS);
    for _ in 0..BLOCKS {
        render(&inputs, &mut state, &mut mem, &mut out, &kx);
        for &sample in &out {
            if !sample.is_finite() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "nonfinite drum kernel output",
                ));
            }
        }
        all.extend_from_slice(&out);
    }
    Ok(all)
}

fn run(mut output: impl Write, args: Args) -> io::Result<()> {
    let (render, state_floats, note): (RenderFn, usize, f32) = match args.engine {
        Engine::Kick => (dual_kick::render, dual_kick::STATE_FLOATS, KICK_NOTE),
        Engine::Snare => (snare_pair::render, snare_pair::STATE_FLOATS, SNARE_NOTE),
        Engine::Hat => (hat_pair::render, hat_pair::STATE_FLOATS, HAT_NOTE),
    };
    let freq_hz = note_hz(note);
    if !freq_hz.is_finite() || freq_hz <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid comparison frequency",
        ));
    }

    let main = render_mode(render, state_floats, freq_hz, &args, 0.0, 1, SAMPLE_RATE)?;
    let aux = render_mode(render, state_floats, freq_hz, &args, 1.0, 1, SAMPLE_RATE)?;

    for (&main_sample, &aux_sample) in main.iter().zip(&aux) {
        writeln!(output, "{main_sample:.9} {aux_sample:.9}")?;
    }
    output.flush()
}

fn main() -> io::Result<()> {
    run(BufWriter::new(io::stdout().lock()), args()?)
}
