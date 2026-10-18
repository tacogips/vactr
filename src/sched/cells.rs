//! Authoritative control cells (design 11.3 "Value changes go through
//! control cells", 12.8.5).
//!
//! Commit turns a late-bound control into `Ctl::Cell(id)`; the evaluator
//! owns the authoritative value of every cell here, and the audio side
//! reads its copy at voice start. The transport is per tier:
//!
//! - native: `AtomicCells` shared with the callback; a write is a release
//!   store, visible to the next voice start. No cell message is sent.
//! - browser: the worklet's isolated `Mirror`, kept by `CellInit` (once per
//!   incarnation, ack-gated), per-tick `CellBatch`es coalesced latest-wins
//!   (at most one in flight plus one pending), `CellRetire` with
//!   epoch-gated id reuse, and a full-snapshot `resync`. Until an
//!   incarnation is acknowledged, commit downgrades it to `Ctl::Const`
//!   with the current value (the documented Const-fallback exception).
//!
//! `AudioHost::post` carries one fixed-size record, and a `CellBatch`'s
//! entries follow its header in the byte stream, so the browser tier sends
//! cell records through a `CellPort` (12.8.5 "only the transport differs").

use std::collections::BTreeMap;

use crate::dsp::cells::{AtomicCells, CellId};
use crate::dsp::controls::{ControlRow, CtlDomain};
use crate::host::caps::AudioHost;
use crate::host::wire::{Ctl, CtlMsg, HostMsg};
use crate::ns::namespace::VarSlotRef;
use crate::reader::span::{FileId, Span};
use crate::sched::commit::encode_value;
use crate::sched::slots::CtlId;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::value::Value;

/// The browser tier's cell channel to the worklet.
pub trait CellPort {
    /// Sends `CellInit` or `CellRetire`.
    fn post(&mut self, msg: CtlMsg);
    /// Sends one `CellBatch` with its entries.
    fn post_batch(&mut self, seq: u32, entries: &[(CellId, u32, f32)]);
    /// Moves the cell acknowledgments that arrived into `out`.
    fn drain(&mut self, out: &mut Vec<HostMsg>);
}

/// The cell transport of a host tier.
pub enum Tier {
    Native(AtomicCells),
    Browser(Box<dyn CellPort>),
}

impl std::fmt::Debug for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier::Native(_) => f.write_str("Native"),
            Tier::Browser(_) => f.write_str("Browser"),
        }
    }
}

/// What a cell carries.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CellKey {
    /// A control of events whose value came from a var or tweak slot.
    Site { slot: u64, ctl: CtlId },
    /// A cell an instrument owns (a signal input, a cell-backed default).
    External(CellId),
}

/// How a source value becomes the cell's `f32`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellMap {
    /// Encoded through the control row.
    Direct,
    /// A note number (or note name) as a frequency in Hz (`note`/`n` ->
    /// `freq`).
    NoteToFreq,
}

#[derive(Debug)]
struct Entry {
    cell: CellId,
    epoch: u32,
    value: f32,
    acked: bool,
    init_ticks: u32,
    init_reported: bool,
    row: Option<&'static ControlRow>,
    map: CellMap,
    momentary: Option<f32>,
}

#[derive(Debug)]
struct InFlight {
    seq: u32,
    entries: Vec<(CellId, u32, f32)>,
    ticks: u32,
    reported: bool,
}

/// Per-tier counters the tests read.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct CellStats {
    pub batches_sent: u32,
    pub resends: u32,
    pub inits_sent: u32,
}

/// The authoritative cell table.
#[derive(Debug)]
pub struct ControlCells {
    tier: Tier,
    entries: BTreeMap<CellKey, Entry>,
    free: Vec<CellId>,
    next: u32,
    pool: u32,
    epochs: BTreeMap<CellId, u32>,
    retiring: BTreeMap<CellId, u32>,
    pending: BTreeMap<CellId, (u32, f32)>,
    in_flight: Option<InFlight>,
    seq: u32,
    resync_seq: Option<u32>,
    exhausted_reported: bool,
    resend_ticks: u32,
    diag_ticks: u32,
    stats: CellStats,
}

