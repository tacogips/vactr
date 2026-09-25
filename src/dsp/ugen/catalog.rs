//! The ugen catalog: names and input ports (design 12.1, design-music 4, 7).
//!
//! Port `i` of a node is fed by an `Edge { port: i }`, by a node parameter
//! whose control id names the port (`port_ctl`), or else by its default: a
//! port named after a control-table row reads the voice's value of that
//! control (the implicit control names of 12.8.6, B2); any other port reads
//! its constant default. Effect nodes take the audio input on port 0 and
//! the kind's parameters, in catalog order, on ports 1.. .

use crate::dsp::arena::FaultCode;
use crate::dsp::cells::CellId;
use crate::dsp::controls;
use crate::dsp::effects::{self, EFFECT_PARAM_BASE};
use crate::dsp::graph::{BankRef, EffectKind, GranSrc, TableRef, UGenSpec};
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;

use super::{BuildError, Node, RawGraph};

/// The most input ports a node has (`granular` has ten).
pub const MAX_PORTS: usize = 10;

/// One input port: its name and constant default.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Port {
    pub name: &'static str,
    pub default: f32,
}

const fn p(name: &'static str, default: f32) -> Port {
    Port { name, default }
}

const FREQ: Port = p("freq", 440.0);
const IN: Port = p("in", 0.0);

const OSC: &[Port] = &[FREQ];
const PULSE: &[Port] = &[FREQ, p("width", 0.5)];
const FILTER: &[Port] = &[IN, p("cutoff", 1200.0), p("res", 0.3)];
const SVF: &[Port] = &[IN, p("cutoff", 1200.0), p("res", 0.3), p("mode", 0.0)];
const DELAY: &[Port] = &[IN, p("time", 0.25), p("feedback", 0.0)];
const COMB: &[Port] = &[IN, p("time", 0.01), p("feedback", 0.5)];
const PERC: &[Port] = &[p("attack", 0.01), p("release", 0.3)];
const ADSR: &[Port] = &[
    p("attack", 0.01),
    p("decay", 0.1),
    p("sustain", 1.0),
    p("release", 0.1),
];
const LINE: &[Port] = &[p("from", 1.0), p("to", 0.0), p("dur", 1.0)];
/// `sample-play` reads exactly these controls, plus the event's `bank`
/// (TASK-008 criterion 3: no region machinery beyond begin/end/speed/loop).
pub const SAMPLE: &[Port] = &[
    p("speed", 1.0),
    p("begin", 0.0),
    p("end", 1.0),
    p("loop", 0.0),
];
const MUL: &[Port] = &[p("a", 1.0), p("b", 1.0)];
const ADD: &[Port] = &[p("a", 0.0), p("b", 0.0)];
const VCO: &[Port] = &[
    FREQ,
    p("wave", 0.0),
    p("unison", 1.0),
    p("detune", 0.1),
    p("drift", 0.002),
    p("width", 0.5),
];
const FM_OP: &[Port] = &[FREQ, p("ratio", 1.0), p("index", 1.0), p("mod", 0.0)];
const FM_MOD: &[Port] = &[IN, p("mod", 0.0), p("index", 1.0)];
const PD: &[Port] = &[FREQ, p("shape", 0.5)];
const ADDITIVE: &[Port] = &[FREQ, p("count", 8.0), p("tilt", 1.0)];
const WAVETABLE: &[Port] = &[FREQ, p("position", 0.0)];
/// The granular controls (design-music 6), in port order.
pub const GRANULAR: &[Port] = &[
    p("position", 0.0),
    p("density", 24.0),
    p("size", 0.08),
    p("spray", 0.0),
    p("pitch", 0.0),
    p("pitch-spray", 0.0),
    p("envelope", 0.0),
    p("reverse", 0.0),
    p("freeze", 0.0),
    p("stereo-spray", 0.0),
];
const NONE: &[Port] = &[];

/// The ports of a node kind (effects: see `effect_port`).
#[must_use]
pub fn ports(node: &Node) -> &'static [Port] {
    match node {
        Node::SinOsc | Node::Saw | Node::Tri | Node::SubOsc => OSC,
        Node::Pulse => PULSE,
        Node::WhiteNoise | Node::Const(_) | Node::Param(_) | Node::Effect { .. } => NONE,
        Node::Lpf | Node::Hpf | Node::Bpf | Node::Ladder => FILTER,
        Node::Svf => SVF,
        Node::Delay => DELAY,
        Node::Comb => COMB,
        Node::EnvPerc => PERC,
        Node::EnvAdsr => ADSR,
        Node::Line => LINE,
        Node::SamplePlay(_) => SAMPLE,
        Node::Mul => MUL,
        Node::Add => ADD,
        Node::Vco { .. } => VCO,
        Node::FmOp => FM_OP,
        Node::FmMod => FM_MOD,
        Node::PhaseDistortion => PD,
        Node::Additive { .. } => ADDITIVE,
        Node::Wavetable(_) => WAVETABLE,
        Node::Granular(_) => GRANULAR,
    }
}

