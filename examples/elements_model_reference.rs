//! Deterministic raw-kernel probe for the `exciter-voice` Elements adaptation.
//!
//! Usage: `CARGO_TERM_QUIET=true cargo run -q --example elements_model_reference -- <model> <scenario>`
//! Models are `modal`, `string`, and `strings`. Scenarios are listed in
//! `SCENARIOS`; bow-only scenarios are the fair procedural-exciter comparison,
//! while `sample-blow` and `sample-strike` use the source's sample-dependent
//! exciters and are explicitly replacement comparisons.
//!
//! The input ports passed to `elements_internal::render` follow the raw kernel
//! contract: 0 frequency in Hz; 1–20 are `ex-env-shape`, `ex-bow-level`,
//! `ex-bow-timbre`, `ex-blow-level`, `ex-blow-meta`, `ex-blow-timbre`,
//! `ex-strike-level`, `ex-strike-meta`, `ex-strike-timbre`, `ex-signature`,
//! `ex-geometry`, `ex-brightness`, `ex-damping`, `ex-position`,
//! `ex-res-mod-frequency`, `ex-res-mod-offset`, `ex-reverb-diffusion`,
//! `ex-reverb-lp`, `ex-space`, and `ex-modulation-frequency`; 21 is
//! `ex-gate`, 22 `ex-note` in semitones, 23 `ex-modulation`, 24 `ex-strength`,
//! 25 `ex-model` (0 modal, 1 string, 2 strings), 26 the main/aux selector,
//! 27 `ex-alternate`, 28 `ex-external-blow`, and 29 `ex-external-strike`.
//! All `ex-*` values are supplied as block constants. The selector, external
//! inputs, and alternate path are held at zero; gate is held on. Frequency is
//! C4 (note 60, 261.625565 Hz), sample rate is 32 kHz, and blocks are 16 frames.
//!
//! Output is NDJSON: one scenario metadata record, followed by one record per
//! frame containing `frame`, `main`, and `aux` samples. No upstream code or
//! sample data is referenced by this probe.

use std::io::{self, BufWriter, Write};

use vactr::dsp::arena::SampleStore;
use vactr::dsp::caps::CapabilitySet;
use vactr::dsp::effects::FxStats;
use vactr::dsp::ugen::{elements_internal, Inp, Kx, NodeState, MAX_PORTS};

const SAMPLE_RATE: f32 = 32_000.0;
const BLOCK_FRAMES: usize = 16;
const TOTAL_FRAMES: usize = 3 * SAMPLE_RATE as usize;
const FREQUENCY_HZ: f32 = 261.625_58;

const CONTROL_NAMES: [&str; 20] = [
    "ex-env-shape",
    "ex-bow-level",
    "ex-bow-timbre",
    "ex-blow-level",
    "ex-blow-meta",
    "ex-blow-timbre",
    "ex-strike-level",
    "ex-strike-meta",
    "ex-strike-timbre",
    "ex-signature",
    "ex-geometry",
    "ex-brightness",
    "ex-damping",
    "ex-position",
    "ex-res-mod-frequency",
    "ex-res-mod-offset",
    "ex-reverb-diffusion",
    "ex-reverb-lp",
    "ex-space",
    "ex-modulation-frequency",
];

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    classification: &'static str,
    patch: [f32; 20],
}

// Patch order is the documented ex-* port order 1 through 20 above.
const SCENARIOS: [Scenario; 5] = [
    Scenario {
        name: "bow-warm",
        classification: "fair-bow-only",
        patch: [
            0.50, 0.72, 0.18, 0.0, 0.45, 0.35, 0.0, 0.50, 0.50, 0.52, 0.30, 0.38, 0.30, 0.28, 0.20,
            0.18, 0.42, 0.55, 0.35, 0.25,
        ],
    },
    Scenario {
        name: "bow-bright",
        classification: "fair-bow-only",
        patch: [
            0.35, 0.80, 0.78, 0.0, 0.55, 0.78, 0.0, 0.35, 0.72, 0.78, 0.74, 0.82, 0.24, 0.62, 0.58,
            0.42, 0.70, 0.62, 0.72, 0.48,
        ],
    },
    Scenario {
        name: "bow-muted",
        classification: "fair-bow-only",
        patch: [
            0.72, 0.55, 0.42, 0.0, 0.25, 0.22, 0.0, 0.70, 0.24, 0.35, 0.18, 0.24, 0.78, 0.42, 0.12,
            0.08, 0.25, 0.82, 0.20, 0.10,
        ],
    },
    Scenario {
        name: "sample-blow",
        classification: "replacement-not-expected-to-match",
        patch: [
            0.50, 0.0, 0.50, 0.68, 0.58, 0.62, 0.0, 0.50, 0.50, 0.55, 0.48, 0.60, 0.42, 0.40, 0.32,
            0.24, 0.50, 0.58, 0.42, 0.30,
        ],
    },
    Scenario {
        name: "sample-strike",
        classification: "replacement-not-expected-to-match",
        patch: [
            0.34, 0.0, 0.50, 0.0, 0.50, 0.50, 0.76, 0.66, 0.74, 0.62, 0.56, 0.72, 0.36, 0.58, 0.26,
            0.20, 0.52, 0.54, 0.48, 0.24,
        ],
    },
];

