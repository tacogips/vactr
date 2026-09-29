//! Resource stores and the browser install protocol (design 12.2, 16.1).
//!
//! `SampleStore` holds every installed sample, wavetable and impulse
//! response under a resource id. The native tier keeps the loader's
//! `Arc<SampleData>` (handed over, never copied, and handed back for
//! dropping on the evaluator thread); the browser tier copies slices into
//! one fixed-capacity arena with a first-fit free list. Capacities are
//! fixed at construction and never change.
//!
//! Retirement is uniform: `retire` marks a resource `Retiring`; the engine
//! releases it (and acks `Retired`) only when no queued event, voice or
//! template references it any more.
//!
//! The graph byte codec (`encode_inst`, `encode_bus`, `decode_graph`)
//! carries `InstDef`/`BusDef` into the worklet; a template stays within one
//! 64 KB slice because of `NODE_CAP`, and larger bytes are
//! `graph-too-large`.

use std::sync::Arc;

use crate::dsp::bus::{BusTemplate, SlotState, MAX_CHAIN};
use crate::dsp::caps::{Cap, CapabilitySet};
use crate::dsp::cells::CellId;
use crate::dsp::effects::MAX_FX_PARAMS;
use crate::dsp::graph::{BusDef, BusId, Edge, EffectKind, InstDef, InstId};
use crate::dsp::ugen::catalog::{effect_index, get_node, put_spec, In, Out};
use crate::dsp::ugen::{BuildEnv, BuildError, RawGraph, Template};
use crate::dsp::voice::Voice;
use crate::host::caps::SampleData;
use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Diagnostic};

/// Install copy budget per `process()` call (16.1).
pub const INSTALL_BYTES_PER_QUANTUM: usize = 65_536;
/// The largest slice payload and the largest graph encoding (16.1).
pub const SLICE_BYTES: usize = 65_536;
/// The default browser arena (16.1).
pub const DEFAULT_ARENA_BYTES: usize = 64 << 20;
/// Resource ids live at once (installing, live or retiring).
pub const MAX_RESOURCES: usize = 256;
/// Free-list extents.
pub const MAX_EXTENTS: usize = 256;
/// Install records that may wait for credit before `install-queue-overflow`.
pub const DEFERRED_MAX: usize = 8;

/// Where resources live.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoreKind {
    /// Native: loader `Arc`s handed over.
    NativeArc,
    /// Browser: a preallocated arena of `bytes`.
    Arena { bytes: usize },
}

/// One installed sample as the audio side reads it.
#[derive(Clone, Copy, Debug)]
pub struct SampleView<'a> {
    /// Interleaved frames.
    pub data: &'a [f32],
    pub channels: u8,
    pub rate: u32,
}

impl SampleView<'_> {
    /// The number of frames.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.data.len() / usize::from(self.channels.max(1))
    }
}

/// A resource's lifecycle state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResState {
    Installing,
    Live,
    Retiring,
}

/// Why an install record failed on the audio side (the host reports the
/// matching diagnostic).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaultCode {
    ArenaExhausted,
    InstallQueueOverflow,
    GraphTooLarge,
    /// A slice, retire or graph for an unknown or duplicate id.
    BadResource,
    /// Undecodable install bytes.
    BadRecord,
    /// The graph needs four host output channels but this engine has two.
    OutputChannels,
    /// External audio input is short, malformed, or nonfinite.
    InputBuffer,
}

impl FaultCode {
    /// The diagnostic code the host reports.
    #[must_use]
    pub const fn diag(self) -> DiagCode {
        match self {
            FaultCode::ArenaExhausted => DiagCode::ArenaExhausted,
            FaultCode::InstallQueueOverflow => DiagCode::InstallQueueOverflow,
            FaultCode::GraphTooLarge => DiagCode::GraphTooLarge,
            FaultCode::BadResource | FaultCode::BadRecord | FaultCode::InputBuffer => {
                DiagCode::HostTransport
            }
            FaultCode::OutputChannels => DiagCode::BeyondCapability,
        }
    }
}

