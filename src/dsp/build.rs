//! Lowering an `inst` or `bus` node tree to its POD template (design 12.1,
//! 12.5, 12.8.6).
//!
//! The encoding BE-DSP's voices and buses read:
//!
//! - `InstDef::nodes` is in post order: every input precedes the node that
//!   reads it, and the output is the LAST node. A node shared in the tree
//!   (the same `Rc`) is lowered once; each control has one `Param` node.
//! - Every input is an `Edge`. A non-effect argument's name is its own
//!   keyword or its `ports()` position; the edge's port is that name's
//!   index in the RUNTIME catalog port list (`catalog_port`, R2, aliased by
//!   `catalog_alias`), else `NAMED_PORT + param_id`. A resource argument
//!   (`is_resource_arg`: sample-play `bank`/`n`, wavetable `table`,
//!   granular `source`) is skipped: no node, no edge — the voice reads the
//!   event's `bank`; commit selects `n`. An EFFECT parameter (unit or
//!   bus/master chain) is effect-local (`effects::param_ctl`, R6, never
//!   `param_id`); non-constant lands on port `1 + index` (subject port 0).
//! - A number is a `Const` node, a control name a `Param` node, a keyword the
//!   index in its port's enum (a bank, table or source keyword is the
//!   resource placeholder `0`: `ns::insts::realize_inst` records the header
//!   default's keyword on the `InstEntry` instead; `InstResolver::route`
//!   resolves it to the installed sample), a list (`partials:`) `node_params`
//!   entries `(node, param_id, Const)` in order.
//! - A signal becomes a synthetic parameter `SIGNAL_CTL_BASE + k` whose
//!   default is a `Ctl::Cell` the scheduler writes once per tick (returned
//!   as a `SignalInput`, 12.8.6); with no free cell it is `Ctl::Const(0)`.
//! - A `BusDef` is the linear effect chain from the bus input outwards.
//!
//! More than `NODE_CAP` nodes (or effects) is `graph-too-large` (16.1).

use std::collections::HashMap;
use std::rc::Rc;

use crate::dsp::caps::{Cap, CapabilitySet};
use crate::dsp::cells::CellId;
use crate::dsp::controls::{self, CtlDomain};
use crate::dsp::effects;
use crate::dsp::graph::{
    BusDef, BusId, Edge, EffectKind, EffectSpec, InstDef, InstId, UGenInput, UGenKind, UGenNode,
    UGenSpec, NODE_CAP,
};
use crate::host::caps::SignalInput;
use crate::host::wire::Ctl;
use crate::pattern::signal::Sig;
use crate::reader::span::Span;
use crate::sched::slots::CtlId;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::{name_of_kw, KwId};
use crate::vm::fail::{FailCode, Failure};

mod names;

use names::{catalog_port, is_resource_arg, CODEC_KINDS, RADIO_KINDS};
pub use names::{
    is_processor, ports, ugen_named, DSP_KEYWORDS, EXTRA_CTL_BASE, EXTRA_PARAMS, NAMED_PORT,
    SIGNAL_CTL_BASE, SVF_MODES, UGENS,
};

/// The parameters an effect's positional arguments (after its subject)
/// name, in order.
#[must_use]
pub fn effect_ports(kind: EffectKind) -> &'static [&'static str] {
    match kind {
        EffectKind::Lpf | EffectKind::Hpf | EffectKind::Bpf | EffectKind::Notch => {
            &["cutoff", "res"]
        }
        EffectKind::Delay | EffectKind::PingPong | EffectKind::Comb => &["time", "feedback"],
        EffectKind::Gain => &["gain"],
        EffectKind::Pan | EffectKind::Balance => &["pan"],
        EffectKind::Room => &["room", "size"],
        EffectKind::Width => &["width"],
        _ => &["amount"],
    }
}

/// The wire id of a parameter name: its control-table id, else its
/// `EXTRA_PARAMS` id.
#[must_use]
pub fn param_id(name: &str) -> Option<CtlId> {
    if let Some(r) = controls::row(name) {
        return Some(r.ctl);
    }
    let k = EXTRA_PARAMS.iter().position(|p| *p == name)?;
    Some(CtlId::new(EXTRA_CTL_BASE + u16::try_from(k).ok()?))
}

/// A lowering problem: the failure of the defining form, and the
/// diagnostic when there is one (`graph-too-large`).
#[derive(Clone, Debug)]
pub struct LowerError {
    pub failure: Failure,
    pub diag: Option<Box<Diagnostic>>,
}

