//! Domain entries of the native signature table (design 7.1.3): patterns,
//! controls, signals and sounds (design-music.md sections 1-3 and 7) and
//! visuals (design-visual.md section 2).
//!
//! Masks (design 5.5): a pattern subject or a sink argument is `Value`; a
//! numeric or control parameter of a domain constructor is `Late` (captured
//! deferred as a `PParam`/`VParam`, so `osc {* 20 {sin time}}` keeps its
//! thunk); a pattern transform argument (`every 4 {p -> ..}`) is `Fn`.
//! Domain parameters take a number, signal, pattern or fn of time
//! (design-visual section 1), so they are typed `any`.

use crate::dsp::build::DSP_KEYWORDS;
use crate::types::natives::{HostCap, NativeMask, NativeSig};

use NativeMask::{Fn as F, Late as L, Value as V};

/// The pattern controls of design-music.md section 2 ("sound parameters
/// available on any pattern", lines 150-153). `shape` is also the visual
/// source, so its entry is the overload group below.
pub(crate) const CONTROLS: [&str; 25] = [
    "gain",
    "pan",
    "speed",
    "lpf",
    "hpf",
    "resonance",
    "room",
    "size",
    "delay",
    "delaytime",
    "delayfeedback",
    "crush",
    "shape",
    "vowel",
    "legato",
    "attack",
    "release",
    "sustain",
    "begin",
    "end",
    "cut",
    "orbit",
    "velocity",
    "start-ms",
    "stop-ms",
];

const fn f(
    name: &'static str,
    min: u8,
    max: u8,
    ty: &'static [&'static str],
    mask: &'static [NativeMask],
) -> NativeSig {
    NativeSig::func(name, min, max, ty, mask)
}

/// A control step: `s :bd > gain 0.4` is `(gain (s :bd) 0.4)`.
const fn control(name: &'static str) -> NativeSig {
    f(name, 2, 2, CONTROL, &[V, L])
}

/// A unit generator or effect: any number of ugen inputs and the DSP
/// named parameters (12.8.6).
const fn dsp(name: &'static str) -> NativeSig {
    f(name, 0, 1, &["fn ugen -> ugen"], &[V])
        .rest()
        .kw(DSP_KEYWORDS)
}

/// The named arguments of `spectrum`: the analyzer unit's, plus `bins` of
/// the tap and buffer forms (14.5.9).
const SPECTRUM_KEYWORDS: [&str; DSP_KEYWORDS.len() + 1] = {
    let mut out = [""; DSP_KEYWORDS.len() + 1];
    let mut k = 0;
    while k < DSP_KEYWORDS.len() {
        out[k] = DSP_KEYWORDS[k];
        k += 1;
    }
    out[k] = "bins";
    out
};

const fn signal(name: &'static str) -> NativeSig {
    NativeSig::value(name, &["signal"])
}

const CONTROL: &[&str] = &["fn ctl any -> ctl"];
const PAT_1: &[&str] = &["fn (pattern 'a) -> pattern 'a"];
const PAT_P: &[&str] = &["fn (pattern 'a) any -> pattern 'a"];
const PAT_PP: &[&str] = &["fn (pattern 'a) any any -> pattern 'a"];
const PAT_F: &[&str] = &["fn (pattern 'a) (fn (pattern 'a) -> pattern 'a) -> pattern 'a"];
const PAT_PF: &[&str] = &["fn (pattern 'a) any (fn (pattern 'a) -> pattern 'a) -> pattern 'a"];
const PAT_PPF: &[&str] = &["fn (pattern 'a) any any (fn (pattern 'a) -> pattern 'a) -> pattern 'a"];
const STEP_P: &[&str] = &["fn 'a any -> pattern 'a"];
const STEPS: &[&str] = &["fn 'a -> pattern 'a"];
const LISTED: &[&str] = &["fn [pattern 'a] -> pattern 'a"];
const KIT: &[&str] = &["[keyword: sound]"];

const T1: &[&str] = &["fn tex any -> tex"];
const T2: &[&str] = &["fn tex any any -> tex"];
const T4: &[&str] = &["fn tex any any any any -> tex"];
const B0: &[&str] = &["fn tex tex -> tex"];
const B1: &[&str] = &["fn tex tex any -> tex"];
const B2: &[&str] = &["fn tex tex any any -> tex"];
const B4: &[&str] = &["fn tex tex any any any any -> tex"];

