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

use crate::dsp::controls::{self, CtlDomain, DeclaredParam};
use crate::dsp::meta::{self, Curve, EditorDecl, EditorKind, ParamMeta, Unit};
use crate::host::wire::Ctl;
use crate::ns::insts::{InstEntry, InstRegistry};
use crate::session::protocol::{WireEditorDecl, WireParamMeta};
use crate::value::intern::name_of_kw;

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
        Unit::Metres => "m",
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
        default: Some(p.default),
        label: Some(p.label.clone()),
        choices: p.choices.iter().map(|s| (*s).to_string()).collect(),
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
fn pattern_param(name: &'static str, range: [f32; 2], default: f32) -> WireParamMeta {
    WireParamMeta {
        name: name.to_string(),
        ctl: None,
        range,
        curve: "linear".to_string(),
        unit: "none".to_string(),
        group: 0,
        default: Some(default),
        label: Some(meta::label_of(name)),
        choices: Vec::new(),
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
            pattern_param("hits", [0.0, 16.0], 3.0),
            pattern_param("steps", [1.0, 16.0], 8.0),
            pattern_param("rotation", [0.0, 16.0], 0.0),
        ],
    ));
    for name in ["maybe", "degrade-by"] {
        out.push(pattern_decl(
            name,
            "probability-dial",
            vec![pattern_param("probability", [0.0, 1.0], 0.5)],
        ));
    }
    for name in ["hold", "fast", "slow"] {
        out.push(pattern_decl(
            name,
            "length-handle",
            vec![pattern_param("factor", [0.0, 8.0], 2.0)],
        ));
    }
    for name in ["sine", "saw", "tri", "square", "rand", "perlin"] {
        out.push(pattern_decl(name, "lfo-shape", Vec::new()));
    }
    out
}

/// The default of a declared header parameter, per the live registry: the
/// instrument's own `Ctl::Const`, the current value of its `Ctl::Cell`
/// tweak site, or (with neither) the control row's default (DDRUM-006: a
/// user `inst` header's default must be ITS default, not the shared row's).
fn instance_default(entry: &InstEntry, param: &DeclaredParam, fallback: f32) -> f32 {
    entry
        .def
        .params
        .iter()
        .find_map(|(ctl, v)| (*ctl == param.ctl).then_some(*v))
        .map_or(fallback, |v| match v {
            Ctl::Const(c) => c,
            Ctl::Cell(cell) => entry.default_cell_value(cell).unwrap_or(fallback),
        })
}

/// One header parameter's wire metadata: full metadata (range, default,
/// label, enum choices) when its name is a control-table row, else the
/// declared bounds only (DDRUM-006).
fn declared_param_wire(entry: &InstEntry, param: &DeclaredParam) -> WireParamMeta {
    let pname = name_of_kw(param.name);
    let Some(row) = controls::row(&pname) else {
        return WireParamMeta {
            name: pname.to_string(),
            ctl: Some(param.ctl.get()),
            range: param
                .range
                .map_or([f32::MIN, f32::MAX], |(lo, hi)| [lo, hi]),
            curve: "linear".to_string(),
            unit: "none".to_string(),
            group: 0,
            default: None,
            label: Some(meta::label_of(&pname)),
            choices: Vec::new(),
        };
    };
    let choices: Vec<String> = match row.domain {
        CtlDomain::Enum(names) => names.iter().map(|s| (*s).to_string()).collect(),
        _ => Vec::new(),
    };
    let curve = if matches!(row.domain, CtlDomain::Float) {
        "linear"
    } else {
        "stepped"
    };
    WireParamMeta {
        name: pname.to_string(),
        ctl: Some(param.ctl.get()),
        range: [row.range.0, row.range.1],
        curve: curve.to_string(),
        unit: "none".to_string(),
        group: 0,
        default: Some(instance_default(entry, param, row.default)),
        label: Some(meta::label_of(&pname)),
        choices,
    }
}

/// Editor declarations for installed user instruments. Their header schema
/// is the same schema the scheduler uses to encode event controls.
#[must_use]
pub fn instrument_decls(registry: &InstRegistry) -> Vec<WireEditorDecl> {
    registry
        .entries()
        .filter(|entry| {
            meta::decl_for(&name_of_kw(entry.name)).is_none()
                || entry
                    .params
                    .iter()
                    .any(|param| crate::dsp::controls::row(&name_of_kw(param.name)).is_none())
        })
        .map(|entry| {
            let name = name_of_kw(entry.name).to_string();
            let mut params: Vec<WireParamMeta> = entry
                .params
                .iter()
                .map(|param| declared_param_wire(entry, param))
                .collect();
            if name == "stage-linked-voice" {
                params.push(WireParamMeta {
                    name: "segments".to_string(),
                    ctl: None,
                    range: [1.0, 36.0],
                    curve: "immutable-list".to_string(),
                    unit: "stride4".to_string(),
                    group: 1,
                    default: None,
                    label: Some("Segments".to_string()),
                    choices: Vec::new(),
                });
            }
            WireEditorDecl {
                name,
                kind: "scalar".to_string(),
                multiband: None,
                params,
            }
        })
        .collect()
}