impl LowerError {
    fn ty(msg: impl Into<String>) -> Self {
        Self {
            failure: Failure::new(FailCode::Type, msg),
            diag: None,
        }
    }
}

/// What lowering produced besides the template.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub signals: Vec<SignalInput>,
    /// `beyond-capability` findings (the definition still installs; the
    /// audio side clamps, 12.6).
    pub diags: Vec<Diagnostic>,
}

/// The shared lowering context: host limits, the definition's span, and
/// the cell allocator for signal inputs (the instrument registry's).
pub struct Lowering<'a> {
    pub caps: &'a CapabilitySet,
    pub span: Span,
    pub alloc: &'a mut dyn FnMut() -> Option<CellId>,
}

/// Lowers an `inst` body's tree; `header` is the parameter table.
///
/// # Errors
/// `graph-too-large` past `NODE_CAP` nodes; `type` for an inexpressible or
/// unknown-parameter node.
pub fn lower_inst(
    id: InstId,
    root: &Rc<UGenNode>,
    header: &[(CtlId, Ctl)],
    lw: Lowering<'_>,
) -> Result<(InstDef, Extras), LowerError> {
    let mut g = Graph {
        lw,
        nodes: Vec::new(),
        edges: Vec::new(),
        node_params: Vec::new(),
        params: header.to_vec(),
        memo: HashMap::new(),
        param_nodes: HashMap::new(),
        consts: HashMap::new(),
        extras: Extras::default(),
    };
    g.node(root, 0)?;
    let def = InstDef {
        id,
        params: g.params.into_boxed_slice(),
        nodes: g.nodes.into_boxed_slice(),
        edges: g.edges.into_boxed_slice(),
        node_params: g.node_params.into_boxed_slice(),
    };
    Ok((def, g.extras))
}

/// Lowers a `bus`/`master` body: the effect chain from the bus input out.
///
/// # Errors
/// `type` off a linear effect chain; `graph-too-large` past `NODE_CAP`.
pub fn lower_bus(
    id: BusId,
    root: &Rc<UGenNode>,
    mut lw: Lowering<'_>,
) -> Result<(BusDef, Extras), LowerError> {
    let mut chain = Vec::new();
    let mut extras = Extras::default();
    let mut cur = Rc::clone(root);
    loop {
        if chain.len() > NODE_CAP {
            return Err(too_large(lw.span, "a bus chain"));
        }
        let kind = match &cur.kind {
            UGenKind::BusInput => break,
            UGenKind::Effect(k) => *k,
            UGenKind::Ugen(_) => {
                return Err(LowerError::ty(
                    "a bus chain is a chain of effects over the bus input",
                ))
            }
        };
        let mut params = Vec::new();
        let mut next = None;
        let mut pos = 0;
        for (k, (name, inp)) in cur.args.iter().enumerate() {
            match (name, inp) {
                (None, UGenInput::Node(n)) if k == 0 => next = Some(Rc::clone(n)),
                (_, UGenInput::Node(_)) => {
                    return Err(LowerError::ty(
                        "a bus parameter takes a number, keyword, bool or signal",
                    ))
                }
                _ => {
                    let pname = port_name(*name, effect_ports(kind), &mut pos)?;
                    let ctl = bus_param(kind, &pname, inp, &mut lw, &mut extras)?;
                    // `room`'s first port is really its `mix` parameter.
                    let alias = (kind == EffectKind::Room && &*pname == "room").then_some("mix");
                    let id = effects::param_ctl(kind, alias.unwrap_or(&pname))
                        .ok_or_else(|| LowerError::ty(format!("unknown parameter `{pname}:`")))?;
                    params.push((id, ctl));
                }
            }
        }
        chain.push(EffectSpec {
            kind,
            params: params.into_boxed_slice(),
        });
        match next {
            Some(n) => cur = n,
            None => break,
        }
    }
    chain.reverse();
    Ok((
        BusDef {
            id,
            chain: chain.into_boxed_slice(),
        },
        extras,
    ))
}

fn too_large(span: Span, what: &str) -> LowerError {
    let msg = format!("{what} has more than {NODE_CAP} nodes");
    LowerError {
        failure: Failure::new(FailCode::InstFailed, msg.clone()),
        diag: Some(Box::new(Diagnostic::error(
            DiagCode::GraphTooLarge,
            span,
            msg,
        ))),
    }
}

