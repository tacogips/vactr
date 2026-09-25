//! G2 editor metadata (design 15.1.2 G2, command.md "editor-decl",
//! "manifest"): every `dsp::meta::all()` builtin, plus a fixed
//! pattern-function table so the editor, the LSP and the directives share
//! one source (13.5 "every builtin declares"). Ranges, curves and editor
//! kinds are never persisted.
//!
//! The pattern-function table's names are checked against
//! `design-docs/specs/design-music.md` section 4 ("patterns (Tidal/Strudel
//! names)" and "signals"): `euclid`, `maybe`, `degrade-by`, `hold`, `fast`,
//! `slow`, `sine`, `saw`, `tri`, `square`, `rand` and `perlin` are all
//! listed there, so none is omitted. `saw` and `tri` are ALSO ugen names
//! (`dsp::meta::all()` already declares an oscillator `EditorDecl` for
//! each); the pattern-function entries below are appended anyway, since
//! G2 asks for the fixed table verbatim, so `editor_decls()` carries two
//! entries named `saw` and two named `tri` (one `filter-response`/
//! `envelope-shape`-style ugen entry from the catalog, one `lfo-shape`
//! signal entry) — a client keys `manifest.editors` by `(name, kind)`,
//! not by `name` alone.

use crate::dsp::meta::{self, Curve, EditorDecl, EditorKind, ParamMeta, Unit};
use crate::session::protocol::{WireEditorDecl, WireParamMeta};

fn kind_name(k: EditorKind) -> &'static str {
    match k {
        EditorKind::EqCurve => "eq-curve",
        EditorKind::FilterResponse => "filter-response",
        EditorKind::DynamicsTransfer { .. } => "dynamics-transfer",
        EditorKind::EnvelopeShape => "envelope-shape",
        EditorKind::DelayTaps => "delay-taps",
        EditorKind::ReverbRoom => "reverb-room",
        EditorKind::SamplerWave => "sampler-wave",
        EditorKind::WavetableFrames => "wavetable-frames",
        EditorKind::GranularRegion => "granular-region",
        EditorKind::LfoShape => "lfo-shape",
        EditorKind::StereoField => "stereo-field",
        EditorKind::XyPad => "xy-pad",
        EditorKind::EuclidRing => "euclid-ring",
        EditorKind::ProbabilityDial => "probability-dial",
        EditorKind::LengthHandle => "length-handle",
        EditorKind::Scalar => "scalar",
    }
}

fn curve_name(c: Curve) -> &'static str {
    match c {
        Curve::Linear => "linear",
        Curve::Log => "log",
        Curve::Stepped => "stepped",
    }
}

fn unit_name(u: Unit) -> &'static str {
    match u {
        Unit::None => "none",
        Unit::Db => "db",
        Unit::Seconds => "s",
        Unit::Millis => "ms",
        Unit::Hz => "hz",
        Unit::Semitones => "st",
    }
}

fn param_wire(p: &ParamMeta) -> WireParamMeta {
    WireParamMeta {
        name: p.name.to_string(),
        ctl: Some(p.ctl.get()),
        range: [p.range.0, p.range.1],
        curve: curve_name(p.curve).to_string(),
        unit: unit_name(p.unit).to_string(),
        group: u32::from(p.group),
    }
}

fn decl_wire(d: &EditorDecl) -> WireEditorDecl {
    WireEditorDecl {
        name: d.name.to_string(),
        kind: kind_name(d.kind).to_string(),
        multiband: matches!(d.kind, EditorKind::DynamicsTransfer { multiband: true })
            .then_some(true),
        params: d.params.iter().map(param_wire).collect(),
    }
}

/// A pattern-function parameter: no `ctl` (no control-table row).
fn pattern_param(name: &'static str, range: [f32; 2]) -> WireParamMeta {
    WireParamMeta {
        name: name.to_string(),
        ctl: None,
        range,
        curve: "linear".to_string(),
        unit: "none".to_string(),
        group: 0,
    }
}

fn pattern_decl(
    name: &'static str,
    kind: &'static str,
    params: Vec<WireParamMeta>,
) -> WireEditorDecl {
    WireEditorDecl {
        name: name.to_string(),
        kind: kind.to_string(),
        multiband: None,
        params,
    }
}

/// Every builtin's `EditorDecl` (`dsp::meta::all()`) plus the fixed
/// pattern-function table of design 15.1.2 G2: `euclid` -> `euclid-ring`
/// (`hits`, `steps`, `rotation`); `maybe` and `degrade-by` ->
/// `probability-dial` (`probability`, range `0..1`); `hold`, `fast` and
/// `slow` -> `length-handle` (`factor`); `sine`, `saw`, `tri`, `square`,
/// `rand` and `perlin` -> `lfo-shape` (no params).
#[must_use]
pub fn editor_decls() -> Vec<WireEditorDecl> {
    let mut out: Vec<WireEditorDecl> = meta::all().iter().map(decl_wire).collect();
    out.push(pattern_decl(
        "euclid",
        "euclid-ring",
        vec![
            pattern_param("hits", [0.0, 16.0]),
            pattern_param("steps", [1.0, 16.0]),
            pattern_param("rotation", [0.0, 16.0]),
        ],
    ));
    for name in ["maybe", "degrade-by"] {
        out.push(pattern_decl(
            name,
            "probability-dial",
            vec![pattern_param("probability", [0.0, 1.0])],
        ));
    }
    for name in ["hold", "fast", "slow"] {
        out.push(pattern_decl(
            name,
            "length-handle",
            vec![pattern_param("factor", [0.0, 8.0])],
        ));
    }
    for name in ["sine", "saw", "tri", "square", "rand", "perlin"] {
        out.push(pattern_decl(name, "lfo-shape", Vec::new()));
    }
    out
}