fn nowhere() -> Span {
    Span::new(FileId::new(0), 0, 0)
}

impl ControlCells {
    /// A table on `tier` whose runtime cells take ids `0..pool`, below the
    /// instrument registry's range (`INST_CELL_BASE..`, BE-INST): the
    /// audio-side table (`pool` ids) holds both.
    #[must_use]
    pub fn new(tier: Tier, pool: usize, resend_ticks: u32, diag_ticks: u32) -> Self {
        Self {
            tier,
            entries: BTreeMap::new(),
            free: Vec::new(),
            next: 0,
            pool: u32::try_from(pool)
                .unwrap_or(u32::MAX)
                .min(crate::ns::insts::INST_CELL_BASE),
            epochs: BTreeMap::new(),
            retiring: BTreeMap::new(),
            pending: BTreeMap::new(),
            in_flight: None,
            seq: 0,
            resync_seq: None,
            exhausted_reported: false,
            resend_ticks: resend_ticks.max(1),
            diag_ticks: diag_ticks.max(1),
            stats: CellStats::default(),
        }
    }

    /// True on the browser tier.
    #[must_use]
    pub fn is_browser(&self) -> bool {
        matches!(self.tier, Tier::Browser(_))
    }

    /// The control for a late-bound value at commit: `Ctl::Cell` once the
    /// incarnation is usable, else `Ctl::Const` with the current value (the
    /// pre-ack downgrade, and the pool-exhaustion fallback, which reports
    /// one `beyond-capability`). A first reference allocates the cell.
    pub fn ctl_for(
        &mut self,
        key: CellKey,
        row: Option<&'static ControlRow>,
        map: CellMap,
        value: f32,
        diags: &mut Vec<Diagnostic>,
    ) -> Ctl {
        if !self.entries.contains_key(&key) {
            let cell = match key {
                CellKey::External(c) => Some(c),
                CellKey::Site { .. } => self.alloc(),
            };
            let Some(cell) = cell else {
                if !self.exhausted_reported {
                    self.exhausted_reported = true;
                    diags.push(Diagnostic::error(
                        DiagCode::BeyondCapability,
                        nowhere(),
                        format!(
                            "control cell pool exhausted ({} cells): late-bound controls \
                             commit as constants",
                            self.pool
                        ),
                    ));
                }
                return Ctl::Const(value);
            };
            self.install(key, cell, row, map, value);
        }
        let usable = !self.is_browser() || self.resync_seq.is_none();
        let Some(e) = self.entries.get_mut(&key) else {
            return Ctl::Const(value);
        };
        if e.value.to_bits() != value.to_bits() {
            e.value = value;
            let (cell, epoch) = (e.cell, e.epoch);
            self.publish(cell, epoch, value);
        }
        let Some(e) = self.entries.get(&key) else {
            return Ctl::Const(value);
        };
        if e.acked && usable {
            Ctl::Cell(e.cell)
        } else {
            Ctl::Const(e.momentary.unwrap_or(e.value))
        }
    }

    fn alloc(&mut self) -> Option<CellId> {
        if let Some(c) = self.free.pop() {
            return Some(c);
        }
        if self.next < self.pool {
            self.next += 1;
            return Some(CellId::new(self.next - 1));
        }
        None
    }

    fn install(
        &mut self,
        key: CellKey,
        cell: CellId,
        row: Option<&'static ControlRow>,
        map: CellMap,
        value: f32,
    ) {
        let epoch = self.epochs.get(&cell).copied().unwrap_or(0) + 1;
        self.epochs.insert(cell, epoch);
        let acked = match &mut self.tier {
            Tier::Native(cells) => {
                cells.set(cell, value);
                true
            }
            Tier::Browser(port) => {
                port.post(CtlMsg::CellInit { cell, epoch, value });
                self.stats.inits_sent += 1;
                false
            }
        };
        self.entries.insert(
            key,
            Entry {
                cell,
                epoch,
                value,
                acked,
                init_ticks: 0,
                init_reported: false,
                row,
                map,
                momentary: None,
            },
        );
    }

