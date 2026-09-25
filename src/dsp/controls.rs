//! The control table (design 12.8.7).
//!
//! One row per control name: its wire id, default, editor range, where the
//! scheduler routes it and which values it takes. `Ctl` carries only `f32`,
//! so `encode` turns a keyword into its index in the row's enum list and a
//! bool into 0 or 1; a bank, table or bus keyword is a resource the caller
//! encodes as its installed id.
//!
//! `gain` shares `amp`'s id: it is the pattern name of the instrument's
//! `amp` (Q4). `note` and `n` keep their own ids; commit maps them to
//! `freq` through the scale (and `n` on a bank selects the sample). A
//! control the playing instrument declares as a parameter goes to it; the
//! row's route is where a control no instrument declares goes, and a
//! control nobody takes is ignored with no diagnostic.

use crate::sched::slots::CtlId;
use crate::value::intern::name_of_kw;
use crate::value::value::Value;
use crate::vm::call::kind_name;
use crate::vm::fail::{FailCode, Failure};

/// Where a control goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CtlRoute {
    /// An instrument parameter, by name.
    InstParam,
    /// A parameter of the slot's orbit effect unit.
    OrbitFx { unit: u8, param: u8 },
    /// A parameter of the bus reverb unit (`room`, `size`).
    BusUnit { param: u8 },
    /// Read by the scheduler, never sent to the audio side.
    Scheduler,
}

/// Which values a control takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CtlDomain {
    Float,
    Bool,
    /// A keyword from this list, encoded as its index.
    Enum(&'static [&'static str]),
    /// A bank, table, source or bus keyword, encoded by the caller.
    Resource,
}

/// One control.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ControlRow {
    pub name: &'static str,
    pub ctl: CtlId,
    pub default: f32,
    pub range: (f32, f32),
    pub route: CtlRoute,
    pub domain: CtlDomain,
}

/// The orbit effect units.
pub mod orbit {
    pub const FILTER: u8 = 0;
    pub const DELAY: u8 = 1;
    pub const DISTORT: u8 = 2;
    pub const VOWEL: u8 = 3;
}

const WAVES: &[&str] = &["saw", "pulse", "square", "tri", "sine"];
const ENVELOPES: &[&str] = &["hann", "tri", "trapezoid", "expo"];
const VOWELS: &[&str] = &["a", "e", "i", "o", "u"];

const fn mk(
    name: &'static str,
    id: u16,
    default: f32,
    range: (f32, f32),
    route: CtlRoute,
    domain: CtlDomain,
) -> ControlRow {
    ControlRow {
        name,
        ctl: CtlId::new(id),
        default,
        range,
        route,
        domain,
    }
}

const fn param(name: &'static str, id: u16, default: f32, range: (f32, f32)) -> ControlRow {
    mk(
        name,
        id,
        default,
        range,
        CtlRoute::InstParam,
        CtlDomain::Float,
    )
}

const fn fx(
    name: &'static str,
    id: u16,
    default: f32,
    range: (f32, f32),
    unit: u8,
    p: u8,
) -> ControlRow {
    mk(
        name,
        id,
        default,
        range,
        CtlRoute::OrbitFx { unit, param: p },
        CtlDomain::Float,
    )
}

const fn sched(name: &'static str, id: u16, domain: CtlDomain) -> ControlRow {
    mk(name, id, 0.0, (0.0, 0.0), CtlRoute::Scheduler, domain)
}

const AMP: u16 = 1;

