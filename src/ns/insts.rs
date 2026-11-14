//! The instrument registry (design 12.8.6).
//!
//! An `inst` definition compiles to a closure; after the defining form runs,
//! `realize_inst` calls that closure ONCE with every header parameter bound
//! to a `Param` node (M3: header parameters are control names), lowers the
//! returned tree to an `InstDef` (`dsp::build`), registers it under the
//! definition's name, rebinds the name to `Sound::Inst(id)` and stages the
//! `Install` effect. A failing body is `inst-failed` for the defining form,
//! except output selection errors which retain their call failure code. The
//! whole-form rollback keeps the previous definition.
//!
//! The registry is shared (`Rc<RefCell<..>>`) by the evaluator side (the VM
//! reads it for realization, `bus`/`master` and the `s :k` fallback) and the
//! runtime (`InstResolver`). The prelude templates (`sampler`, `analog`,
//! `fm`, `pd`, `additive`, `wavetable`, `granular`) are ordinary `inst`
//! source in `src/prelude/templates.vact`, realized into the registry when
//! the evaluator is built. They are sounds (`s :analog`), not prelude
//! bindings: `additive`, `wavetable` and `granular` stay the ugens.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::compile::{compile, CompileCx};
use crate::dsp::build::{
    header_resource, lower_inst_with_resources, DeclaredGraphResource, GraphResourceCaptureMode,
    GraphResourceInput, LowerError, Lowering,
};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellId;
use crate::dsp::controls::{self, CtlDomain, DeclaredParam, ScalarType};
use crate::dsp::graph::{BusDef, BusId, InstDef, InstId, UGenKind, UGenNode, UGenSpec};
use crate::host::caps::{GraphHandle, InstResolver, Route, SampleSrc, SignalInput};
use crate::host::wire::Ctl;
use crate::ns::load::read_forms;
use crate::ns::namespace::{FormGen, Namespace, Prelude, SlotKind, VarSlotRef};
use crate::ns::stage::{EffectSink, StagedEffect};
use crate::reader::node::Node;
use crate::reader::span::{FileId, Span};
use crate::sched::slots::CtlId;
use crate::types::diag::Diagnostic;
use crate::value::intern::{intern_kw, intern_sym, name_of_kw, name_of_sym, KwId};
use crate::value::value::{Sound, Value};
use crate::vm::call::kind_name;
use crate::vm::fail::{FailCode, Failure, FailureKind};
use crate::vm::vm::Vm;

/// The prelude template source (design 12.4, 12.8.6).
pub const TEMPLATES: &str = include_str!("../prelude/templates.vact");
/// The file id the template source reads under.
pub const TEMPLATE_FILE: FileId = FileId::new(u32::MAX - 1);
/// The prelude template names, in source order.
pub const TEMPLATE_NAMES: [&str; 73] = [
    "sampler",
    "analog",
    "fm",
    "phase-drum",
    "feedback-metal-drum",
    "digital-drum",
    "digital-snare",
    "digital-metal",
    "digital-hat",
    "low-drum",
    "wire-drum",
    "metal-hat",
    "filter-voice",
    "phase-pair-voice",
    "fm-pair-voice",
    "six-bank-a-voice",
    "six-bank-b-voice",
    "six-bank-c-voice",
    "speech-voice",
    "resonator-voice",
    "string-choir-voice",
    "exciter-voice",
    "spectrum-voice",
    "clock-noise-voice",
    "dual-kick-voice",
    "dual-snare-voice",
    "dual-hat-voice",
    "swarm-voice",
    "particle-voice",
    "modal-voice",
    "string-voice",
    "chip-voice",
    "analog-pair-voice",
    "grain-pair-voice",
    "shape-voice",
    "string-machine-voice",
    "terrain-voice",
    "wave-grid-voice",
    "chord-layer-voice",
    "macro-five-voice",
    "macro-sub-sync-voice",
    "macro-triple-voice",
    "macro-digital-voice",
    "macro-filter-voice",
    "macro-formant-voice",
    "macro-fm-voice",
    "macro-physical-voice",
    "macro-struck-voice",
    "macro-percussion-voice",
    "macro-wave-grid-voice",
    "macro-wave-line-voice",
    "macro-noise-voice",
    "macro-cloud-voice",
    "fusion-drum",
    "pd",
    "additive",
    "wavetable",
    "granular",
    "tidal-voice",
    "tidal-poly-voice",
    "peak-motion-voice",
    "stage-voice",
    "stage-chain-voice",
    "frame-lfo-voice",
    "frame-keyframe-voice",
    "peak-pulse-voice",
    "number-station-voice",
    "analog-bass",
    "acid-bass",
    "fm-bass",
    "wobble-bass",
    "sub-bass",
    "reese-bass",
];