    /// Makes a new value visible: a release store natively, the next batch
    /// in the browser.
    fn publish(&mut self, cell: CellId, epoch: u32, value: f32) {
        match &self.tier {
            Tier::Native(cells) => {
                cells.set(cell, value);
            }
            Tier::Browser(_) => {
                self.pending.insert(cell, (epoch, value));
            }
        }
    }

    /// A var or tweak slot changed: every cell it feeds takes the new value
    /// (encoded per control; a value outside a control's domain leaves that
    /// cell unchanged).
    pub fn write_slot(&mut self, slot: &VarSlotRef) {
        let id = slot.id();
        let value = slot.base();
        let updates: Vec<(CellId, u32, f32, CellKey)> = self
            .entries
            .iter()
            .filter(|(k, _)| matches!(k, CellKey::Site { slot, .. } if *slot == id))
            .filter_map(|(k, e)| {
                let row = e.row?;
                let v = encode_value(row, e.map, &value)?;
                Some((e.cell, e.epoch, v, *k))
            })
            .collect();
        for (cell, epoch, v, k) in updates {
            if let Some(e) = self.entries.get_mut(&k) {
                e.value = v;
            }
            self.publish(cell, epoch, v);
        }
    }

    /// Writes an instrument-owned cell (a sampled signal input).
    pub fn write_external(&mut self, cell: CellId, value: f32) {
        let key = CellKey::External(cell);
        if !self.entries.contains_key(&key) {
            self.install(key, cell, None, CellMap::Direct, value);
            return;
        }
        if let Some(e) = self.entries.get_mut(&key) {
            if e.value.to_bits() == value.to_bits() {
                return;
            }
            e.value = value;
            let epoch = e.epoch;
            self.publish(cell, epoch, value);
        }
    }

    /// Initializes an instrument-owned cell once (a cell-backed `InstDef`
    /// default at install, 11.3): natively a store, in the browser a
    /// `CellInit` that later commits are ack-gated on.
    pub fn ensure_external(&mut self, cell: CellId, value: f32) {
        let key = CellKey::External(cell);
        if !self.entries.contains_key(&key) {
            self.install(key, cell, None, CellMap::Direct, value);
        }
    }

    /// Retires every cell fed by a slot whose owner went away; the id is
    /// reused only after the retire is acknowledged (browser) and always
    /// with a greater epoch.
    pub fn retire_slot(&mut self, slot: u64) {
        let keys: Vec<CellKey> = self
            .entries
            .keys()
            .filter(|k| matches!(k, CellKey::Site { slot: s, .. } if *s == slot))
            .copied()
            .collect();
        for k in keys {
            let Some(e) = self.entries.remove(&k) else {
                continue;
            };
            self.pending.remove(&e.cell);
            match &mut self.tier {
                Tier::Native(_) => self.free.push(e.cell),
                Tier::Browser(port) => {
                    port.post(CtlMsg::CellRetire {
                        cell: e.cell,
                        epoch: e.epoch,
                    });
                    self.retiring.insert(e.cell, e.epoch);
                }
            }
        }
    }

    /// Applies one acknowledgment from the audio side.
    pub fn on_msg(&mut self, msg: &HostMsg) {
        match *msg {
            HostMsg::CellInitAck { cell, epoch } => {
                for e in self.entries.values_mut() {
                    if e.cell == cell && e.epoch == epoch {
                        e.acked = true;
                    }
                }
            }
            HostMsg::CellBatchAck { seq } => {
                if self.in_flight.as_ref().is_some_and(|f| f.seq == seq) {
                    self.in_flight = None;
                }
                if self.resync_seq == Some(seq) {
                    self.resync_seq = None;
                }
            }
            HostMsg::CellRetired { cell, epoch } => {
                if self.retiring.get(&cell) == Some(&epoch) {
                    self.retiring.remove(&cell);
                    self.free.push(cell);
                }
            }
            _ => {}
        }
    }