fn select_model(name: &str) -> io::Result<usize> {
    match name {
        "modal" => Ok(0),
        "string" => Ok(1),
        "strings" => Ok(2),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "model must be modal, string, or strings",
        )),
    }
}

fn select_scenario(name: &str) -> io::Result<Scenario> {
    SCENARIOS
        .iter()
        .copied()
        .find(|scenario| scenario.name == name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "scenario must be bow-warm, bow-bright, bow-muted, sample-blow, or sample-strike",
            )
        })
}

fn render_channel(model: usize, patch: &[f32; 20], aux: bool) -> io::Result<Vec<f32>> {
    let mut inputs = [Inp::Val(0.0); MAX_PORTS];
    inputs[0] = Inp::Val(FREQUENCY_HZ);
    for (index, value) in patch.iter().copied().enumerate() {
        inputs[index + 1] = Inp::Val(value);
    }
    inputs[21] = Inp::Val(1.0); // ex-gate: held on throughout the fixed note.
    inputs[22] = Inp::Val(0.0); // ex-note offset: fixed at note 60 / C4.
    inputs[23] = Inp::Val(0.0); // ex-modulation.
    inputs[24] = Inp::Val(1.0); // ex-strength.
    inputs[25] = Inp::Val(model as f32); // ex-model.
    inputs[26] = Inp::Val(f32::from(aux)); // main (0) or auxiliary (1).
    inputs[27] = Inp::Val(0.0); // ex-alternate.
    inputs[28] = Inp::Val(0.0); // ex-external-blow.
    inputs[29] = Inp::Val(0.0); // ex-external-strike.

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
    let mut memory = vec![0.0; elements_internal::mem_len(SAMPLE_RATE)];
    let mut block = [0.0; BLOCK_FRAMES];
    let mut rendered = Vec::with_capacity(TOTAL_FRAMES);
    for _ in 0..TOTAL_FRAMES / BLOCK_FRAMES {
        elements_internal::render(&inputs, &mut state, &mut memory, &mut block, &kx);
        if block.iter().any(|sample| !sample.is_finite()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "elements kernel produced a nonfinite sample",
            ));
        }
        rendered.extend_from_slice(&block);
    }
    Ok(rendered)
}

fn emit(
    mut output: impl Write,
    model_name: &str,
    model: usize,
    scenario: Scenario,
) -> io::Result<()> {
    let main = render_channel(model, &scenario.patch, false)?;
    let aux = render_channel(model, &scenario.patch, true)?;
    write!(
        output,
        "{{\"type\":\"metadata\",\"family\":\"elements-model\",\"model\":\"{model_name}\",\"scenario\":\"{}\",\"classification\":\"{}\",\"sample_rate\":32000,\"block_frames\":16,\"note\":60,\"gate\":1,\"frames\":{},\"controls\":{{",
        scenario.name,
        scenario.classification,
        TOTAL_FRAMES
    )?;
    for (index, (name, value)) in CONTROL_NAMES.iter().zip(scenario.patch).enumerate() {
        if index > 0 {
            write!(output, ",")?;
        }
        write!(output, "\"{name}\":{value:.6}")?;
    }
    writeln!(output, "}}}}")?;
    for frame in 0..TOTAL_FRAMES {
        writeln!(
            output,
            "{{\"type\":\"sample\",\"frame\":{frame},\"main\":{:.9},\"aux\":{:.9}}}",
            main[frame], aux[frame]
        )?;
    }
    output.flush()
}

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let model_name = args
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: <model> <scenario>"))?;
    let scenario_name = args
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: <model> <scenario>"))?;
    if args.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected exactly a model and scenario",
        ));
    }
    let model = select_model(&model_name)?;
    let scenario = select_scenario(&scenario_name)?;
    emit(
        BufWriter::new(io::stdout().lock()),
        &model_name,
        model,
        scenario,
    )
}