/// The control cells the registry hands out (inst defaults that are tweak
/// sites, signal inputs): the top of the scheduler's default pool of 1024.
pub const INST_CELL_BASE: u32 = 896;
pub const INST_CELL_COUNT: u32 = 128;
/// The first id of a header parameter that is not a control-table row.
pub const CUSTOM_CTL_BASE: u16 = 192;

/// The VM's view of instruments: the registry, and whether an `inst` body
/// (realization) or a `bus`/`master` body is being evaluated (B2).
#[derive(Default)]
pub struct DspCx {
    pub registry: Option<Rc<RefCell<InstRegistry>>>,
    pub inst: u32,
    pub bus: u32,
    /// Realization diagnostics (`graph-too-large`, `beyond-capability`)
    /// the evaluator attaches to the defining form.
    pub diags: Vec<Diagnostic>,
}

/// One installed instrument.
#[derive(Clone, Debug)]
pub struct InstEntry {
    pub name: KwId,
    pub def: Arc<InstDef>,
    /// Instrument-local control schema used by commit and editor metadata.
    pub params: Vec<DeclaredParam>,
    /// Cell-backed defaults and the tweak slots that feed them.
    pub cells: Vec<(CellId, VarSlotRef)>,
    pub signals: Vec<SignalInput>,
    /// The default keyword of a `CtlDomain::Resource` header parameter
    /// (`bank`, `table` or `source`), when the instrument declares one
    /// (R2b, serial repair of the BE-FINAL STOP finding): `route` resolves
    /// it to a `SampleSrc::Bank`, the same way a raw host-bank sound does.
    pub resource: Option<KwId>,
}

impl InstEntry {
    /// The evaluated header-site value for an instrument-owned default cell.
    /// Invalid later tweak values leave the last valid audio value in place.
    #[must_use]
    pub fn default_cell_value(&self, cell: CellId) -> Option<f32> {
        let (_, slot) = self.cells.iter().find(|(id, _)| *id == cell)?;
        let ctl = self
            .def
            .params
            .iter()
            .find_map(|(ctl, value)| (*value == Ctl::Cell(cell)).then_some(*ctl))?;
        self.params
            .iter()
            .find(|param| param.ctl == ctl)?
            .encode(&slot.get())
            .ok()
    }
}

/// One installed bus (or `master`).
#[derive(Clone, Debug)]
pub struct BusEntry {
    pub id: BusId,
    pub def: Arc<BusDef>,
    pub signals: Vec<SignalInput>,
}

/// Genuine resources paired with the exact committed instrument allocation.
/// ```compile_fail
/// use vactr::ns::insts::InstResourceDeclaration;
/// fn alter(mut record: InstResourceDeclaration) { record.resources.clear(); }
/// ```
#[derive(Clone, Debug)]
pub struct InstResourceDeclaration {
    graph: Arc<InstDef>,
    resources: Vec<DeclaredGraphResource>,
}
impl InstResourceDeclaration {
    #[must_use]
    pub fn graph(&self) -> &Arc<InstDef> {
        &self.graph
    }
    #[must_use]
    pub fn resources(&self) -> &[DeclaredGraphResource] {
        &self.resources
    }
}
/// Genuine resources paired with the exact committed bus allocation.
#[derive(Clone, Debug)]
pub struct BusResourceDeclaration {
    graph: Arc<BusDef>,
    resources: Vec<DeclaredGraphResource>,
}
impl BusResourceDeclaration {
    #[must_use]
    pub fn graph(&self) -> &Arc<BusDef> {
        &self.graph
    }
    #[must_use]
    pub fn resources(&self) -> &[DeclaredGraphResource] {
        &self.resources
    }
}

