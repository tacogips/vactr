//! Port declarations for instrument voice kernels.

use super::{p, Port, FREQ, IN};

/// The Plaits voice-layer processor, in the stable kernel port order.
pub(super) const VACTROL_GATE: &[Port] = &[
    IN,
    FREQ,
    p("velocity", 1.0),
    p("lpg-mode", 0.0),
    p("lpg-decay", 0.5),
    p("lpg-color", 0.5),
    p("slot", 0.0),
    p("lane", 0.0),
    p("clocked", 0.0),
];

/// The trigger-patched Plaits decay modulation helper.
pub(super) const DECAY_MOD: &[Port] = &[p("lpg-decay", 0.5), p("amount", 0.0), p("target", 0.0)];

/// Original coupled-FM percussion; every sound parameter has its own port.
pub(super) const FEEDBACK_METAL: &[Port] = &[
    p("freq", 180.0),
    p("velocity", 1.0),
    p("metal-ratio", 2.4),
    p("metal-index", 3.0),
    p("metal-feedback", 0.35),
    p("metal-mod-decay", 0.16),
    p("metal-body-decay", 0.42),
    p("metal-pitch-drop", 0.5),
    p("metal-noise-level", 0.18),
    p("metal-noise-decay", 0.075),
    p("metal-noise-color", 5000.0),
    p("metal-noise-to-fm", 0.45),
    p("metal-cutoff", 1800.0),
    p("metal-resonance", 1.5),
    p("metal-drive", 0.22),
];

/// The tonal digital drum core: every shared drum-family control plus the
/// tonal additions, in the shared core contract's signal-chain order
/// (design-music 4.1, DDRUM-001/003). `cps` is a hidden implicit port: no
/// template ever wires it; `sched::commit` fills it from the event's tempo
/// for a voice that declares `lfo-sync`, and `onset-time` similarly anchors
/// `lfo-retrigger: false` phase to host time (see `digital_drum` module).
pub(super) const DIGITAL_DRUM: &[Port] = &[
    p("freq", 110.0),
    p("velocity", 1.0),
    p("wave", 4.0),
    p("coarse", 0.0),
    p("fine", 0.0),
    p("mod-wave", 4.0),
    p("mod-freq", 2.0),
    p("mod-level", 0.5),
    p("fm-depth", 1.0),
    p("pitch-decay", 0.05),
    p("pitch-depth", 12.0),
    p("pitch-slope", 0.5),
    p("osc-mix", 0.5),
    p("mod-decay", 0.15),
    p("mod-slope", 0.5),
    p("transient-wave", 0.0),
    p("transient-freq", 4000.0),
    p("transient-level", 0.3),
    p("filter-type", 0.0),
    p("cutoff", 1200.0),
    p("res", 0.3),
    p("filter-drive", 0.0),
    p("drive", 0.0),
    p("decimation", 0.0),
    p("amp-attack", 0.001),
    p("amp-decay", 0.3),
    p("amp-slope", 0.6),
    p("volume-velocity", 1.0),
    p("velocity-depth", 0.0),
    p("velocity-target", 0.0),
    p("lfo-wave", 0.0),
    p("lfo-rate", 4.0),
    p("lfo-depth", 0.0),
    p("lfo-offset", 0.0),
    p("lfo-target", 0.0),
    p("lfo-retrigger", 1.0),
    p("lfo-sync", 0.0),
    p("repeat-count", 0.0),
    p("repeat-time", 0.1),
    p("cps", 0.5),
    p("onset-time", 0.0),
];