/// The number of ports of a node, effects included.
#[must_use]
pub fn port_count(node: &Node) -> usize {
    match node {
        Node::Effect { kind, .. } => (1 + effects::params(*kind).len()).min(MAX_PORTS),
        _ => ports(node).len(),
    }
}

/// The constant default of port `i` (effects: 0 for the input, the
/// parameter default otherwise).
#[must_use]
pub fn port_default(node: &Node, i: usize) -> f32 {
    match node {
        Node::Effect { kind, .. } => match i {
            0 => 0.0,
            _ => effects::params(*kind).get(i - 1).map_or(0.0, |d| d.default),
        },
        _ => ports(node).get(i).map_or(0.0, |p| p.default),
    }
}

/// The control id that addresses port `i` as a node parameter: the
/// control-table row of the port's name, else `EFFECT_PARAM_BASE + i`.
/// (Effect parameters use their effect-local ids, `effects::param_ctl`.)
#[must_use]
pub fn port_ctl(node: &Node, i: usize) -> Option<CtlId> {
    let port = ports(node).get(i)?;
    Some(match controls::row(port.name) {
        Some(row) => row.ctl,
        None => CtlId::new(EFFECT_PARAM_BASE + u16::try_from(i).ok()?),
    })
}

/// The port a node parameter id addresses.
#[must_use]
pub fn port_of(node: &Node, ctl: CtlId) -> Option<usize> {
    (0..ports(node).len()).find(|&i| port_ctl(node, i) == Some(ctl))
}

/// The voice control a port reads when nothing feeds it (a port named
/// after an instrument-parameter row of the control table), else `None`.
#[must_use]
pub fn implicit_ctl(node: &Node, i: usize) -> Option<CtlId> {
    let port = ports(node).get(i)?;
    controls::row(port.name)
        .filter(|r| r.route == controls::CtlRoute::InstParam)
        .map(|r| r.ctl)
}

/// The catalog name of a ugen (design-music section 7, kebab-case).
#[must_use]
pub fn ugen_name(spec: &UGenSpec) -> &'static str {
    match spec {
        UGenSpec::SinOsc => "sin-osc",
        UGenSpec::Saw => "saw",
        UGenSpec::Pulse => "pulse",
        UGenSpec::Tri => "tri",
        UGenSpec::WhiteNoise => "white-noise",
        UGenSpec::Lpf => "lpf",
        UGenSpec::Hpf => "hpf",
        UGenSpec::Bpf => "bpf",
        UGenSpec::Delay => "delay",
        UGenSpec::Comb => "comb",
        UGenSpec::EnvPerc => "env-perc",
        UGenSpec::EnvAdsr => "env-adsr",
        UGenSpec::Line => "line",
        UGenSpec::SamplePlay(_) => "sample-play",
        UGenSpec::Mul => "*",
        UGenSpec::Add => "+",
        UGenSpec::Const(_) => "const",
        UGenSpec::Param(_) => "param",
        UGenSpec::Vco { .. } => "vco",
        UGenSpec::SubOsc => "sub-osc",
        UGenSpec::Ladder => "ladder",
        UGenSpec::Svf => "svf",
        UGenSpec::FmOp => "fm-op",
        UGenSpec::FmMod => "fm-mod",
        UGenSpec::PhaseDistortion => "phase-distortion",
        UGenSpec::Additive { .. } => "additive",
        UGenSpec::Wavetable(_) => "wavetable",
        UGenSpec::Granular(_) => "granular",
        UGenSpec::Effect(e) => e.kind.name(),
    }
}

/// The user-facing ugen names (design-music section 7 sound row), each once.
pub const UGEN_NAMES: &[&str] = &[
    "sin-osc",
    "saw",
    "pulse",
    "tri",
    "white-noise",
    "lpf",
    "hpf",
    "bpf",
    "delay",
    "comb",
    "env-perc",
    "env-adsr",
    "line",
    "sample-play",
    "vco",
    "sub-osc",
    "ladder",
    "svf",
    "fm-op",
    "fm-mod",
    "phase-distortion",
    "additive",
    "wavetable",
    "granular",
];