/// Instruments and buses by name.
#[derive(Debug)]
pub struct InstRegistry {
    capture_mode: GraphResourceCaptureMode,
    inst_resources: BTreeMap<InstId, InstResourceDeclaration>,
    bus_resources: BTreeMap<BusId, BusResourceDeclaration>,
    insts: BTreeMap<InstId, InstEntry>,
    names: BTreeMap<KwId, InstId>,
    buses: BTreeMap<KwId, BusEntry>,
    master: Option<BusEntry>,
    next_inst: u32,
    next_bus: u32,
    next_cell: u32,
    cell_end: u32,
    custom: BTreeMap<KwId, CtlId>,
    /// The host limits realization checks against (12.8.8).
    pub caps: CapabilitySet,
    template_errors: Vec<String>,
}

impl Default for InstRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InstRegistry {
    /// An empty registry over the native tier's limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            capture_mode: GraphResourceCaptureMode::Legacy,
            inst_resources: BTreeMap::new(),
            bus_resources: BTreeMap::new(),
            insts: BTreeMap::new(),
            names: BTreeMap::new(),
            buses: BTreeMap::new(),
            master: None,
            next_inst: 0,
            // Bus id 0 is `master`.
            next_bus: 1,
            next_cell: INST_CELL_BASE,
            cell_end: INST_CELL_BASE + INST_CELL_COUNT,
            custom: BTreeMap::new(),
            caps: CapabilitySet::native(),
            template_errors: Vec::new(),
        }
    }

    /// A new registry, shareable by the evaluator and the runtime.
    #[must_use]
    pub fn shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }

    /// The instrument registered as `name`.
    #[must_use]
    pub fn id_of(&self, name: KwId) -> Option<InstId> {
        self.names.get(&name).copied()
    }

    /// `Sound::Inst` of the instrument registered as `name`.
    #[must_use]
    pub fn sound(&self, name: KwId) -> Option<Value> {
        self.id_of(name)
            .map(|id| Value::Sound(Rc::new(Sound::Inst(id))))
    }

    /// An installed instrument.
    #[must_use]
    pub fn entry(&self, id: InstId) -> Option<&InstEntry> {
        self.insts.get(&id)
    }

    /// Every installed instrument, by id.
    pub fn entries(&self) -> impl Iterator<Item = &InstEntry> + '_ {
        self.insts.values()
    }

    /// Parameter names of installed instruments, for checking and compiling
    /// pattern steps in subsequent forms.
    pub fn declared_names(&self) -> impl Iterator<Item = Rc<str>> + '_ {
        self.insts
            .values()
            .flat_map(|entry| entry.params.iter())
            .map(|param| name_of_kw(param.name))
    }

    /// The bus declared as `name`.
    #[must_use]
    pub fn bus(&self, name: KwId) -> Option<&BusEntry> {
        self.buses.get(&name)
    }

    /// The `master` chain.
    #[must_use]
    pub fn master(&self) -> Option<&BusEntry> {
        self.master.as_ref()
    }

    /// Every installed named bus and `master`, for the editor's analyzer
    /// telemetry (design 15.1.2 G4): `None` names `master`.
    pub fn buses(&self) -> impl Iterator<Item = (Option<KwId>, &BusEntry)> {
        self.buses
            .iter()
            .map(|(k, e)| (Some(*k), e))
            .chain(self.master.iter().map(|e| (None, e)))
    }

    /// Cell-backed defaults of an instrument and their tweak slots: the
    /// runtime initializes each cell from its slot and writes it on
    /// `TweakRefresh` (11.3, 13).
    #[must_use]
    pub fn default_cells(&self, id: InstId) -> Vec<(CellId, VarSlotRef)> {
        self.insts
            .get(&id)
            .map_or_else(Vec::new, |e| e.cells.clone())
    }

    /// Problems loading the prelude templates (none is expected).
    #[must_use]
    pub fn template_errors(&self) -> &[String] {
        &self.template_errors
    }

    /// The wire id of a header parameter: its control-table row, else a
    /// per-registry custom id.
    pub fn ctl_of(&mut self, name: KwId) -> Result<CtlId, Failure> {
        if let Some(r) = controls::row(&name_of_kw(name)) {
            return Ok(r.ctl);
        }
        if let Some(id) = self.custom.get(&name) {
            return Ok(*id);
        }
        let offset = u16::try_from(self.custom.len()).map_err(|_| {
            Failure::new(
                FailCode::TooManyControls,
                "too many custom instrument parameter names",
            )
        })?;
        let next = CUSTOM_CTL_BASE
            .checked_add(offset)
            .filter(|id| *id < crate::dsp::build::SIGNAL_CTL_BASE)
            .ok_or_else(|| {
                Failure::new(
                    FailCode::TooManyControls,
                    "custom instrument parameter ids are exhausted",
                )
            })?;
        let id = CtlId::new(next);
        self.custom.insert(name, id);
        Ok(id)
    }

    /// A control cell from the registry's range; `None` when it is used up
    /// (the caller falls back to a constant, as the scheduler's pool does).
    pub fn alloc_cell(&mut self) -> Option<CellId> {
        if self.next_cell >= self.cell_end {
            return None;
        }
        self.next_cell += 1;
        Some(CellId::new(self.next_cell - 1))
    }

    /// The id `name` has or would get.
    fn peek_id(&self, name: KwId) -> InstId {
        self.id_of(name).unwrap_or(InstId::new(self.next_inst))
    }

    fn install(&mut self, entry: InstEntry, id: InstId) {
        self.inst_resources.remove(&id);
        if self.names.insert(entry.name, id).is_none() {
            self.next_inst += 1;
        }
        self.insts.insert(id, entry);
    }

    /// Registers a bus definition (`None` for `master`); returns the graph
    /// to install.
    pub fn install_bus(
        &mut self,
        name: Option<KwId>,
        def: BusDef,
        signals: Vec<SignalInput>,
    ) -> GraphHandle {
        let id = def.id;
        self.bus_resources.remove(&id);
        let def = Arc::new(def);
        let entry = BusEntry {
            id,
            def: Arc::clone(&def),
            signals,
        };
        match name {
            Some(k) => {
                if !self.buses.contains_key(&k) {
                    self.next_bus += 1;
                }
                self.buses.insert(k, entry);
                GraphHandle::Bus { id, def }
            }
            None => {
                self.master = Some(entry);
                GraphHandle::Master(def)
            }
        }
    }

    /// Explicit opt-in used by a fresh isolated candidate before its declarations.
    pub fn enable_closed_song_resources(&mut self) {
        self.capture_mode = GraphResourceCaptureMode::ClosedSong;
    }
    #[must_use]
    pub fn resource_capture_mode(&self) -> GraphResourceCaptureMode {
        self.capture_mode
    }
    #[must_use]
    pub fn inst_resources(&self, id: InstId) -> Option<&InstResourceDeclaration> {
        self.inst_resources.get(&id).filter(|record| {
            self.entry(id)
                .is_some_and(|entry| Arc::ptr_eq(&entry.def, &record.graph))
        })
    }
    #[must_use]
    pub fn bus_resources(&self, id: BusId) -> Option<&BusResourceDeclaration> {
        self.bus_resources.get(&id).filter(|record| {
            self.buses()
                .any(|(_, entry)| entry.id == id && Arc::ptr_eq(&entry.def, &record.graph))
        })
    }
    pub(crate) fn install_with_resources(
        &mut self,
        entry: InstEntry,
        id: InstId,
        resources: Vec<DeclaredGraphResource>,
    ) {
        let graph = Arc::clone(&entry.def);
        self.install(entry, id);
        self.inst_resources
            .insert(id, InstResourceDeclaration { graph, resources });
    }
    pub(crate) fn install_bus_with_resources(
        &mut self,
        name: Option<KwId>,
        def: BusDef,
        signals: Vec<SignalInput>,
        resources: Vec<DeclaredGraphResource>,
    ) -> GraphHandle {
        let graph = self.install_bus(name, def, signals);
        let def = match &graph {
            GraphHandle::Bus { def, .. } | GraphHandle::Master(def) => Arc::clone(def),
            GraphHandle::Inst { .. } => unreachable!("bus commit"),
        };
        self.bus_resources.insert(
            def.id,
            BusResourceDeclaration {
                graph: def,
                resources,
            },
        );
        graph
    }

    /// Every installed instrument, bus and `master` graph, for a second
    /// audio host to install (the offline render, 14.5.9).
    #[must_use]
    pub fn graphs(&self) -> Vec<GraphHandle> {
        let insts = self.insts.iter().map(|(id, e)| GraphHandle::Inst {
            id: *id,
            def: Arc::clone(&e.def),
        });
        let buses = self.buses.values().map(|b| GraphHandle::Bus {
            id: b.id,
            def: Arc::clone(&b.def),
        });
        let master = self
            .master
            .iter()
            .map(|b| GraphHandle::Master(Arc::clone(&b.def)));
        insts.chain(buses).chain(master).collect()
    }

    /// The id a bus named `name` has or would get (`master` is 0).
    #[must_use]
    pub fn bus_id(&self, name: Option<KwId>) -> BusId {
        match name {
            None => BusId::new(0),
            Some(k) => self
                .buses
                .get(&k)
                .map_or(BusId::new(self.next_bus), |b| b.id),
        }
    }

    fn sampler(&self, src: SampleSrc) -> Result<Route, Failure> {
        match self.id_of(intern_kw("sampler")) {
            Some(inst) => Ok(Route::Audio {
                inst,
                sample: Some(src),
            }),
            None => Err(Failure::new(
                FailCode::UnknownSound,
                "no `sampler` instrument plays samples",
            )),
        }
    }

    /// The route of an installed instrument: its resource header default
    /// (R2b), when it has one, else no sample.
    fn audio_route(&self, id: InstId) -> Route {
        let sample = self
            .insts
            .get(&id)
            .and_then(|e| e.resource)
            .map(|kw| SampleSrc::Bank { kw, index: 0 });
        Route::Audio { inst: id, sample }
    }
}