/// A failed install, drained from the engine by the host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fault {
    pub code: FaultCode,
    pub resource: u32,
}

/// The kind of an install request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResourceKind {
    Sample,
    Table,
    Ir,
    Graph,
}

/// An install as the evaluator asks for it; checked by `admit` before any
/// byte is sent (16.1 admission).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InstallRequest {
    pub resource: u32,
    pub kind: ResourceKind,
    pub frames: usize,
    pub channels: u8,
    pub rate: u32,
    /// Encoded graph bytes (`Graph` only).
    pub bytes: usize,
    /// Where the install came from, carried into any diagnostic.
    pub origin: Option<Span>,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    id: u32,
    gen: u32,
    state: ResState,
    channels: u8,
    rate: u32,
    off: usize,
    len: usize,
    received: usize,
}

/// The installed samples, tables and impulse responses.
#[derive(Debug)]
pub struct SampleStore {
    kind: StoreKind,
    entries: Box<[Option<Entry>]>,
    native: Box<[Option<Arc<SampleData>>]>,
    arena: Box<[f32]>,
    free: Box<[(usize, usize)]>,
    n_free: usize,
}

impl Default for SampleStore {
    fn default() -> Self {
        Self::new(StoreKind::NativeArc)
    }
}

impl SampleStore {
    /// A store; the browser arena is allocated here, once.
    #[must_use]
    pub fn new(kind: StoreKind) -> Self {
        let floats = match kind {
            StoreKind::NativeArc => 0,
            StoreKind::Arena { bytes } => bytes / 4,
        };
        let mut free = vec![(0, 0); MAX_EXTENTS].into_boxed_slice();
        let n_free = usize::from(floats > 0);
        free[0] = (0, floats);
        Self {
            kind,
            entries: vec![None; MAX_RESOURCES].into_boxed_slice(),
            native: (0..MAX_RESOURCES).map(|_| None).collect(),
            arena: vec![0.0; floats].into_boxed_slice(),
            free,
            n_free,
        }
    }

    /// The store kind.
    #[must_use]
    pub fn kind(&self) -> StoreKind {
        self.kind
    }

    /// The arena capacity in bytes (0 natively); fixed after `new`.
    #[must_use]
    pub fn capacity_bytes(&self) -> usize {
        self.arena.len() * 4
    }

    /// The arena's base address, for the capacity-never-changes proof.
    #[must_use]
    pub fn arena_ptr(&self) -> *const f32 {
        self.arena.as_ptr()
    }

    /// Free arena floats (the largest extent is what admission checks).
    #[must_use]
    pub fn free_floats(&self) -> usize {
        self.free[..self.n_free].iter().map(|e| e.1).sum()
    }

    fn largest_extent(&self) -> usize {
        self.free[..self.n_free]
            .iter()
            .map(|e| e.1)
            .max()
            .unwrap_or(0)
    }