/// The snare core: `DIGITAL_DRUM` plus the band-passed noise controls,
/// inserted after the FM section per the shared core contract's signal
/// chain (sources, then transient, then filter).
pub(super) const DIGITAL_SNARE: &[Port] = &[
    p("freq", 180.0),
    p("velocity", 1.0),
    p("wave", 4.0),
    p("coarse", 0.0),
    p("fine", 0.0),
    p("mod-wave", 4.0),
    p("mod-freq", 2.0),
    p("mod-level", 0.5),
    p("fm-depth", 1.0),
    p("pitch-decay", 0.05),
    p("pitch-depth", 12.0),
    p("pitch-slope", 0.5),
    p("osc-mix", 0.5),
    p("mod-decay", 0.15),
    p("mod-slope", 0.5),
    p("noise-freq", 2500.0),
    p("noise-mix", 0.3),
    p("transient-wave", 0.0),
    p("transient-freq", 4000.0),
    p("transient-level", 0.3),
    p("filter-type", 0.0),
    p("cutoff", 1200.0),
    p("res", 0.3),
    p("filter-drive", 0.0),
    p("drive", 0.0),
    p("decimation", 0.0),
    p("amp-attack", 0.001),
    p("amp-decay", 0.3),
    p("amp-slope", 0.6),
    p("volume-velocity", 1.0),
    p("velocity-depth", 0.0),
    p("velocity-target", 0.0),
    p("lfo-wave", 0.0),
    p("lfo-rate", 4.0),
    p("lfo-depth", 0.0),
    p("lfo-offset", 0.0),
    p("lfo-target", 0.0),
    p("lfo-retrigger", 1.0),
    p("lfo-sync", 0.0),
    p("repeat-count", 0.0),
    p("repeat-time", 0.1),
    p("cps", 0.5),
    p("onset-time", 0.0),
];

/// The metal/cymbal core: the shared surface plus the metal family
/// additions (DDRUM-001/004): `mod2-*` (the second FM modulator),
/// `mod-slope` and `metal-spread` (inharmonic-bank spread),
/// `repeat-count`/`repeat-time`. `cps`/`onset-time` are hidden implicit
/// ports (see `digital_drum` module doc comment).
pub(super) const DIGITAL_METAL: &[Port] = &[
    p("freq", 300.0),
    p("velocity", 1.0),
    p("wave", 4.0),
    p("coarse", 0.0),
    p("fine", 0.0),
    p("mod-wave", 4.0),
    p("mod-freq", 2.0),
    p("mod-level", 0.5),
    p("mod2-wave", 4.0),
    p("mod2-freq", 3.0),
    p("mod2-level", 0.3),
    p("fm-depth", 1.0),
    p("mod-decay", 0.15),
    p("mod-slope", 0.5),
    p("metal-spread", 0.3),
    p("transient-wave", 0.0),
    p("transient-freq", 4000.0),
    p("transient-level", 0.3),
    p("filter-type", 0.0),
    p("cutoff", 1200.0),
    p("res", 0.3),
    p("filter-drive", 0.0),
    p("drive", 0.0),
    p("decimation", 0.0),
    p("amp-attack", 0.001),
    p("amp-decay", 0.3),
    p("amp-slope", 0.6),
    p("volume-velocity", 1.0),
    p("velocity-depth", 0.0),
    p("velocity-target", 0.0),
    p("lfo-wave", 0.0),
    p("lfo-rate", 4.0),
    p("lfo-depth", 0.0),
    p("lfo-offset", 0.0),
    p("lfo-target", 0.0),
    p("lfo-retrigger", 1.0),
    p("lfo-sync", 0.0),
    p("repeat-count", 0.0),
    p("repeat-time", 0.1),
    p("cps", 0.5),
    p("onset-time", 0.0),
];

/// The hat core: the shared surface plus the hat family additions
/// (DDRUM-001/004): `mod2-*`, `open-decay`/`closed-decay`/`hat-open`. No
/// `mod-slope` (the modulation envelope is fixed fully exponential) and no
/// `repeat-count`/`repeat-time` (a hat voice does not self-repeat).
pub(super) const DIGITAL_HAT: &[Port] = &[
    p("freq", 400.0),
    p("velocity", 1.0),
    p("wave", 4.0),
    p("coarse", 0.0),
    p("fine", 0.0),
    p("mod-wave", 4.0),
    p("mod-freq", 2.0),
    p("mod-level", 0.5),
    p("mod2-wave", 4.0),
    p("mod2-freq", 3.0),
    p("mod2-level", 0.3),
    p("fm-depth", 1.0),
    p("mod-decay", 0.15),
    p("transient-wave", 0.0),
    p("transient-freq", 4000.0),
    p("transient-level", 0.3),
    p("filter-type", 0.0),
    p("cutoff", 1200.0),
    p("res", 0.3),
    p("filter-drive", 0.0),
    p("drive", 0.0),
    p("decimation", 0.0),
    p("amp-attack", 0.001),
    p("amp-decay", 0.3),
    p("amp-slope", 0.6),
    p("volume-velocity", 1.0),
    p("velocity-depth", 0.0),
    p("velocity-target", 0.0),
    p("lfo-wave", 0.0),
    p("lfo-rate", 4.0),
    p("lfo-depth", 0.0),
    p("lfo-offset", 0.0),
    p("lfo-target", 0.0),
    p("lfo-retrigger", 1.0),
    p("lfo-sync", 0.0),
    p("open-decay", 0.6),
    p("closed-decay", 0.08),
    p("hat-open", 0.0),
    p("cps", 0.5),
    p("onset-time", 0.0),
];