/// Every control. Ids are dense and unique except `gain`, which is `amp`.
pub static ROWS: &[ControlRow] = &[
    param("freq", 0, 440.0, (20.0, 20_000.0)),
    param("amp", AMP, 0.5, (0.0, 1.0)),
    param("gain", AMP, 1.0, (0.0, 2.0)),
    param("note", 2, 0.0, (-60.0, 127.0)),
    param("n", 3, 0.0, (0.0, 127.0)),
    param("pan", 4, 0.5, (0.0, 1.0)),
    param("speed", 5, 1.0, (-4.0, 4.0)),
    param("velocity", 6, 1.0, (0.0, 1.0)),
    param("begin", 7, 0.0, (0.0, 1.0)),
    param("end", 8, 1.0, (0.0, 1.0)),
    param("attack", 9, 0.01, (0.0, 10.0)),
    param("decay", 10, 0.1, (0.0, 10.0)),
    param("sustain", 11, 1.0, (0.0, 1.0)),
    param("release", 12, 0.1, (0.0, 10.0)),
    param("cutoff", 13, 1200.0, (20.0, 20_000.0)),
    param("res", 14, 0.3, (0.0, 1.0)),
    mk(
        "wave",
        15,
        0.0,
        (0.0, 4.0),
        CtlRoute::InstParam,
        CtlDomain::Enum(WAVES),
    ),
    param("unison", 16, 1.0, (1.0, 16.0)),
    param("detune", 17, 0.1, (0.0, 1.0)),
    param("drift", 18, 0.002, (0.0, 0.1)),
    param("ratio", 19, 1.0, (0.0, 32.0)),
    param("index", 20, 1.0, (0.0, 32.0)),
    param("algorithm", 21, 1.0, (1.0, 32.0)),
    param("position", 22, 0.0, (0.0, 1.0)),
    mk(
        "table",
        23,
        0.0,
        (0.0, 0.0),
        CtlRoute::InstParam,
        CtlDomain::Resource,
    ),
    mk(
        "bank",
        24,
        0.0,
        (0.0, 0.0),
        CtlRoute::InstParam,
        CtlDomain::Resource,
    ),
    mk(
        "loop",
        25,
        0.0,
        (0.0, 1.0),
        CtlRoute::InstParam,
        CtlDomain::Bool,
    ),
    param("density", 26, 24.0, (0.0, 1000.0)),
    param("spray", 27, 0.0, (0.0, 1.0)),
    param("pitch", 28, 0.0, (-24.0, 24.0)),
    param("pitch-spray", 29, 0.0, (0.0, 1.0)),
    mk(
        "envelope",
        30,
        0.0,
        (0.0, 3.0),
        CtlRoute::InstParam,
        CtlDomain::Enum(ENVELOPES),
    ),
    param("reverse", 31, 0.0, (0.0, 1.0)),
    mk(
        "freeze",
        32,
        0.0,
        (0.0, 1.0),
        CtlRoute::InstParam,
        CtlDomain::Bool,
    ),
    param("stereo-spray", 33, 0.0, (0.0, 1.0)),
    mk(
        "source",
        34,
        0.0,
        (0.0, 0.0),
        CtlRoute::InstParam,
        CtlDomain::Resource,
    ),
    fx("lpf", 35, 20_000.0, (20.0, 20_000.0), orbit::FILTER, 0),
    fx("hpf", 36, 20.0, (20.0, 20_000.0), orbit::FILTER, 1),
    fx("resonance", 37, 0.0, (0.0, 1.0), orbit::FILTER, 2),
    fx("delay", 38, 0.0, (0.0, 1.0), orbit::DELAY, 0),
    fx("delaytime", 39, 0.25, (0.0, 4.0), orbit::DELAY, 1),
    fx("delayfeedback", 40, 0.5, (0.0, 0.99), orbit::DELAY, 2),
    fx("crush", 41, 16.0, (1.0, 16.0), orbit::DISTORT, 0),
    fx("shape", 42, 0.0, (0.0, 1.0), orbit::DISTORT, 1),
    mk(
        "vowel",
        43,
        0.0,
        (0.0, 4.0),
        CtlRoute::OrbitFx {
            unit: orbit::VOWEL,
            param: 0,
        },
        CtlDomain::Enum(VOWELS),
    ),
    mk(
        "room",
        44,
        0.0,
        (0.0, 1.0),
        CtlRoute::BusUnit { param: 0 },
        CtlDomain::Float,
    ),
    mk(
        "size",
        45,
        0.5,
        (0.0, 1.0),
        CtlRoute::BusUnit { param: 1 },
        CtlDomain::Float,
    ),
    sched("orbit", 46, CtlDomain::Float),
    sched("bus", 47, CtlDomain::Resource),
    sched("cut", 48, CtlDomain::Float),
    sched("legato", 49, CtlDomain::Float),
];

/// The row of a control name.
#[must_use]
pub fn row(name: &str) -> Option<&'static ControlRow> {
    ROWS.iter().find(|r| r.name == name)
}

/// The row of a wire id (`amp` for `gain`'s shared id).
#[must_use]
pub fn row_by_id(ctl: CtlId) -> Option<&'static ControlRow> {
    ROWS.iter().find(|r| r.ctl == ctl)
}

/// Encodes a control value as the `f32` a `Ctl::Const` carries.
///
/// # Errors
/// `Failure(type)` for a value outside the row's domain (an unknown
/// keyword, a non-number for a float, a non-bool for a bool) and for a
/// resource control, which the caller encodes.
pub fn encode(row: &ControlRow, value: &Value) -> Result<f32, Failure> {
    let bad = |what: &str| {
        Failure::new(
            FailCode::Type,
            format!(
                "control `{}` expects {what}, got {}",
                row.name,
                kind_name(value)
            ),
        )
    };
    match row.domain {
        CtlDomain::Float => number(value).ok_or_else(|| bad("a number")),
        CtlDomain::Bool => match value {
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            _ => Err(bad("a bool")),
        },
        CtlDomain::Enum(names) => match value {
            Value::Keyword(k) => {
                let name = name_of_kw(*k);
                names
                    .iter()
                    .position(|n| *n == &*name)
                    .map(|i| i as f32)
                    .ok_or_else(|| {
                        Failure::new(
                            FailCode::Type,
                            format!(
                                "control `{}` expects one of :{}, got :{name}",
                                row.name,
                                names.join(" :")
                            ),
                        )
                    })
            }
            _ => Err(bad("a keyword")),
        },
        CtlDomain::Resource => Err(Failure::new(
            FailCode::Type,
            format!("control `{}` names a resource the caller encodes", row.name),
        )),
    }
}

/// A number as `f32`.
fn number(value: &Value) -> Option<f32> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    match value {
        Value::Int(i) => Some(*i as f32),
        Value::Int64(i) => Some(*i as f32),
        Value::Float(x) => Some(*x),
        Value::Float64(x) => Some(*x as f32),
        Value::Ratio(r) => Some(r.to_f64() as f32),
        _ => None,
    }
}