impl InstResolver for InstRegistry {
    fn route(&self, sound: &Sound) -> Result<Route, Failure> {
        match sound {
            Sound::Inst(id) if self.insts.contains_key(id) => Ok(self.audio_route(*id)),
            Sound::Inst(id) => Err(Failure::new(
                FailCode::UnknownSound,
                format!("instrument #{} is not installed", id.get()),
            )),
            // A registered name plays that instrument; any other builtin
            // sound is a host bank, and every sample IS a sampler.
            Sound::Builtin(k) => match self.id_of(*k) {
                Some(inst) => Ok(self.audio_route(inst)),
                None => self.sampler(SampleSrc::Bank { kw: *k, index: 0 }),
            },
            Sound::Sample(p) => self.sampler(SampleSrc::Path(p.clone())),
            Sound::MidiOut(ch) => Ok(Route::Midi { ch: *ch }),
            Sound::Osc(addr) => Ok(Route::Osc {
                addr: Rc::clone(addr),
            }),
            // A captured or rendered buffer plays through the sampler; the
            // commit installs its frames under the buffer's id (14.5.9).
            Sound::Buffer(b) => self.sampler(SampleSrc::Buffer { id: b.id }),
        }
    }

    fn inst(&self, id: InstId) -> Option<Arc<InstDef>> {
        self.insts.get(&id).map(|e| Arc::clone(&e.def))
    }