/// Every domain entry, in id order after the core entries.
pub(crate) static DOMAIN: &[NativeSig] = &[
    // Sounds and the sound kit (design-music section 2, 7.1.4).
    f("s", 1, 1, &["fn (pattern sound) -> ctl"], &[V]).kw(&["kit"]),
    f("sound", 1, 1, &["fn (pattern sound) -> ctl"], &[V]).kw(&["kit"]),
    f("sample", 1, 1, &["fn path -> sound"], &[V]),
    f("midi", 1, 1, &["fn int -> sound"], &[V]).needs(&[HostCap::MidiOut]),
    NativeSig::value("default-sound-kit", KIT),
    NativeSig::value("sound-kit", KIT),
    // The digital-drum-family kit alias dict (design-music 4.1, DDRUM-006).
    NativeSig::value("digital-kit", KIT),
    // Structure-giving steps after `s` (10.1 first-structure rule, 11.7).
    control("n"),
    control("note"),
    f("midi-notes", 1, 1, &["fn ctl -> ctl"], &[V])
        .kw(&["channel"])
        .needs(&[HostCap::MidiIn]),
    // Controls (section 2; `shape` is in the overload group).
    control("gain"),
    control("pan"),
    control("speed"),
    control("lpf"),
    control("hpf"),
    control("resonance"),
    control("room"),
    control("size"),
    control("delay"),
    control("delaytime"),
    control("delayfeedback"),
    control("crush"),
    control("vowel"),
    control("legato"),
    control("attack"),
    control("release"),
    control("sustain"),
    control("begin"),
    control("end"),
    control("start-ms"),
    control("stop-ms"),
    control("cut"),
    control("orbit"),
    control("velocity"),
    // An `inst` template header parameter compiled as a call head
    // (compiler.rs, design 12.8.6 M3, B2): the control name arrives as a
    // keyword argument. The name has a space, so it can never be written
    // in source; only the compiler emits a call to it.
    f(
        "inst control",
        3,
        3,
        &["fn ctl any keyword -> ctl"],
        &[V, L, V],
    ),
    // The fixed overload group (20 Q2, M1): number subject = visual source,
    // pattern subject = the `shape` control; pattern subject = scale notes,
    // texture subject = the visual transform.
    f(
        "shape",
        1,
        3,
        &["fn float any any -> tex", "fn ctl any -> ctl"],
        &[V, L, L],
    ),
    f(
        "scale",
        2,
        6,
        &[
            "fn (pattern 'a) any any -> pattern 'a",
            "fn tex any any any any any -> tex",
        ],
        &[V, L, L, L, L, L],
    ),
    // Pattern transforms (section 3 and the section 7 table).
    f("fast", 2, 2, PAT_P, &[V, L]),
    f("slow", 2, 2, PAT_P, &[V, L]),
    f("hurry", 2, 2, PAT_P, &[V, L]),
    f("rev", 1, 1, PAT_1, &[V]),
    f("every", 3, 3, PAT_PF, &[V, L, F]),
    f("whenmod", 4, 4, PAT_PPF, &[V, L, L, F]),
    f("sometimes", 2, 2, PAT_F, &[V, F]),
    f("rarely", 2, 2, PAT_F, &[V, F]),
    f("often", 2, 2, PAT_F, &[V, F]),
    f("sometimes-by", 3, 3, PAT_PF, &[V, L, F]),
    f("degrade-by", 2, 2, PAT_P, &[V, L]),
    f("superimpose", 2, 2, PAT_F, &[V, F]),
    f("off", 3, 3, PAT_PF, &[V, L, F]),
    f("jux", 2, 2, PAT_F, &[V, F]),
    f("iter", 2, 2, PAT_P, &[V, L]),
    f("ply", 2, 2, PAT_P, &[V, L]),
    f("chunk", 3, 3, PAT_PF, &[V, L, F]),
    f("chop", 2, 2, PAT_P, &[V, L]),
    f("striate", 2, 2, PAT_P, &[V, L]),
    f("slice", 3, 3, PAT_PP, &[V, L, L]),
    f("splice", 3, 3, PAT_PP, &[V, L, L]),
    f("loop-at", 2, 2, PAT_P, &[V, L]),
    f("fit", 1, 1, PAT_1, &[V]),
    f("chord", 2, 2, PAT_P, &[V, L]),
    f("voicing", 1, 1, PAT_1, &[V]),
    f("arp", 2, 2, PAT_P, &[V, L]),
    // Tidal `struct`, renamed (20 Q3); `struct` is never a pattern function.
    f("grid", 2, 2, PAT_P, &[V, L]),
    // Steps.
    f("alt", 1, 1, STEPS, &[V]).rest(),
    f("choose", 1, 1, STEPS, &[V]).rest(),
    f("maybe", 1, 2, STEP_P, &[V, L]),
    f("euclid", 3, 3, &["fn 'a any any -> pattern 'a"], &[V, L, L]).kw(&["rotation"]),
    f("hold", 2, 2, STEP_P, &[V, L]),
    f("stack", 1, 1, LISTED, &[V]),
    f("cat", 1, 1, LISTED, &[V]),
    f("fastcat", 1, 1, LISTED, &[V]),
    f(
        "segment",
        2,
        2,
        &["fn signal any -> pattern float"],
        &[V, L],
    ),
    // Subject first (M2): `range sine 200 2000`.
    f("range", 3, 3, &["fn 'a any any -> 'a"], &[V, L, L]),
    // Signals (section 3, the section 7 table).
    signal("sine"),
    signal("saw"),
    signal("tri"),
    signal("square"),
    signal("rand"),
    signal("perlin"),
    signal("time"),
    signal("beat"),
    signal("phase"),
    signal("cycle"),
    signal("amp").needs(&[HostCap::Analysis]),
    f("irand", 1, 1, &["fn any -> signal"], &[L]),
    f("fft", 1, 1, &["fn int -> signal"], &[V]).needs(&[HostCap::Analysis]),
    // Self-analysis (design 14.5.9, the 12.3 amendment): live taps of a bus
    // source and analyses of sample values. `scope`, `spectrum` and
    // `render` are in the M1 subject-overload group; `spectrum` over a ugen
    // is the analyzer unit (12.8.6).
    f(
        "scope",
        2,
        2,
        &["fn keyword int -> [float]", "fn sound int -> [float]"],
        &[V, V],
    ),
    f(
        "spectrum",
        0,
        1,
        &[
            "fn ugen -> ugen",
            "fn keyword -> [float]",
            "fn sound -> [float]",
        ],
        &[V],
    )
    .rest()
    .kw(&SPECTRUM_KEYWORDS),
    f("capture", 2, 2, &["fn keyword any -> sound"], &[V, V])
        .effect()
        .needs(&[HostCap::Analysis]),
    f("rms", 1, 1, &["fn sound -> float"], &[V]),
    f("peak", 1, 1, &["fn sound -> float"], &[V]),
    f("cc", 1, 1, &["fn int -> signal"], &[V])
        .kw(&["channel"])
        .needs(&[HostCap::MidiIn]),
    f("lag", 2, 2, &["fn signal any -> signal"], &[V, L]),
    f(
        "map-range",
        3,
        5,
        &["fn signal any any any any -> signal"],
        &[V, L, L, L, L],
    ),
    // Visual sources (design-visual section 1; Hydra defaults make every
    // parameter optional). `shape` is in the overload group above.
    f("osc", 0, 3, &["fn any any any -> tex"], &[L, L, L]),
    f("noise", 0, 2, &["fn any any -> tex"], &[L, L]),
    f("voronoi", 0, 3, &["fn any any any -> tex"], &[L, L, L]),
    f("gradient", 0, 1, &["fn any -> tex"], &[L]),
    f("solid", 0, 4, &["fn any any any any -> tex"], &[L, L, L, L]),
    f("src", 1, 1, &["fn keyword -> tex"], &[V]),
    f("text", 1, 1, &["fn string -> tex"], &[V]),
    // Geometry (`tile` is Hydra's `repeat`).
    f("rotate", 1, 3, T2, &[V, L, L]),
    f("pixelate", 1, 3, T2, &[V, L, L]),
    f("tile", 1, 5, T4, &[V, L, L, L, L]),
    f("tile-x", 1, 3, T2, &[V, L, L]),
    f("tile-y", 1, 3, T2, &[V, L, L]),
    f("kaleid", 1, 2, T1, &[V, L]),
    f("scroll", 1, 5, T4, &[V, L, L, L, L]),
    // Color.
    f("posterize", 1, 3, T2, &[V, L, L]),
    f("shift", 1, 5, T4, &[V, L, L, L, L]),
    f("invert", 1, 2, T1, &[V, L]),
    f("contrast", 1, 2, T1, &[V, L]),
    f("brightness", 1, 2, T1, &[V, L]),
    f("luma", 1, 3, T2, &[V, L, L]),
    f("thresh", 1, 3, T2, &[V, L, L]),
    f("color", 1, 5, T4, &[V, L, L, L, L]),
    f("saturate", 1, 2, T1, &[V, L]),
    f("hue", 1, 2, T1, &[V, L]),
    f("colorama", 1, 2, T1, &[V, L]),
    // Blend. `add`/`sub` are also pattern arithmetic (`add p 7`,
    // design-music section 3), so their subject is generic.
    f("add", 2, 3, &["fn 'a any any -> 'a"], &[V, V, L]),
    f("sub", 2, 3, &["fn 'a any any -> 'a"], &[V, V, L]),
    f("layer", 2, 2, B0, &[V, V]),
    f("blend", 2, 3, B1, &[V, V, L]),
    f("mult", 2, 3, B1, &[V, V, L]),
    f("diff", 2, 2, B0, &[V, V]),
    f("mask", 2, 2, B0, &[V, V]),
    // Modulate family.
    f("modulate", 2, 3, B1, &[V, V, L]),
    f("modulate-tile", 2, 6, B4, &[V, V, L, L, L, L]),
    f("modulate-kaleid", 2, 3, B1, &[V, V, L]),
    f("modulate-scroll", 2, 6, B4, &[V, V, L, L, L, L]),
    f("modulate-rotate", 2, 4, B2, &[V, V, L, L]),
    f("modulate-scale", 2, 4, B2, &[V, V, L, L]),
    f("modulate-pixelate", 2, 4, B2, &[V, V, L, L]),
    // Outputs and canvas.
    NativeSig::value("o0", &["keyword"]),
    NativeSig::value("o1", &["keyword"]),
    NativeSig::value("o2", &["keyword"]),
    NativeSig::value("o3", &["keyword"]),
    f("out", 1, 2, &["fn tex keyword -> tex"], &[V, V])
        .effect()
        .needs(&[HostCap::Render]),
    // A keyword or no subject: the visual setting; a number: offline render
    // (14.5.9).
    f(
        "render",
        0,
        1,
        &["fn keyword -> nil", "fn any -> sound"],
        &[V],
    )
    .effect()
    .needs(&[HostCap::Render]),
    f("use-fps", 1, 1, &["fn any -> nil"], &[V])
        .effect()
        .needs(&[HostCap::Render]),
    f("use-canvas", 2, 2, &["fn int int -> nil"], &[V, V])
        .effect()
        .needs(&[HostCap::Render]), // Unit generators and effects (design-music sections 2 and 4-6,
    // design 12.8.6). `saw`, `tri`, `lpf`, `hpf`, `delay`, `gain`, `pan`,
    // `room`, `saturate` and `range` are the entries above: the VM picks
    // their DSP meaning from the subject (B2).
    dsp("dsf-osc"),
    dsp("chebyshev-osc"),
    dsp("gaussian-noise"),
    dsp("lorenz-osc"),
    dsp("rossler-osc"),
    dsp("am-formant-osc"),
    dsp("sin-osc"),
    dsp("pulse"),
    dsp("white-noise"),
    dsp("host-in-l"),
    dsp("host-in-r"),
    dsp("bpf"),
    dsp("comb"),
    dsp("env-perc"),
    dsp("env-adsr"),
    dsp("line"),
    dsp("sample-play"),
    dsp("vco"),
    dsp("sub-osc"),
    dsp("ladder"),
    dsp("svf"),
    dsp("fm-op"),
    dsp("fm-mod"),
    dsp("fm-drum"),
    dsp("feedback-metal-core"),
    dsp("digital-drum-core"),
    dsp("digital-snare-core"),
    dsp("digital-metal-core"),
    dsp("digital-hat-core"),
    dsp("bass-core"),
    dsp("analog-percussion"),
    dsp("va-source"),
    dsp("va-filter"),
    dsp("phase-pair"),
    dsp("fm-pair"),
    dsp("six-op-original"),
    dsp("speech-original"),
    dsp("resonator-part-core"),
    dsp("string-choir-core"),
    dsp("exciter-core"),
    dsp("tidal-function-core"),
    dsp("tidal-poly-core"),
    dsp("peak-function-core"),
    dsp("stage-segment-core"),
    dsp("stage-chain-core"),
    dsp("stage-linked-core"),
    dsp("frame-lfo-core"),
    dsp("frame-keyframe-core"),
    dsp("peak-pulse-core"),
    dsp("number-station-core"),
    dsp("spectrum-pair"),
    dsp("clock-noise-pair"),
    dsp("dual-kick-core"),
    dsp("snare-pair-core"),
    dsp("hat-pair-core"),
    dsp("swarm-pair-core"),
    dsp("particle-pair-core"),
    dsp("modal-pair-core"),
    dsp("string-pair-core"),
    dsp("chip-pair-core"),
    dsp("analog-pair-core"),
    dsp("grain-pair-core"),
    dsp("shape-pair-core"),
    dsp("vactrol-gate"),
    dsp("decay-mod"),
    dsp("string-machine-core"),
    dsp("terrain-pair-core"),
    dsp("wave-grid-core"),
    dsp("chord-layer-core"),
    dsp("macro-five-core"),
    dsp("macro-sub-sync-core"),
    dsp("macro-triple-core"),
    dsp("macro-digital-core"),
    dsp("macro-filter-core"),
    dsp("macro-formant-core"),
    dsp("macro-fm-core"),
    dsp("macro-physical-core"),
    dsp("macro-struck-core"),
    dsp("macro-percussion-core"),
    dsp("macro-wave-grid-core"),
    dsp("macro-wave-line-core"),
    dsp("macro-noise-core"),
    dsp("macro-cloud-core"),
    dsp("aux-out"),
    dsp("out-3"),
    dsp("out-4"),
    dsp("feedback-drum"),
    dsp("noise-drum"),
    dsp("sine-drum"),
    dsp("phase-distortion"),
    dsp("additive"),
    dsp("wavetable"),
    dsp("granular"),
    dsp("svf-filter"),
    dsp("butterworth-filter"),
    dsp("chebyshev-filter"),
    dsp("ladder-filter"),
    dsp("allpass-filter"),
    dsp("parametric-resonator"),
    dsp("feedback-resonator"),
    dsp("foldback"),
    dsp("variable-clip"),
    dsp("alien-wah"),
    dsp("dynamic-convolution"),
    dsp("early-reflections"),
    dsp("schroeder-reverb"),
    dsp("spring-reverb"),
    dsp("space-reverb"),
    dsp("shimmer-reverb"),
    dsp("tape-delay"),
    dsp("diffusion-delay"),
    dsp("lofi"),
    dsp("compressor"),
    dsp("expander"),
    dsp("gate"),
    dsp("limiter"),
    dsp("multiband-compressor"),
    dsp("multiband-expander"),
    dsp("transient"),
    dsp("multiband-transient"),
    dsp("auto-level"),
    dsp("sag"),
    dsp("peq"),
    dsp("geq"),
    dsp("dynamic-eq"),
    dsp("tilt"),
    dsp("tone"),
    dsp("loudness-eq"),
    dsp("notch"),
    dsp("narrow"),
    dsp("linear-phase-eq"),
    dsp("group-delay-eq"),
    dsp("crossover"),
    dsp("ping-pong"),
    dsp("multitap"),
    dsp("time-align"),
    dsp("plate"),
    dsp("fdn"),
    dsp("convolution"),
    dsp("scatter"),
    dsp("tube"),
    dsp("clip"),
    dsp("harmonics"),
    dsp("exciter"),
    dsp("multiband-saturate"),
    dsp("sub-synth"),
    dsp("bandwidth-extend"),
    dsp("dynamic-saturate"),
    dsp("chorus"),
    dsp("flanger"),
    dsp("phaser"),
    dsp("tremolo"),
    dsp("auto-pan"),
    dsp("auto-filter"),
    dsp("pitch-shift"),
    dsp("pitch-shift-hq"),
    dsp("freq-shift"),
    dsp("rotary"),
    dsp("wow-flutter"),
    dsp("doppler"),
    dsp("vibrato"),
    dsp("bitcrush"),
    dsp("decimate"),
    dsp("jitter"),
    dsp("noise-blend"),
    dsp("hum"),
    dsp("tape"),
    dsp("cassette"),
    dsp("vinyl"),
    dsp("vinyl-artifacts"),
    dsp("codec"),
    dsp("radio"),
    dsp("tv-audio"),
    dsp("digital-error"),
    dsp("dsd-imd"),
    dsp("modal"),
    dsp("horn"),
    dsp("width"),
    dsp("balance"),
    dsp("multiband-balance"),
    dsp("ms"),
    dsp("crossfeed"),
    dsp("crosstalk-cancel"),
    dsp("phase-select-eq"),
    dsp("spatial-map"),
    dsp("matrix"),
    dsp("declick"),
    dsp("declip"),
    dsp("dehum"),
    dsp("denoise"),
    dsp("mute"),
    dsp("polarity"),
    dsp("dc-offset"),
    dsp("dry-wet"),
    dsp("section"),
    dsp("channel-divider"),
    dsp("dual-mod"),
    dsp("resonant-bank"),
    dsp("exciter-bank"),
    dsp("stream-envelope"),
    dsp("stream-vactr"),
    dsp("stream-follower"),
    dsp("stream-compressor"),
    dsp("stream-filter"),
    dsp("stream-lorenz"),
    dsp("shift-pair"),
    dsp("keyframe-mixer"),
    dsp("fir-crossover"),
    dsp("granulate"),
    dsp("texture-grain"),
    dsp("texture-stretch"),
    dsp("texture-loop"),
    dsp("texture-spectral"),
    dsp("level"),
    dsp("spectrogram"),
    dsp("note-spectrogram"),
    dsp("oscilloscope"),
    dsp("pitch-meter"),
    dsp("stereo-meter"),
    f("bus", 2, 2, &["fn ctl keyword -> ctl"], &[V, L]),
    f("master", 1, 1, &["fn any -> nil"], &[F]).effect(),
    NativeSig::value("lowpass", &["keyword"]),
    NativeSig::value("highpass", &["keyword"]),
    NativeSig::value("bandpass", &["keyword"]),
    // Append finite song APIs to preserve every existing native ID.
    f("part", 1, 1, &["fn [keyword: any] -> part"], &[V]).kw(&["duration"]),
    f("part-repeat", 2, 2, &["fn part int64 -> part"], &[V, V]).kw(&["seed-mode"]),
    f("sequence", 1, 1, &["fn [part] -> part"], &[V]),
    f(
        "replace-track",
        3,
        3,
        &["fn part keyword (pattern any) -> part"],
        &[V, V, V],
    ),
    f(
        "transform-instrument",
        4,
        4,
        &["fn part keyword sound (fn (pattern sound) -> ctl) -> part"],
        &[V, V, V, F],
    ),
    f(
        "part-events",
        4,
        4,
        &["fn part keyword ratio ratio -> [[keyword: any]]"],
        &[V, V, V, V],
    ),
    f(
        "delete-event",
        2,
        2,
        &["fn part event-handle -> part"],
        &[V, V],
    ),
    f(
        "overwrite-region",
        5,
        5,
        &["fn part keyword ratio ratio (pattern any) -> part"],
        &[V, V, V, V, V],
    ),
    f(
        "instrument-fx",
        4,
        4,
        &["fn part keyword sound keyword -> part"],
        &[V, V, V, V],
    ),
    f("song", 1, 1, &["fn part -> song"], &[V]).kw(&[
        "bpm",
        "cycle-beats",
        "meter",
        "seed",
        "tail-seconds",
    ]),
    f("play-song", 1, 1, &["fn song -> song"], &[V]).effect(),
];