/// The parameter name of an argument: its own name, else the next
/// positional port.
fn port_name(
    name: Option<KwId>,
    ports: &[&'static str],
    pos: &mut usize,
) -> Result<Rc<str>, LowerError> {
    match name {
        Some(k) => Ok(name_of_kw(k)),
        None => {
            let p = ports
                .get(*pos)
                .ok_or_else(|| LowerError::ty("too many positional parameters"))?;
            *pos += 1;
            Ok(Rc::from(*p))
        }
    }
}

fn id_of(name: &str) -> Result<CtlId, LowerError> {
    param_id(name).ok_or_else(|| LowerError::ty(format!("unknown parameter `{name}:`")))
}

/// A bus unit's parameter value.
fn bus_param(
    kind: EffectKind,
    pname: &str,
    inp: &UGenInput,
    lw: &mut Lowering<'_>,
    extras: &mut Extras,
) -> Result<Ctl, LowerError> {
    Ok(match inp {
        UGenInput::Const(v) => Ctl::Const(*v),
        UGenInput::Keyword(k) => Ctl::Const(keyword(Some(kind), pname, *k)?),
        // A control name has no voice on a bus: the unit starts from the
        // control's default (per-event routing is the scheduler's).
        UGenInput::Param(c) => Ctl::Const(controls::row_by_id(*c).map_or(0.0, |r| r.default)),
        UGenInput::Signal(s) => signal_cell(s, lw, extras),
        UGenInput::List(_) | UGenInput::Node(_) => {
            return Err(LowerError::ty(format!(
                "the bus parameter `{pname}:` takes a number, keyword, bool or signal"
            )))
        }
    })
}

/// A signal input needs a live cell (the scheduler writes it once per
/// tick); with the instrument-default pool exhausted (DDRUM-002A), the
/// input still installs, frozen at `0.0`, but reports a `cell-capacity`
/// diagnostic instead of silently dropping the signal.
fn signal_cell(s: &Rc<Sig>, lw: &mut Lowering<'_>, extras: &mut Extras) -> Ctl {
    match (lw.alloc)() {
        Some(cell) => {
            extras.signals.push(SignalInput {
                cell,
                sig: Rc::clone(s),
            });
            Ctl::Cell(cell)
        }
        None => {
            extras.diags.push(Diagnostic::error(
                DiagCode::CellCapacity,
                lw.span,
                "a signal input has no free instrument-default cell; it is frozen at 0.0",
            ));
            Ctl::Const(0.0)
        }
    }
}

/// A keyword argument: its index in the port's enum; a resource keyword
/// (bank, table, source) is the placeholder `0`.
fn keyword(effect: Option<EffectKind>, port: &str, k: KwId) -> Result<f32, LowerError> {
    let name = name_of_kw(k);
    let list: Option<&[&str]> = match (effect, port) {
        (Some(EffectKind::Codec), "kind") => Some(CODEC_KINDS),
        (Some(EffectKind::Radio), "kind") => Some(RADIO_KINDS),
        (_, "mode") => Some(SVF_MODES),
        _ => match controls::row(port).map(|r| r.domain) {
            Some(CtlDomain::Enum(names)) => Some(names),
            Some(CtlDomain::Resource) => return Ok(0.0),
            _ => None,
        },
    };
    #[allow(clippy::cast_precision_loss)]
    list.and_then(|l| l.iter().position(|n| *n == &*name))
        .map(|i| i as f32)
        .ok_or_else(|| LowerError::ty(format!("`{port}:` does not take :{name}")))
}

/// One lowering in progress.
struct Graph<'a> {
    lw: Lowering<'a>,
    nodes: Vec<UGenSpec>,
    edges: Vec<Edge>,
    node_params: Vec<(u16, CtlId, Ctl)>,
    params: Vec<(CtlId, Ctl)>,
    memo: HashMap<*const UGenNode, u16>,
    param_nodes: HashMap<CtlId, u16>,
    consts: HashMap<u32, u16>,
    extras: Extras,
}

/// Where one input comes from.
enum Src {
    Node(u16),
    List(Rc<[f32]>),
}