    fn declared_param(&self, id: InstId, name: KwId) -> Option<DeclaredParam> {
        self.insts
            .get(&id)?
            .params
            .iter()
            .find(|p| p.name == name)
            .copied()
    }

    fn signal_inputs(&self) -> Vec<SignalInput> {
        let insts = self.insts.values().flat_map(|e| e.signals.iter());
        let buses = self.buses.values().chain(self.master.iter());
        insts
            .chain(buses.flat_map(|b| b.signals.iter()))
            .cloned()
            .collect()
    }

    /// The id of the bus declared as `name` (R3): a lookup only, never an
    /// allocation (`None` for an undeclared name).
    fn bus(&self, name: KwId) -> Option<BusId> {
        self.buses.get(&name).map(|b| b.id)
    }
}

impl InstResolver for Rc<RefCell<InstRegistry>> {
    fn route(&self, sound: &Sound) -> Result<Route, Failure> {
        self.borrow().route(sound)
    }

    fn inst(&self, id: InstId) -> Option<Arc<InstDef>> {
        self.borrow().inst(id)
    }

    fn declared_param(&self, id: InstId, name: KwId) -> Option<DeclaredParam> {
        self.borrow().declared_param(id, name)
    }

    fn signal_inputs(&self) -> Vec<SignalInput> {
        self.borrow().signal_inputs()
    }