/// The prelude synthesis templates (design 12.4).
pub const TEMPLATE_NAMES: &[&str] = &[
    "sampler",
    "analog",
    "fm",
    "pd",
    "additive",
    "wavetable",
    "granular",
];

/// The node of a spec (an effect's parameters are not included).
#[must_use]
pub fn node_of(spec: &UGenSpec) -> Node {
    match spec {
        UGenSpec::SinOsc => Node::SinOsc,
        UGenSpec::Saw => Node::Saw,
        UGenSpec::Pulse => Node::Pulse,
        UGenSpec::Tri => Node::Tri,
        UGenSpec::WhiteNoise => Node::WhiteNoise,
        UGenSpec::Lpf => Node::Lpf,
        UGenSpec::Hpf => Node::Hpf,
        UGenSpec::Bpf => Node::Bpf,
        UGenSpec::Delay => Node::Delay,
        UGenSpec::Comb => Node::Comb,
        UGenSpec::EnvPerc => Node::EnvPerc,
        UGenSpec::EnvAdsr => Node::EnvAdsr,
        UGenSpec::Line => Node::Line,
        UGenSpec::SamplePlay(b) => Node::SamplePlay(*b),
        UGenSpec::Mul => Node::Mul,
        UGenSpec::Add => Node::Add,
        UGenSpec::Const(v) => Node::Const(*v),
        UGenSpec::Param(c) => Node::Param(*c),
        UGenSpec::Vco { unison_max } => Node::Vco {
            unison_max: *unison_max,
        },
        UGenSpec::SubOsc => Node::SubOsc,
        UGenSpec::Ladder => Node::Ladder,
        UGenSpec::Svf => Node::Svf,
        UGenSpec::FmOp => Node::FmOp,
        UGenSpec::FmMod => Node::FmMod,
        UGenSpec::PhaseDistortion => Node::PhaseDistortion,
        UGenSpec::Additive { partials_max } => Node::Additive {
            partials_max: *partials_max,
        },
        UGenSpec::Wavetable(t) => Node::Wavetable(*t),
        UGenSpec::Granular(g) => Node::Granular(*g),
        UGenSpec::Effect(e) => Node::Effect {
            kind: e.kind,
            fx: 0,
        },
    }
}

// ---- node byte encoding (the graph codec's node layer, arena.rs) ----------

/// A little-endian graph-encoding writer.
pub(crate) struct Out<'a>(pub(crate) &'a mut Vec<u8>);

impl Out<'_> {
    pub(crate) fn u8(&mut self, x: u8) {
        self.0.push(x);
    }
    pub(crate) fn u16(&mut self, x: u16) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, x: u32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    pub(crate) fn f32(&mut self, x: f32) {
        self.u32(x.to_bits());
    }
    pub(crate) fn ctl(&mut self, id: CtlId, c: Ctl) {
        self.u16(id.get());
        match c {
            Ctl::Const(v) => {
                self.u8(0);
                self.f32(v);
            }
            Ctl::Cell(cell) => {
                self.u8(1);
                self.u32(cell.get());
            }
        }
    }
    pub(crate) fn len16(&mut self, n: usize) -> Result<(), BuildError> {
        self.u16(u16::try_from(n).map_err(|_| BuildError::TooManyNodes)?);
        Ok(())
    }
}