impl Graph<'_> {
    fn push(&mut self, spec: UGenSpec) -> Result<u16, LowerError> {
        if self.nodes.len() >= NODE_CAP {
            return Err(too_large(self.lw.span, "the instrument"));
        }
        self.nodes.push(spec);
        u16::try_from(self.nodes.len() - 1).map_err(|_| too_large(self.lw.span, "the instrument"))
    }

    fn param(&mut self, c: CtlId) -> Result<u16, LowerError> {
        if let Some(i) = self.param_nodes.get(&c) {
            return Ok(*i);
        }
        let i = self.push(UGenSpec::Param(c))?;
        self.param_nodes.insert(c, i);
        Ok(i)
    }

    fn konst(&mut self, v: f32) -> Result<u16, LowerError> {
        if let Some(i) = self.consts.get(&v.to_bits()) {
            return Ok(*i);
        }
        let i = self.push(UGenSpec::Const(v))?;
        self.consts.insert(v.to_bits(), i);
        Ok(i)
    }

    fn input(
        &mut self,
        inp: &UGenInput,
        effect: Option<EffectKind>,
        port: &str,
        depth: usize,
    ) -> Result<Src, LowerError> {
        Ok(Src::Node(match inp {
            UGenInput::Node(n) => self.node(n, depth + 1)?,
            UGenInput::Const(v) => self.konst(*v)?,
            UGenInput::Param(c) => self.param(*c)?,
            UGenInput::Keyword(k) => self.konst(keyword(effect, port, *k)?)?,
            UGenInput::List(xs) => return Ok(Src::List(Rc::clone(xs))),
            UGenInput::Signal(s) => {
                let k = u16::try_from(self.extras.signals.len()).unwrap_or(u16::MAX);
                let ctl = CtlId::new(SIGNAL_CTL_BASE.saturating_add(k));
                let cell = signal_cell(s, &mut self.lw, &mut self.extras);
                self.params.push((ctl, cell));
                self.param(ctl)?
            }
        }))
    }

    /// Lowers one node after its inputs; returns its index.
    fn node(&mut self, n: &Rc<UGenNode>, depth: usize) -> Result<u16, LowerError> {
        if let Some(i) = self.memo.get(&Rc::as_ptr(n)) {
            return Ok(*i);
        }
        // A path longer than the cap has more distinct nodes than the cap.
        if depth > NODE_CAP {
            return Err(too_large(self.lw.span, "the instrument"));
        }
        let (mut spec, effect) = match &n.kind {
            UGenKind::Ugen(UGenSpec::Param(c)) => return self.param(*c),
            UGenKind::Ugen(UGenSpec::Const(v)) => return self.konst(*v),
            UGenKind::Ugen(s) => (s.clone(), None),
            UGenKind::Effect(k) => (
                UGenSpec::Effect(EffectSpec {
                    kind: *k,
                    params: Box::new([]),
                }),
                Some(*k),
            ),
            UGenKind::BusInput => {
                return Err(LowerError::ty(
                    "an instrument has no bus input; effects in an `inst` need a subject",
                ))
            }
        };
        let port_list = match effect {
            Some(k) => effect_ports(k),
            None => ports(&spec),
        };
        let mut wires: Vec<(Src, u8, Rc<str>)> = Vec::new();
        let mut fx_params: Vec<(CtlId, Ctl)> = Vec::new();
        let mut pos = 0usize;
        for (k, (name, inp)) in n.args.iter().enumerate() {
            let subject = effect.is_some() && k == 0 && name.is_none();
            if subject {
                let src = self.input(inp, effect, "in", depth)?;
                wires.push((src, 0, Rc::from("in")));
                continue;
            }
            let pname = port_name(*name, port_list, &mut pos)?;
            // R2a: no node, no edge (a dangling node would add DC).
            if effect.is_none() && is_resource_arg(&spec, &pname) {
                continue;
            }
            // R6: effect-local id; non-constant lands on port `1 + index`.
            if let Some(kind) = effect {
                let ctl = effects::param_ctl(kind, &pname)
                    .ok_or_else(|| LowerError::ty(format!("unknown parameter `{pname}:`")))?;
                let v = match inp {
                    UGenInput::Const(v) => Some(*v),
                    UGenInput::Keyword(kw) => Some(keyword(Some(kind), &pname, *kw)?),
                    _ => None,
                };
                if let Some(v) = v {
                    fx_params.push((ctl, Ctl::Const(v)));
                    continue;
                }
                let port =
                    u8::try_from(1 + ctl.get() - effects::EFFECT_PARAM_BASE).unwrap_or(u8::MAX);
                let src = self.input(inp, effect, &pname, depth)?;
                wires.push((src, port, pname));
                continue;
            }
            let listed = port_list.iter().position(|p| **p == *pname);
            if (matches!(spec, UGenSpec::FrameKeyframe { .. }) && &*pname == "frames")
                || (matches!(spec, UGenSpec::StageLinked { .. }) && &*pname == "segments")
            {
                if !matches!(inp, UGenInput::List(_)) {
                    return Err(LowerError::ty("payload requires a constant numeric list"));
                }
                continue;
            }
            // A list input (`partials:`) never becomes an edge (it lands in
            // `node_params`, addressed by `id_of`, below): it needs no port.
            let port = if matches!(inp, UGenInput::List(_)) {
                0
            } else if listed.is_some() {
                // R2a: the RUNTIME catalog's port index, not `ports()`'s.
                catalog_port(&spec, &pname).ok_or_else(|| {
                    LowerError::ty(format!("parameter `{pname}:` has no runtime port"))
                })?
            } else {
                let id = id_of(&pname)?.get();
                u8::try_from(u16::from(NAMED_PORT) + id)
                    .map_err(|_| LowerError::ty(format!("parameter `{pname}:` has no port")))?
            };
            let src = self.input(inp, effect, &pname, depth)?;
            wires.push((src, port, pname));
        }
        self.refine(&mut spec, n)?;
        if let UGenSpec::Effect(e) = &mut spec {
            e.params = fx_params.into_boxed_slice();
        }
        let me = self.push(spec)?;
        for (src, port, pname) in wires {
            match src {
                Src::Node(from) => self.edges.push(Edge { from, to: me, port }),
                Src::List(xs) => {
                    let id = match effect {
                        Some(kind) => effects::param_ctl(kind, &pname).ok_or_else(|| {
                            LowerError::ty(format!("unknown parameter `{pname}:`"))
                        })?,
                        None => id_of(&pname)?,
                    };
                    for x in xs.iter() {
                        self.node_params.push((me, id, Ctl::Const(*x)));
                    }
                }
            }
        }
        self.memo.insert(Rc::as_ptr(n), me);
        Ok(me)
    }

    /// Fills node-shape constants from the arguments (unison and partial
    /// counts) and checks grain limits that are constant (12.8.8).
    fn refine(&mut self, spec: &mut UGenSpec, n: &UGenNode) -> Result<(), LowerError> {
        let arg = |name: &str| {
            n.args
                .iter()
                .find(|(k, _)| k.is_some_and(|k| &*name_of_kw(k) == name))
                .map(|(_, v)| v)
        };
        match spec {
            UGenSpec::FrameKeyframe { data } => {
                let Some(UGenInput::List(xs)) = arg("frames") else {
                    return Err(LowerError::ty("frames: requires a constant numeric list"));
                };
                let parsed = crate::dsp::ugen::frame_keyframe::FrameData::from_flat(xs)
                    .map_err(LowerError::ty)?;
                *data = Some(Box::new(parsed));
            }
            UGenSpec::StageLinked { data } => {
                let Some(UGenInput::List(xs)) = arg("segments") else {
                    return Err(LowerError::ty("segments: requires a constant numeric list"));
                };
                let parsed = crate::dsp::ugen::stage_linked::StageData::from_flat(xs)
                    .map_err(LowerError::ty)?;
                *data = Some(Box::new(parsed));
            }
            UGenSpec::Vco { unison_max } => {
                *unison_max = match arg("unison") {
                    Some(UGenInput::Const(v)) => clamp_u8(*v, 1, 16),
                    Some(_) => 16,
                    None => 1,
                };
            }
            UGenSpec::Additive { partials_max } => {
                *partials_max = match arg("partials") {
                    Some(UGenInput::List(xs)) => u8::try_from(xs.len()).unwrap_or(u8::MAX),
                    _ => 16,
                };
            }
            UGenSpec::Granular(_) => {
                let caps = self.lw.caps;
                let span = Some(self.lw.span);
                if let Some(UGenInput::Const(d)) = arg("density") {
                    if let Err(diag) = caps.require(Cap::GrainDensity(*d), span) {
                        self.extras.diags.push(diag);
                    }
                }
                if let Some(UGenInput::Const(s)) = arg("size") {
                    if let Err(diag) = caps.require(Cap::GrainSize(*s), span) {
                        self.extras.diags.push(diag);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn clamp_u8(v: f32, lo: u8, hi: u8) -> u8 {
    if v.is_nan() {
        return lo;
    }
    (v.round().clamp(f32::from(lo), f32::from(hi))) as u8
}