    fn bus(&self, name: KwId) -> Option<BusId> {
        // Fully qualified: `InstRegistry` also has an inherent `bus` (the
        // full `&BusEntry` lookup other callers use), which would
        // otherwise shadow this trait method.
        InstResolver::bus(&*self.borrow(), name)
    }
}

fn inst_failed(name: KwId, why: &str, diag: Option<Box<Diagnostic>>) -> LowerError {
    inst_failure(name, FailCode::InstFailed, why, diag)
}

fn inst_failure(
    name: KwId,
    code: FailCode,
    why: &str,
    diag: Option<Box<Diagnostic>>,
) -> LowerError {
    LowerError {
        failure: Failure::new(code, format!("`{}` failed: {why}", name_of_kw(name))),
        diag,
    }
}

fn is_output_selection_failure(failure: &Failure) -> bool {
    failure.kind == FailureKind::OutputSelection
}

/// A `Param` node for a control.
#[must_use]
pub fn param_node(ctl: CtlId) -> Value {
    Value::UGen(Rc::new(UGenNode {
        kind: UGenKind::Ugen(UGenSpec::Param(ctl)),
        args: Box::new([]),
    }))
}

/// A header default as a control: a tweak site is a cell (13), anything
/// else a constant encoded through the control's row.
///
/// A tweak site that cannot get a cell (the instrument-default pool is
/// exhausted, DDRUM-002A) still installs, with that one default frozen at
/// its current value, but reports a `cell-capacity` diagnostic instead of
/// silently dropping the tweak site.
#[allow(clippy::too_many_arguments)]
fn default_ctl(
    reg: &mut InstRegistry,
    inst_name: KwId,
    param_name: KwId,
    span: Span,
    ctl: CtlId,
    default: Option<&Value>,
    cells: &mut Vec<(CellId, VarSlotRef)>,
    diags: &mut Vec<Diagnostic>,
) -> Ctl {
    let row = controls::row_by_id(ctl);
    let fallback = row.map_or(0.0, |r| r.default);
    match default {
        None | Some(Value::Nil) => Ctl::Const(fallback),
        Some(Value::VarRef(slot)) if slot.kind() == SlotKind::Tweak => match reg.alloc_cell() {
            Some(cell) => {
                cells.push((cell, slot.clone()));
                Ctl::Cell(cell)
            }
            None => {
                diags.push(Diagnostic::error(
                    crate::types::diag::DiagCode::CellCapacity,
                    span,
                    format!(
                        "`{}`'s `{}` default has no free instrument-default cell; it is frozen \
                         at its current value",
                        name_of_kw(inst_name),
                        name_of_kw(param_name)
                    ),
                ));
                Ctl::Const(number(&slot.get()).unwrap_or(fallback))
            }
        },
        Some(Value::VarRef(slot)) => Ctl::Const(number(&slot.get()).unwrap_or(fallback)),
        Some(v) => Ctl::Const(match (row, v) {
            (Some(r), _) if !matches!(r.domain, CtlDomain::Resource) => {
                controls::encode(r, v).unwrap_or(fallback)
            }
            (_, Value::Bool(b)) => f32::from(u8::from(*b)),
            _ => number(v).unwrap_or(fallback),
        }),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn number(v: &Value) -> Option<f32> {
    match v {
        Value::Int(i) => Some(*i as f32),
        Value::Int64(i) => Some(*i as f32),
        Value::Float(x) => Some(*x),
        Value::Float64(x) => Some(*x as f32),
        Value::Ratio(r) => Some(r.to_f64() as f32),
        _ => None,
    }
}

/// Realizes the `inst` definition bound in `slot` (its value is the body
/// closure): calls the body once, lowers, registers, rebinds the name to
/// `Sound::Inst(id)` and stages `Install`. With no registry the closure is
/// left as it is.
///
/// # Errors
/// `inst-failed` (with `graph-too-large` as a diagnostic when the graph is
/// over the cap); nothing is registered or staged then.
pub fn realize_inst(
    vm: &mut Vm,
    ns: &Namespace,
    slot: &VarSlotRef,
    span: Span,
) -> Result<Value, LowerError> {
    let Some(reg) = vm.dsp.registry.clone() else {
        return Ok(slot.get());
    };
    let name = intern_kw(&name_of_sym(slot.name()));
    let Value::Fn(c) = slot.get() else {
        return Err(inst_failed(
            name,
            "the definition is not an instrument body",
            None,
        ));
    };
    let fixed = usize::from(c.proto.arity.fixed);
    let ncap = usize::from(c.proto.captures);
    let mut header = Vec::new();
    let mut params = Vec::new();
    let mut cells = Vec::new();
    let mut default_diags: Vec<Diagnostic> = Vec::new();
    // R2b: the default keyword of a `CtlDomain::Resource` header parameter
    // (`bank`, `table` or `source`), if this instrument declares one.
    let mut resource: Option<KwId> = None;
    let mut resource_headers = Vec::new();
    let (mut args, mut kw) = (Vec::new(), Vec::new());
    for (k, pname) in c.proto.arity.names.iter().enumerate() {
        let mut r = reg.borrow_mut();
        let default = (k >= fixed)
            .then(|| c.captures.get(ncap + k - fixed))
            .flatten();
        // A list default (`partials`) is a realization-time constant of the
        // node, not a control (12.8.7).
        let v = match default {
            Some(list @ Value::List(_)) => list.clone(),
            _ => {
                if header.len() >= crate::dsp::ugen::MAX_PARAMS {
                    return Err(inst_failed(
                        name,
                        "the instrument declares too many parameters",
                        None,
                    ));
                }
                let ctl = r
                    .ctl_of(*pname)
                    .map_err(|e| inst_failed(name, &e.message, None))?;
                let ty = c
                    .proto
                    .arity
                    .scalar_types
                    .get(k)
                    .copied()
                    .unwrap_or(ScalarType::Float);
                let row = controls::row_by_id(ctl);
                let param = DeclaredParam {
                    name: *pname,
                    ctl,
                    ty,
                    range: row.map(|r| r.range),
                };
                if row.is_none() {
                    let v = match default {
                        Some(Value::VarRef(slot)) => slot.get(),
                        Some(v) => v.clone(),
                        None => Value::Nil,
                    };
                    if ty != ScalarType::Unsupported && !matches!(v, Value::Nil) {
                        param
                            .encode(&v)
                            .map_err(|e| inst_failed(name, &e.message, None))?;
                    }
                }
                if let (Some(row), Some(Value::Keyword(rkw))) = (controls::row_by_id(ctl), default)
                {
                    if row.domain == CtlDomain::Resource {
                        resource = Some(*rkw);
                        resource_headers.push(header_resource(ctl, GraphResourceInput::Bank(*rkw)));
                    }
                }
                header.push((
                    ctl,
                    default_ctl(
                        &mut r,
                        name,
                        *pname,
                        span,
                        ctl,
                        default,
                        &mut cells,
                        &mut default_diags,
                    ),
                ));
                params.push(param);
                param_node(ctl)
            }
        };
        if k < fixed {
            args.push(v);
        } else {
            kw.push((*pname, v));
        }
    }
    vm.dsp.diags.extend(default_diags);
    vm.dsp.inst += 1;
    let out = vm.call_value(ns, &Value::Fn(Rc::clone(&c)), args, kw);
    vm.dsp.inst -= 1;
    let root = match out {
        Ok(Value::UGen(n)) => n,
        Ok(other) => {
            let why = format!("the body is {}, not a unit generator", kind_name(&other));
            return Err(inst_failed(name, &why, None));
        }
        Err(f) => {
            let code = if is_output_selection_failure(&f) {
                f.code
            } else {
                FailCode::InstFailed
            };
            return Err(inst_failure(name, code, &f.message, None));
        }
    };
    let mut r = reg.borrow_mut();
    let id = r.peek_id(name);
    let caps = r.caps;
    let mode = r.resource_capture_mode();
    let mut alloc = || r.alloc_cell();
    let lw = Lowering {
        caps: &caps,
        span,
        alloc: &mut alloc,
    };
    let (def, mut extras) = lower_inst_with_resources(id, &root, &header, lw, mode)
        .map_err(|e| inst_failed(name, &e.failure.message, e.diag))?;
    if resource_headers.len() + extras.resources.len()
        > crate::dsp::graph::NODE_CAP + crate::dsp::ugen::MAX_PARAMS
    {
        return Err(inst_failed(
            name,
            "graph resource declarations exceed the graph bound",
            None,
        ));
    }
    resource_headers.append(&mut extras.resources);
    vm.dsp.diags.extend(extras.diags);
    let def = Arc::new(def);
    r.install_with_resources(
        InstEntry {
            name,
            def: Arc::clone(&def),
            params,
            cells,
            signals: extras.signals,
            resource,
        },
        id,
        resource_headers,
    );
    vm.effects_mut()
        .push(StagedEffect::Install(GraphHandle::Inst { id, def }));
    let v = Value::Sound(Rc::new(Sound::Inst(id)));
    slot.set(v.clone());
    Ok(v)
}

/// The slot an `inst` definition form defined, when `form` is one.
#[must_use]
pub fn inst_target(form: &Node, targets: &[VarSlotRef]) -> Option<VarSlotRef> {
    if crate::types::check::def_head(form) != Some("inst") {
        return None;
    }
    let name = form.children.get(1)?.sym_name()?;
    let [slot] = targets else {
        return None;
    };
    let is_body = matches!(slot.get(), Value::Fn(_));
    (is_body && &*name_of_sym(slot.name()) == name).then(|| slot.clone())
}

/// Realizes the prelude templates into the VM's registry and releases their
/// `Install` effects to `sink`. Each template runs in a scratch session over
/// `prelude` with tweak sites off (template defaults are constants).
pub fn load_templates(vm: &mut Vm, prelude: &Rc<Prelude>, sink: &mut dyn EffectSink) {
    let Some(reg) = vm.dsp.registry.clone() else {
        return;
    };
    let mut errors = Vec::new();
    match read_forms(TEMPLATES, TEMPLATE_FILE) {
        Err(d) => errors.push(format!("templates.vact: {d}")),
        Ok(forms) => {
            let scratch = Namespace::with_prelude(Rc::clone(prelude));
            for (k, form) in forms.iter().enumerate() {
                if let Err(e) = load_one(vm, &scratch, form, k) {
                    errors.push(e);
                    vm.effects_mut().drop_all();
                } else {
                    vm.effects_mut().release(sink);
                }
            }
        }
    }
    vm.dsp.diags.clear();
    reg.borrow_mut().template_errors.extend(errors);
}

fn load_one(vm: &mut Vm, ns: &Namespace, form: &Node, k: usize) -> Result<(), String> {
    let name = form
        .children
        .get(1)
        .and_then(Node::sym_name)
        .unwrap_or("?")
        .to_string();
    let mut cx = CompileCx::new(ns, FormGen::new(u64::try_from(k).unwrap_or(0) + 1));
    cx.tweak_sites = false;
    let proto = compile(form, &mut cx).map_err(|d| format!("{name}: {d}"))?;
    vm.run(proto, ns).map_err(|f| format!("{name}: {f}"))?;
    let slot = ns
        .session_slot(intern_sym(&name))
        .ok_or_else(|| format!("{name}: not defined"))?;
    realize_inst(vm, ns, &slot, form.span)
        .map(|_| ())
        .map_err(|e| format!("{name}: {}", e.failure.message))
}