/// The catalog index of an effect kind.
pub(crate) fn effect_index(k: EffectKind) -> u8 {
    EffectKind::ALL
        .iter()
        .position(|x| *x == k)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

/// Encodes one node spec.
pub(crate) fn put_spec(o: &mut Out<'_>, spec: &UGenSpec) -> Result<(), BuildError> {
    let (tag, payload): (u8, u32) = match spec {
        UGenSpec::SinOsc => (0, 0),
        UGenSpec::Saw => (1, 0),
        UGenSpec::Pulse => (2, 0),
        UGenSpec::Tri => (3, 0),
        UGenSpec::WhiteNoise => (4, 0),
        UGenSpec::Lpf => (5, 0),
        UGenSpec::Hpf => (6, 0),
        UGenSpec::Bpf => (7, 0),
        UGenSpec::Delay => (8, 0),
        UGenSpec::Comb => (9, 0),
        UGenSpec::EnvPerc => (10, 0),
        UGenSpec::EnvAdsr => (11, 0),
        UGenSpec::Line => (12, 0),
        UGenSpec::SamplePlay(b) => (13, b.get()),
        UGenSpec::Mul => (14, 0),
        UGenSpec::Add => (15, 0),
        UGenSpec::Const(v) => (16, v.to_bits()),
        UGenSpec::Param(c) => (17, u32::from(c.get())),
        UGenSpec::Vco { unison_max } => (18, u32::from(*unison_max)),
        UGenSpec::SubOsc => (19, 0),
        UGenSpec::Ladder => (20, 0),
        UGenSpec::Svf => (21, 0),
        UGenSpec::FmOp => (22, 0),
        UGenSpec::FmMod => (23, 0),
        UGenSpec::PhaseDistortion => (24, 0),
        UGenSpec::Additive { partials_max } => (25, u32::from(*partials_max)),
        UGenSpec::Wavetable(t) => (26, t.get()),
        UGenSpec::Granular(GranSrc::Sample(b)) => (27, b.get()),
        UGenSpec::Granular(GranSrc::Table(t)) => (28, t.get()),
        UGenSpec::Granular(GranSrc::Bus) => (29, 0),
        UGenSpec::Effect(e) => {
            o.u8(30);
            o.u8(effect_index(e.kind));
            o.len16(e.params.len())?;
            for &(id, c) in e.params.iter() {
                o.ctl(id, c);
            }
            return Ok(());
        }
    };
    o.u8(tag);
    o.u32(payload);
    Ok(())
}

/// A bounds-checked graph-encoding reader.
pub(crate) struct In<'a> {
    pub(crate) b: &'a [u8],
    pub(crate) pos: usize,
}

impl In<'_> {
    pub(crate) fn take<const N: usize>(&mut self) -> Result<[u8; N], FaultCode> {
        let s = self
            .b
            .get(self.pos..self.pos + N)
            .ok_or(FaultCode::BadRecord)?;
        self.pos += N;
        let mut a = [0; N];
        a.copy_from_slice(s);
        Ok(a)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, FaultCode> {
        Ok(self.take::<1>()?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, FaultCode> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, FaultCode> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    pub(crate) fn ctl(&mut self) -> Result<(CtlId, Ctl), FaultCode> {
        let id = CtlId::new(self.u16()?);
        let c = match self.u8()? {
            0 => Ctl::Const(f32::from_bits(self.u32()?)),
            1 => Ctl::Cell(CellId::new(self.u32()?)),
            _ => return Err(FaultCode::BadRecord),
        };
        Ok((id, c))
    }
}

/// Decodes one node into `raw`.
pub(crate) fn get_node(i: &mut In<'_>, raw: &mut RawGraph) -> Result<(), FaultCode> {
    let tag = i.u8()?;
    if tag == 30 {
        let kind = *EffectKind::ALL
            .get(usize::from(i.u8()?))
            .ok_or(FaultCode::BadRecord)?;
        let n = raw
            .push_node(Node::Effect { kind, fx: 0 })
            .map_err(|_| FaultCode::GraphTooLarge)?;
        for _ in 0..i.u16()? {
            let (id, c) = i.ctl()?;
            raw.push_node_param(n, id, c)
                .map_err(|_| FaultCode::GraphTooLarge)?;
        }
        return Ok(());
    }
    let p = i.u32()?;
    let small = u8::try_from(p).unwrap_or(u8::MAX);
    let node = match tag {
        0 => Node::SinOsc,
        1 => Node::Saw,
        2 => Node::Pulse,
        3 => Node::Tri,
        4 => Node::WhiteNoise,
        5 => Node::Lpf,
        6 => Node::Hpf,
        7 => Node::Bpf,
        8 => Node::Delay,
        9 => Node::Comb,
        10 => Node::EnvPerc,
        11 => Node::EnvAdsr,
        12 => Node::Line,
        13 => Node::SamplePlay(BankRef::new(p)),
        14 => Node::Mul,
        15 => Node::Add,
        16 => Node::Const(f32::from_bits(p)),
        17 => Node::Param(CtlId::new(
            u16::try_from(p).map_err(|_| FaultCode::BadRecord)?,
        )),
        18 => Node::Vco { unison_max: small },
        19 => Node::SubOsc,
        20 => Node::Ladder,
        21 => Node::Svf,
        22 => Node::FmOp,
        23 => Node::FmMod,
        24 => Node::PhaseDistortion,
        25 => Node::Additive {
            partials_max: small,
        },
        26 => Node::Wavetable(TableRef::new(p)),
        27 => Node::Granular(GranSrc::Sample(BankRef::new(p))),
        28 => Node::Granular(GranSrc::Table(TableRef::new(p))),
        29 => Node::Granular(GranSrc::Bus),
        _ => return Err(FaultCode::BadRecord),
    };
    raw.push_node(node).map_err(|_| FaultCode::GraphTooLarge)?;
    Ok(())
}