    /// Per tick (browser, after the acknowledgments were applied): re-sends
    /// unacknowledged inits and the in-flight batch every `resend_ticks`,
    /// reports `host-transport` once past `diag_ticks`, and sends the
    /// pending batch when nothing is in flight.
    pub fn tick(&mut self) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let Tier::Browser(port) = &mut self.tier else {
            return diags;
        };
        for e in self.entries.values_mut().filter(|e| !e.acked) {
            e.init_ticks += 1;
            if e.init_ticks % self.resend_ticks == 0 && e.init_ticks < self.diag_ticks {
                port.post(CtlMsg::CellInit {
                    cell: e.cell,
                    epoch: e.epoch,
                    value: e.value,
                });
                self.stats.inits_sent += 1;
            }
            if e.init_ticks >= self.diag_ticks && !e.init_reported {
                e.init_reported = true;
                diags.push(Diagnostic::error(
                    DiagCode::HostTransport,
                    nowhere(),
                    format!(
                        "control cell {} is not acknowledged by the audio side",
                        e.cell.get()
                    ),
                ));
            }
        }
        match &mut self.in_flight {
            Some(f) => {
                f.ticks += 1;
                // Re-send (loss recovery) only up to the transport threshold:
                // past it the port is reported and re-established with
                // `resync`, so a stalled consumer never piles up copies.
                if f.ticks % self.resend_ticks == 0 && f.ticks < self.diag_ticks {
                    port.post_batch(f.seq, &f.entries);
                    self.stats.resends += 1;
                }
                if f.ticks >= self.diag_ticks && !f.reported {
                    f.reported = true;
                    diags.push(Diagnostic::error(
                        DiagCode::HostTransport,
                        nowhere(),
                        format!("control cell batch {} is not acknowledged", f.seq),
                    ));
                }
            }
            None if !self.pending.is_empty() => {
                self.seq += 1;
                let entries: Vec<(CellId, u32, f32)> = std::mem::take(&mut self.pending)
                    .into_iter()
                    .map(|(c, (ep, v))| (c, ep, v))
                    .collect();
                port.post_batch(self.seq, &entries);
                self.stats.batches_sent += 1;
                self.in_flight = Some(InFlight {
                    seq: self.seq,
                    entries,
                    ticks: 0,
                    reported: false,
                });
            }
            None => {}
        }
        diags
    }

    /// Re-synchronizes a re-established browser port: `CellInit` only for
    /// incarnations never acknowledged, then ONE snapshot batch with every
    /// live cell's current value. Commits use the `Const` downgrade until
    /// the snapshot is acknowledged.
    pub fn resync(&mut self) {
        let Tier::Browser(port) = &mut self.tier else {
            return;
        };
        for e in self.entries.values_mut().filter(|e| !e.acked) {
            port.post(CtlMsg::CellInit {
                cell: e.cell,
                epoch: e.epoch,
                value: e.value,
            });
            self.stats.inits_sent += 1;
            e.init_ticks = 0;
        }
        self.pending.clear();
        self.seq += 1;
        let entries: Vec<(CellId, u32, f32)> = self
            .entries
            .values()
            .map(|e| (e.cell, e.epoch, e.value))
            .collect();
        port.post_batch(self.seq, &entries);
        self.stats.batches_sent += 1;
        self.in_flight = Some(InFlight {
            seq: self.seq,
            entries,
            ticks: 0,
            reported: false,
        });
        self.resync_seq = Some(self.seq);
    }

    /// The cell and epoch of a key.
    #[must_use]
    pub fn cell_of(&self, key: CellKey) -> Option<(CellId, u32)> {
        self.entries.get(&key).map(|e| (e.cell, e.epoch))
    }

    /// The authoritative value of a key.
    #[must_use]
    pub fn value(&self, key: CellKey) -> Option<f32> {
        self.entries.get(&key).map(|e| e.value)
    }

    /// Cells currently fed by a slot: cell, epoch, row, map and ack state.
    #[must_use]
    pub fn site_cells(
        &self,
        slot_id: u64,
    ) -> Vec<(CellId, u32, &'static ControlRow, CellMap, bool)> {
        self.entries
            .iter()
            .filter_map(|(key, entry)| {
                matches!(key, CellKey::Site { slot, .. } if *slot == slot_id).then_some((
                    entry.cell,
                    entry.epoch,
                    entry.row?,
                    entry.map,
                    entry.acked,
                ))
            })
            .collect()
    }

    /// Encodes `value` exactly as a commit to this cell would.
    #[must_use]
    pub fn encode_for(&self, cell: CellId, value: &Value) -> Option<f32> {
        self.entries.values().find_map(|entry| {
            (entry.cell == cell)
                .then(|| encode_value(entry.row?, entry.map, value))
                .flatten()
        })
    }

    /// Whether this cell incarnation is still represented by the table.
    #[must_use]
    pub fn has_cell(&self, cell: CellId, epoch: u32) -> bool {
        self.entries
            .values()
            .any(|entry| entry.cell == cell && entry.epoch == epoch)
    }

    /// Metadata for a live cell, used by the momentary ramp sender.
    #[must_use]
    pub fn cell_state(&self, cell: CellId) -> Option<(u32, bool, f32)> {
        self.entries
            .values()
            .find(|entry| entry.cell == cell)
            .map(|entry| (entry.epoch, entry.acked, entry.value))
    }

    /// Whether a row's encoded value is discrete.
    #[must_use]
    pub fn stepped(row: &ControlRow) -> bool {
        matches!(row.domain, CtlDomain::Bool | CtlDomain::Enum(_))
    }

    /// Sets the fallback value used while a browser cell is not acknowledged.
    pub fn set_momentary(&mut self, cell: CellId, epoch: u32, value: Option<f32>) {
        if let Some(entry) = self
            .entries
            .values_mut()
            .find(|entry| entry.cell == cell && entry.epoch == epoch)
        {
            entry.momentary = value;
        }
    }

    /// Posts a ramp through the tier's priority control channel.
    pub fn post_ctl(&mut self, msg: CtlMsg, audio: &mut dyn AudioHost) {
        match &mut self.tier {
            Tier::Native(_) => audio.post(msg),
            Tier::Browser(port) => port.post(msg),
        }
    }

    /// Batches outstanding: `(in flight, pending)`, each 0 or 1.
    #[must_use]
    pub fn outstanding(&self) -> (usize, usize) {
        (
            usize::from(self.in_flight.is_some()),
            usize::from(!self.pending.is_empty()),
        )
    }

    /// The pending (coalesced) entries.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub fn stats(&self) -> CellStats {
        self.stats
    }

    /// The native table the audio callback reads, if native.
    #[must_use]
    pub fn native(&self) -> Option<&AtomicCells> {
        match &self.tier {
            Tier::Native(c) => Some(c),
            Tier::Browser(_) => None,
        }
    }

    /// The live cells fed by var or tweak slots.
    #[must_use]
    pub fn live_sites(&self) -> Vec<(CellKey, CellId, u32)> {
        self.entries
            .iter()
            .map(|(k, e)| (*k, e.cell, e.epoch))
            .collect()
    }

    /// Drains the browser port's acknowledgments without ticking (the
    /// runtime routes them).
    pub fn drain_port(&mut self, out: &mut Vec<HostMsg>) {
        if let Tier::Browser(port) = &mut self.tier {
            port.drain(out);
        }
    }
}