    fn slot(&self, id: u32) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.is_some_and(|e| e.id == id))
    }

    /// A resource's state.
    #[must_use]
    pub fn state(&self, id: u32) -> Option<ResState> {
        self.slot(id).and_then(|i| self.entries[i].map(|e| e.state))
    }

    /// A live (or retiring, still referenced) resource.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<SampleView<'_>> {
        let i = self.slot(id)?;
        let e = self.entries[i]?;
        if e.state == ResState::Installing {
            return None;
        }
        let data: &[f32] = match self.kind {
            StoreKind::NativeArc => &self.native[i].as_ref()?.frames,
            StoreKind::Arena { .. } => self.arena.get(e.off..e.off + e.len)?,
        };
        Some(SampleView {
            data,
            channels: e.channels,
            rate: e.rate,
        })
    }

    /// Checks a request against the tier and the free space (16.1
    /// admission); the diagnostic carries the request's origin.
    ///
    /// # Errors
    /// `beyond-capability` for an impulse response over `max_ir_seconds`,
    /// `arena-exhausted` when no extent holds the data, `graph-too-large`
    /// for graph bytes over one slice.
    pub fn admit(&self, req: &InstallRequest, caps: &CapabilitySet) -> Result<(), Diagnostic> {
        let span = req.origin.unwrap_or(Span::new(FileId::new(0), 0, 0));
        match req.kind {
            ResourceKind::Graph => {
                if req.bytes > SLICE_BYTES {
                    return Err(Diagnostic::error(
                        DiagCode::GraphTooLarge,
                        span,
                        format!("graph encoding of {} bytes exceeds one slice", req.bytes),
                    ));
                }
                return Ok(());
            }
            ResourceKind::Ir => {
                #[allow(clippy::cast_precision_loss)]
                let secs = req.frames as f32 / req.rate.max(1) as f32;
                caps.require(Cap::IrSeconds(secs), req.origin)?;
            }
            ResourceKind::Sample | ResourceKind::Table => {}
        }
        if let StoreKind::Arena { .. } = self.kind {
            let need = req.frames * usize::from(req.channels.max(1));
            if need > self.largest_extent() {
                return Err(Diagnostic::error(
                    DiagCode::ArenaExhausted,
                    span,
                    format!(
                        "sample of {} bytes does not fit the free arena ({} bytes free)",
                        need * 4,
                        self.free_floats() * 4
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Starts an arena install: reserves the extent (browser tier).
    ///
    /// # Errors
    /// `ArenaExhausted` or `BadResource` (id in use, table full).
    pub fn begin(
        &mut self,
        id: u32,
        gen: u32,
        frames: usize,
        channels: u8,
        rate: u32,
    ) -> Result<(), FaultCode> {
        if self.slot(id).is_some() {
            return Err(FaultCode::BadResource);
        }
        let i = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(FaultCode::BadResource)?;
        let len = frames * usize::from(channels.max(1));
        let off = self.alloc(len).ok_or(FaultCode::ArenaExhausted)?;
        self.entries[i] = Some(Entry {
            id,
            gen,
            state: ResState::Installing,
            channels: channels.max(1),
            rate,
            off,
            len,
            received: 0,
        });
        Ok(())
    }

    /// Copies one slice of little-endian `f32` samples at float `offset`.
    /// Returns `Some(gen)` when the resource just completed.
    ///
    /// # Errors
    /// `BadResource` for an unknown id or a slice outside the resource.
    pub fn write_slice(
        &mut self,
        id: u32,
        offset: usize,
        bytes: &[u8],
    ) -> Result<Option<u32>, FaultCode> {
        let i = self.slot(id).ok_or(FaultCode::BadResource)?;
        let mut e = self.entries[i].ok_or(FaultCode::BadResource)?;
        let n = bytes.len() / 4;
        if e.state != ResState::Installing || offset + n > e.len {
            return Err(FaultCode::BadResource);
        }
        let dst = self
            .arena
            .get_mut(e.off + offset..e.off + offset + n)
            .ok_or(FaultCode::BadResource)?;
        for (d, b) in dst.iter_mut().zip(bytes.chunks_exact(4)) {
            *d = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        e.received += n;
        let done = e.received >= e.len;
        if done {
            e.state = ResState::Live;
        }
        self.entries[i] = Some(e);
        Ok(done.then_some(e.gen))
    }

    /// Installs a loader `Arc` (native tier); the `Arc` comes back on
    /// failure.
    ///
    /// # Errors
    /// The `Arc` when the id is in use or the table is full.
    pub fn install_arc(
        &mut self,
        id: u32,
        gen: u32,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        if self.slot(id).is_some() {
            return Err(data);
        }
        let Some(i) = self.entries.iter().position(Option::is_none) else {
            return Err(data);
        };
        self.entries[i] = Some(Entry {
            id,
            gen,
            state: ResState::Live,
            channels: data.channels.max(1),
            rate: data.rate,
            off: 0,
            len: data.frames.len(),
            received: data.frames.len(),
        });
        self.native[i] = Some(data);
        Ok(())
    }

    /// Marks a live resource retiring; returns whether it was live or
    /// installing.
    pub fn retire(&mut self, id: u32) -> bool {
        let Some(i) = self.slot(id) else {
            return false;
        };
        match &mut self.entries[i] {
            Some(e) if e.state != ResState::Retiring => {
                e.state = ResState::Retiring;
                true
            }
            _ => false,
        }
    }

    /// The retiring resource ids (the engine checks their references).
    pub fn retiring(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries
            .iter()
            .flatten()
            .filter(|e| e.state == ResState::Retiring)
            .map(|e| e.id)
    }

    /// Frees a retiring resource: the arena extent returns to the free list
    /// and a native `Arc` is handed back for dropping off the audio thread.
    /// `None` when `id` is not retiring.
    pub fn release(&mut self, id: u32) -> Option<Option<Arc<SampleData>>> {
        let i = self.slot(id)?;
        let e = self.entries[i]?;
        if e.state != ResState::Retiring {
            return None;
        }
        self.entries[i] = None;
        if let StoreKind::Arena { .. } = self.kind {
            self.dealloc(e.off, e.len);
        }
        Some(self.native[i].take())
    }

    /// First-fit allocation of `len` floats.
    fn alloc(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            return Some(0);
        }
        let k = self.free[..self.n_free].iter().position(|e| e.1 >= len)?;
        let (off, have) = self.free[k];
        if have == len {
            self.free.copy_within(k + 1..self.n_free, k);
            self.n_free -= 1;
        } else {
            self.free[k] = (off + len, have - len);
        }
        Some(off)
    }

    /// Returns an extent to the free list, coalescing neighbours. When the
    /// list is full the extent is dropped (capacity stays bounded; the
    /// space is leaked until neighbours coalesce).
    fn dealloc(&mut self, off: usize, len: usize) {
        if len == 0 {
            return;
        }
        let k = self.free[..self.n_free]
            .iter()
            .position(|e| e.0 > off)
            .unwrap_or(self.n_free);
        let merge_prev = k > 0 && self.free[k - 1].0 + self.free[k - 1].1 == off;
        let merge_next = k < self.n_free && off + len == self.free[k].0;
        match (merge_prev, merge_next) {
            (true, true) => {
                self.free[k - 1].1 += len + self.free[k].1;
                self.free.copy_within(k + 1..self.n_free, k);
                self.n_free -= 1;
            }
            (true, false) => self.free[k - 1].1 += len,
            (false, true) => self.free[k] = (off, len + self.free[k].1),
            (false, false) => {
                if self.n_free < MAX_EXTENTS {
                    self.free.copy_within(k..self.n_free, k + 1);
                    self.free[k] = (off, len);
                    self.n_free += 1;
                }
            }
        }
    }
}

// ---- graph byte codec ------------------------------------------------------

const G_INST: u8 = b'I';
const G_BUS: u8 = b'B';
const G_MASTER: u8 = b'M';

/// A decoded graph encoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphKind {
    Inst,
    Bus(BusId),
    Master,
}

/// Encodes an instrument template (evaluator side).
///
/// # Errors
/// A capacity `BuildError`.
pub fn encode_inst(def: &InstDef, out: &mut Vec<u8>) -> Result<(), BuildError> {
    out.clear();
    let mut o = Out(out);
    o.u8(G_INST);
    o.u32(def.id.get());
    o.len16(def.params.len())?;
    for &(id, c) in def.params.iter() {
        o.ctl(id, c);
    }
    o.len16(def.nodes.len())?;
    for spec in def.nodes.iter() {
        put_spec(&mut o, spec)?;
    }
    o.len16(def.edges.len())?;
    for e in def.edges.iter() {
        o.u16(e.from);
        o.u16(e.to);
        o.u8(e.port);
    }
    o.len16(def.node_params.len())?;
    for &(n, id, c) in def.node_params.iter() {
        o.u16(n);
        o.ctl(id, c);
    }
    Ok(())
}

/// Encodes a bus (or `master` when `master`) chain.
///
/// # Errors
/// `TooManyEffects` for a chain over `MAX_CHAIN`.
pub fn encode_bus(def: &BusDef, master: bool, out: &mut Vec<u8>) -> Result<(), BuildError> {
    out.clear();
    if def.chain.len() > MAX_CHAIN {
        return Err(BuildError::TooManyEffects);
    }
    let mut o = Out(out);
    o.u8(if master { G_MASTER } else { G_BUS });
    o.u32(def.id.get());
    o.len16(def.chain.len())?;
    for e in def.chain.iter() {
        o.u8(effect_index(e.kind));
        o.len16(e.params.len())?;
        for &(id, c) in e.params.iter() {
            o.ctl(id, c);
        }
    }
    Ok(())
}

/// Decodes graph bytes without allocating: an instrument into `raw`, a bus
/// or master chain into `bus`.
///
/// # Errors
/// `GraphTooLarge` over a capacity, `BadRecord` for malformed bytes.
pub fn decode_graph(
    bytes: &[u8],
    raw: &mut RawGraph,
    bus: &mut BusTemplate,
) -> Result<GraphKind, FaultCode> {
    if bytes.len() > SLICE_BYTES {
        return Err(FaultCode::GraphTooLarge);
    }
    let mut i = In { b: bytes, pos: 0 };
    let tag = i.u8()?;
    let id = i.u32()?;
    match tag {
        G_INST => {
            raw.clear();
            raw.inst = InstId::new(id);
            for _ in 0..i.u16()? {
                let (id, c) = i.ctl()?;
                raw.push_param(id, c)
                    .map_err(|_| FaultCode::GraphTooLarge)?;
            }
            for _ in 0..i.u16()? {
                get_node(&mut i, raw)?;
            }
            for _ in 0..i.u16()? {
                let e = Edge {
                    from: i.u16()?,
                    to: i.u16()?,
                    port: i.u8()?,

                    output: 0,
                };
                raw.push_edge(e).map_err(|_| FaultCode::GraphTooLarge)?;
            }
            for _ in 0..i.u16()? {
                let n = i.u16()?;
                let (id, c) = i.ctl()?;
                raw.push_node_param(n, id, c)
                    .map_err(|_| FaultCode::GraphTooLarge)?;
            }
            Ok(GraphKind::Inst)
        }
        G_BUS | G_MASTER => {
            bus.clear();
            bus.bus = BusId::new(id);
            let n = usize::from(i.u16()?);
            if n > MAX_CHAIN {
                return Err(FaultCode::GraphTooLarge);
            }
            for _ in 0..n {
                let kind = *EffectKind::ALL
                    .get(usize::from(i.u8()?))
                    .ok_or(FaultCode::BadRecord)?;
                let k = bus.push(kind).ok_or(FaultCode::GraphTooLarge)?;
                for _ in 0..i.u16()? {
                    let (id, c) = i.ctl()?;
                    if !bus.push_param(k, id, c) && MAX_FX_PARAMS > 0 {
                        return Err(FaultCode::GraphTooLarge);
                    }
                }
            }
            Ok(if tag == G_MASTER {
                GraphKind::Master
            } else {
                GraphKind::Bus(BusId::new(id))
            })
        }
        _ => Err(FaultCode::BadRecord),
    }
}

// ---- graph template slots (16.1) -------------------------------------------

/// One instrument template slot.
pub struct TemplateSlot {
    pub state: SlotState,
    pub t: Option<Box<Template>>,
}

/// The instrument template slots: on the arena tier every slot holds a
/// preallocated template compiled in place from install bytes; natively a
/// slot adopts the box the evaluator built and gives it back on retirement.
pub struct Templates {
    pub slots: Box<[TemplateSlot]>,
    preallocated: bool,
}

impl Templates {
    /// `n` slots; `preallocate` fills each with an empty template.
    #[must_use]
    pub fn new(n: usize, preallocate: bool) -> Self {
        Self {
            slots: (0..n.max(1))
                .map(|_| TemplateSlot {
                    state: SlotState::Free,
                    t: preallocate.then(Template::boxed),
                })
                .collect(),
            preallocated: preallocate,
        }
    }

    /// The live slot of an instrument.
    #[must_use]
    pub fn live(&self, inst: InstId) -> Option<usize> {
        self.slots.iter().position(|s| {
            s.state == SlotState::Live && s.t.as_ref().is_some_and(|t| t.inst == inst)
        })
    }

    /// The template in slot `i`.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&Template> {
        self.slots.get(i).and_then(|s| s.t.as_deref())
    }

    /// Marks `slot` live; the previous live template of its instrument
    /// retires.
    fn activate(&mut self, slot: usize, inst: InstId) {
        for (i, s) in self.slots.iter_mut().enumerate() {
            if i != slot
                && s.state == SlotState::Live
                && s.t.as_ref().is_some_and(|t| t.inst == inst)
            {
                s.state = SlotState::Retiring;
            }
        }
        self.slots[slot].state = SlotState::Live;
    }

    /// Compiles `raw` into a free preallocated slot (arena tier).
    ///
    /// # Errors
    /// `BadResource` with no free slot, `GraphTooLarge` when it does not
    /// compile.
    pub fn build(
        &mut self,
        raw: &RawGraph,
        env: &BuildEnv,
        resource: u32,
        gen: u32,
    ) -> Result<(), FaultCode> {
        let slot = self
            .slots
            .iter()
            .position(|s| s.state == SlotState::Free && s.t.is_some())
            .ok_or(FaultCode::BadResource)?;
        let t = self.slots[slot].t.as_mut().ok_or(FaultCode::BadResource)?;
        t.build(raw, env).map_err(|_| FaultCode::GraphTooLarge)?;
        t.resource = resource;
        t.gen = gen;
        let inst = t.inst;
        self.activate(slot, inst);
        Ok(())
    }

    /// Adopts a template built on the evaluator thread (native tier).
    ///
    /// # Errors
    /// The box back when no slot is free.
    pub fn adopt(
        &mut self,
        mut t: Box<Template>,
        resource: u32,
        gen: u32,
    ) -> Result<(), Box<Template>> {
        let Some(slot) = self
            .slots
            .iter()
            .position(|s| s.state == SlotState::Free && s.t.is_none())
        else {
            return Err(t);
        };
        t.resource = resource;
        t.gen = gen;
        let inst = t.inst;
        self.slots[slot].t = Some(t);
        self.activate(slot, inst);
        Ok(())
    }

    /// `GraphRetire`: the live template of `resource` starts retiring.
    pub fn retire(&mut self, resource: u32) {
        for s in self.slots.iter_mut() {
            if s.state == SlotState::Live && s.t.as_ref().is_some_and(|t| t.resource == resource) {
                s.state = SlotState::Retiring;
            }
        }
    }

    /// True when an installed template reads resource `id`.
    #[must_use]
    pub fn references(&self, id: u32) -> bool {
        self.slots.iter().any(|s| {
            s.state != SlotState::Free && s.t.as_ref().is_some_and(|t| t.reads_resource(id))
        })
    }

    /// True when a live or retiring template reads `cell`: as a control
    /// default, a node-port input or an effect-node parameter.
    #[must_use]
    pub fn reads_cell(&self, cell: CellId) -> bool {
        self.slots
            .iter()
            .any(|s| s.state != SlotState::Free && s.t.as_ref().is_some_and(|t| t.reads_cell(cell)))
    }

    /// Frees one retiring slot that no voice plays; returns its resource
    /// and, natively, the box to drop off the audio thread.
    pub fn collect(&mut self, voices: &[Voice]) -> Option<(u32, Option<Box<Template>>)> {
        let i = (0..self.slots.len()).find(|&i| {
            self.slots[i].state == SlotState::Retiring
                && !voices.iter().any(|v| v.active && v.tmpl == i)
        })?;
        let s = &mut self.slots[i];
        s.state = SlotState::Free;
        let resource = s.t.as_ref().map_or(0, |t| t.resource);
        let boxed = if self.preallocated { None } else { s.t.take() };
        Some((resource, boxed))
    }
}