pub(super) const ANALOG_PERCUSSION: &[Port] = &[
    p("freq", 80.0),
    p("punch", 0.5),
    p("tone", 0.5),
    p("decay", 0.4),
    p("snappy", 0.5),
    p("metal", 0.5),
    p("mode", 0.0),
];
pub(super) const VA_SOURCE: &[Port] = &[FREQ, p("morph", 0.5)];
pub(super) const VA_FILTER: &[Port] = &[
    IN,
    FREQ,
    p("timbre", 0.5),
    p("filter-harmonics", 0.5),
    p("mode", 0.0),
];
pub(super) const PHASE_PAIR: &[Port] = &[
    FREQ,
    p("phase-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const FM_PAIR: &[Port] = &[
    FREQ,
    p("fm-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const SIX_OP_ORIGINAL: &[Port] = &[
    FREQ,
    p("six-patch", 0.0),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("six-sustain", 0.0),
    p("six-bank", 0.0),
];
pub(super) const SPEECH_ORIGINAL: &[Port] = &[
    FREQ,
    p("speech-harmonics", 0.0),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("speech-sustain", 0.0),
    p("mode", 0.0),
];
pub(super) const SPECTRUM_PAIR: &[Port] = &[
    FREQ,
    p("timbre", 0.5),
    p("morph", 0.5),
    p("spectrum-bumps", 0.5),
    p("mode", 0.0),
];
pub(super) const DUAL_KICK: &[Port] = &[
    FREQ,
    p("kick-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("mode", 0.0),
    p("kick-sustain", 0.0),
];
pub(super) const SNARE_PAIR: &[Port] = &[
    FREQ,
    p("snare-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("mode", 0.0),
    p("snare-sustain", 0.0),
];
pub(super) const HAT_PAIR: &[Port] = &[
    FREQ,
    p("hat-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("mode", 0.0),
    p("hat-sustain", 0.0),
];
pub(super) const SWARM_PAIR: &[Port] = &[
    FREQ,
    p("swarm-spread", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
    p("swarm-continuous", 0.0),
];
pub(super) const PARTICLE_PAIR: &[Port] = &[
    FREQ,
    p("particle-spread", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const MODAL_PAIR: &[Port] = &[
    FREQ,
    p("modal-structure", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("mode", 0.0),
    p("modal-sustain", 0.0),
];
pub(super) const STRING_PAIR: &[Port] = &[
    FREQ,
    p("string-structure", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("velocity", 1.0),
    p("mode", 0.0),
    p("string-sustain", 0.0),
];
pub(super) const CHIP_PAIR: &[Port] = &[
    FREQ,
    p("chip-chord", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
    p("chip-clocked", 0.0),
    p("chip-rate", 8.0),
];
pub(super) const ANALOG_PAIR: &[Port] = &[
    FREQ,
    p("analog-detune", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const CHORD_PAIR: &[Port] = &[
    FREQ,
    p("layer-chord", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const TABLE_TERRAIN_PAIR: &[Port] = &[
    FREQ,
    p("wave-bank", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const TERRAIN_PAIR: &[Port] = &[
    FREQ,
    p("terrain-select", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const STRING_MACHINE_PAIR: &[Port] = &[
    FREQ,
    p("machine-chord", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const SHAPE_PAIR: &[Port] = &[
    FREQ,
    p("shape-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const BRAIDS_FIVE: &[Port] = &[
    FREQ,
    p("macro-shape", 0.0),
    p("macro-color", 0.5),
    p("macro-timbre", 0.5),
    p("macro-strike", 0.0),
    p("macro-sync", 0.0),
];
pub(super) const GRAIN_PAIR: &[Port] = &[
    FREQ,
    p("grain-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];
pub(super) const CLOCK_NOISE_PAIR: &[Port] = &[
    FREQ,
    p("noise-harmonics", 0.5),
    p("timbre", 0.5),
    p("morph", 0.5),
    p("mode", 0.0),
];

/// Rings Part instrument roles plus independent main/aux selection.
pub(super) const RINGS_PART: &[Port] = &[
    FREQ,
    p("reso-model", 0.0),
    p("reso-structure", 0.5),
    p("reso-brightness", 0.5),
    p("reso-damping", 0.5),
    p("reso-position", 0.5),
    p("reso-strum", 1.0),
    p("reso-internal-exciter", 1.0),
    p("reso-internal-strum", 1.0),
    p("reso-internal-note", 1.0),
    p("reso-tonic", 0.0),
    p("reso-note", 0.0),
    p("reso-fm", 0.0),
    p("reso-chord", 0.0),
    p("reso-polyphony", 1.0),
    p("mode", 0.0),
    p("reso-external-in", 0.0),
];

/// Separate Rings StringSynthPart adaptation with six FX selections.
pub(super) const STRING_CHOIR: &[Port] = &[
    FREQ,
    p("choir-structure", 0.5),
    p("choir-brightness", 0.5),
    p("choir-damping", 0.5),
    p("choir-position", 0.5),
    p("choir-strum", 1.0),
    p("choir-internal-exciter", 1.0),
    p("choir-internal-strum", 1.0),
    p("choir-internal-note", 1.0),
    p("choir-tonic", 0.0),
    p("choir-note", 0.0),
    p("choir-fm", 0.0),
    p("choir-chord", 0.0),
    p("choir-polyphony", 1.0),
    p("choir-fx", 4.0),
    p("mode", 0.0),
];

/// Elements internal voice: one frequency, twenty Patch fields, four
/// performance fields, model selector and main/aux selector.
pub(super) const ELEMENTS_INTERNAL: &[Port] = &[
    FREQ,
    p("ex-env-shape", 0.5),
    p("ex-bow-level", 0.0),
    p("ex-bow-timbre", 0.5),
    p("ex-blow-level", 0.0),
    p("ex-blow-meta", 0.5),
    p("ex-blow-timbre", 0.5),
    p("ex-strike-level", 0.8),
    p("ex-strike-meta", 0.5),
    p("ex-strike-timbre", 0.5),
    p("ex-signature", 0.5),
    p("ex-geometry", 0.5),
    p("ex-brightness", 0.5),
    p("ex-damping", 0.5),
    p("ex-position", 0.5),
    p("ex-res-mod-frequency", 0.5),
    p("ex-res-mod-offset", 0.5),
    p("ex-reverb-diffusion", 0.5),
    p("ex-reverb-lp", 0.5),
    p("ex-space", 0.5),
    p("ex-modulation-frequency", 0.5),
    p("ex-gate", 1.0),
    p("ex-note", 0.0),
    p("ex-modulation", 0.0),
    p("ex-strength", 1.0),
    p("ex-model", 0.0),
    p("mode", 0.0),
    p("ex-alternate", 0.0),
    p("ex-external-blow", 0.0),
    p("ex-external-strike", 0.0),
];

/// First-generation Tides roles with sample, bipolar and event flag selectors.
pub(super) const TIDAL_FUNCTION: &[Port] = &[
    FREQ,
    p("tide-shape", 0.5),
    p("tide-slope", 0.5),
    p("tide-smoothness", 0.5),
    p("tide-ratio", 1.0),
    p("tide-sync", 0.0),
    p("tide-gate", 1.0),
    p("tide-clock", 0.0),
    p("tide-freeze", 0.0),
    p("tide-mode", 1.0),
    p("tide-range", 0.0),
    p("tide-pitch", 0.0),
    p("tide-output", 0.0),
];

/// Tides2 four authored channel roles; one selected lane per mono node.
pub(super) const TIDAL_POLY: &[Port] = &[
    FREQ,
    p("poly-width", 0.5),
    p("poly-shape", 0.5),
    p("poly-smoothness", 0.5),
    p("poly-shift", 0.5),
    p("poly-gate", 1.0),
    p("poly-clock", 0.0),
    p("poly-mode", 1.0),
    p("poly-output-mode", 2.0),
    p("poly-range", 1.0),
    p("poly-channel", 0.0),
];

/// Stages single-segment and oscillator controls; separate value/phase lanes.
pub(super) const STAGE_SEGMENT: &[Port] = &[
    FREQ,
    p("stage-type", 0.0),
    p("stage-primary", 0.5),
    p("stage-secondary", 0.5),
    p("stage-loop", 1.0),
    p("stage-gate", 1.0),
    p("stage-trigger", 0.0),
    p("stage-clock", 0.0),
    p("stage-function", 7.0),
    p("stage-channel", 0.0),
];

/// Six independently codeable Stages segment configurations, plus routing.
pub(super) const STAGE_CHAIN: &[Port] = &[
    FREQ,
    p("chain-count", 3.0),
    p("chain-gate", 1.0),
    p("chain-trigger", 0.0),
    p("chain-channel", 0.0),
    p("chain1-type", 0.0),
    p("chain1-loop", 0.0),
    p("chain1-primary", 0.2),
    p("chain1-secondary", 0.5),
    p("chain2-type", 2.0),
    p("chain2-loop", 1.0),
    p("chain2-primary", 0.7),
    p("chain2-secondary", 0.5),
    p("chain3-type", 0.0),
    p("chain3-loop", 0.0),
    p("chain3-primary", 0.2),
    p("chain3-secondary", 0.5),
    p("chain4-type", 0.0),
    p("chain4-loop", 0.0),
    p("chain4-primary", 0.2),
    p("chain4-secondary", 0.5),
    p("chain5-type", 0.0),
    p("chain5-loop", 0.0),
    p("chain5-primary", 0.2),
    p("chain5-secondary", 0.5),
    p("chain6-type", 0.0),
    p("chain6-loop", 0.0),
    p("chain6-primary", 0.2),
    p("chain6-secondary", 0.5),
];

pub(super) const FRAME_LFO: &[Port] = &[
    FREQ,
    p("frame-shape", 0.0),
    p("frame-spread", 0.75),
    p("frame-shape-spread", 0.5),
    p("frame-coupling", 0.5),
    p("frame-offset", 0.0),
    p("frame-channel", 0.0),
];

/// Peaks envelope, LFO and tap-LFO roles with full/half alternates.
pub(super) const PEAK_FUNCTION: &[Port] = &[
    FREQ,
    p("peak-attack", 0.2),
    p("peak-decay", 0.3),
    p("peak-sustain", 0.5),
    p("peak-release", 0.3),
    p("peak-rate", 0.5),
    p("peak-shape", 0.0),
    p("peak-color", 0.0),
    p("peak-reset-phase", 0.0),
    p("peak-level", 1.0),
    p("peak-half", 0.0),
    p("peak-gate", 1.0),
    p("peak-trigger", 0.0),
    p("peak-tap", 0.0),
    p("peak-sync", 0.0),
    p("peak-mode", 1.0),
    p("peak-channel", 0.0),
    p("peak-preset", 0.0),
];

/// Peaks pulse shaper/randomizer/bouncing-ball architectural roles.
pub(super) const PEAK_PULSE: &[Port] = &[
    FREQ,
    p("pulse-mode", 0.0),
    p("pulse-half", 0.0),
    p("pulse-gate", 0.0),
    p("pulse-trigger", 1.0),
    p("pulse-delay", 0.0),
    p("pulse-duration", 0.2),
    p("pulse-interval", 0.4),
    p("pulse-repeats", 2.0),
    p("pulse-accept", 1.0),
    p("pulse-repeat-prob", 0.4),
    p("pulse-randomness", 0.4),
    p("pulse-gravity", 0.5),
    p("pulse-loss", 0.6),
    p("pulse-amplitude", 0.9),
    p("pulse-velocity", 0.0),
    p("pulse-channel", 0.0),
];

/// Original procedural number/tone station; upstream digit asset is excluded.
pub(super) const NUMBER_STATION: &[Port] = &[
    FREQ,
    p("station-voice", 0.0),
    p("station-half", 0.0),
    p("station-tone", 0.5),
    p("station-transition", 0.5),
    p("station-noise", 0.5),
    p("station-drive", 0.5),
    p("station-digit", 0.0),
    p("station-auto", 0.0),
    p("station-gate", 1.0),
    p("station-trigger", 0.0),
    p("station-channel", 0.0),
];
