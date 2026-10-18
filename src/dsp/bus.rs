//! Buses and the master (design 12.5, 12.8.6).
//!
//! A bus is a preallocated slot running an effect chain over the sum of the
//! voices routed to it (the `bus` control); every bus feeds the master,
//! which is the root. Each slot also owns a built-in `room` reverb whose
//! mix and size are the pattern controls `room` and `size` (their
//! `BusUnit` route, 12.8.7); at `room 0` it is bit-transparent.
//!
//! Swaps follow the generation + refcount lifecycle: installing a chain
//! for a bus that already has one marks the old slot `Retiring`; it keeps
//! rendering for the voices that were routed to it and is freed (and
//! `Retired` acked) when none is left. Slots, units and their delay memory
//! are allocated once in `BusGraph::new`; an install only reconfigures a
//! free slot.
//!
//! A `section` unit with `on 0` bypasses the `count` units after it.

use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellRead;
use crate::dsp::effects::prim::DelayLine;
use crate::dsp::effects::{self, FxCtx, FxUnit, MAX_FX_PARAMS};
use crate::dsp::graph::{AudioPortShape, BusDef, BusId, EffectKind};
use crate::dsp::ugen::BuildError;
use crate::host::wire::Ctl;
use crate::sched::slots::CtlId;
use crate::song::routing::{SongLeaseKey, SongRejectCode};

mod occupancy;
mod song_delay;
mod song_runtime;
pub use song_delay::SongPrivateDelayLayout;

/// The most units in one chain.
pub const MAX_CHAIN: usize = 8;
/// Bus slots, the master's included (a swap needs a free one).
pub const DEFAULT_BUS_SLOTS: usize = 6;
/// The master's resource id before any `master:` install.
pub const DEFAULT_MASTER: u32 = u32::MAX;

/// The `room` unit's parameter indices (see `effects::reverb`).
const ROOM_SIZE: usize = 0;

/// A compiled chain (POD, fixed capacity).
#[derive(Clone, Copy, Debug)]
pub struct BusTemplate {
    pub bus: BusId,
    pub n: usize,
    pub kinds: [EffectKind; MAX_CHAIN],
    pub n_params: [u8; MAX_CHAIN],
    pub params: [[(CtlId, Ctl); MAX_FX_PARAMS]; MAX_CHAIN],
}

impl Default for BusTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl BusTemplate {
    /// A bus chain receives and emits independent left/right channels.
    /// The shape is derived from the graph kind on both native and browser
    /// tiers; no extra serialized flag can disagree with the effect kind.
    #[must_use]
    pub const fn port_shape(&self) -> AudioPortShape {
        AudioPortShape::STEREO
    }

    pub(crate) fn remap_song_banks(
        &mut self,
        key: SongLeaseKey,
        stager: &crate::dsp::arena::SongResourceStager,
        store: &crate::dsp::arena::SampleStore,
    ) -> Result<(), SongRejectCode> {
        if self.n > self.params.len() {
            return Err(SongRejectCode::Malformed);
        }
        for (params, n) in self.params.iter().zip(self.n_params.iter()).take(self.n) {
            if usize::from(*n) > params.len() {
                return Err(SongRejectCode::Malformed);
            }
            for (_, ctl) in params.iter().take(usize::from(*n)) {
                crate::dsp::arena::song::validate_ctl(stager, key, *ctl)?;
            }
        }
        for index in 0..self.n {
            if let EffectKind::Analyzer(kind) = self.kinds[index] {
                stager.validate_analyzer(
                    key,
                    kind,
                    &self.params[index][..usize::from(self.n_params[index])],
                )?;
            }
        }
        for k in 0..self.n {
            if self.kinds[k] == EffectKind::Convolution {
                for (id, ctl) in self.params[k].iter().take(usize::from(self.n_params[k])) {
                    if Some(*id) == effects::param_ctl(EffectKind::Convolution, "ir") {
                        stager.resource_ctl(key, *ctl, store)?;
                    }
                }
            }
        }
        for (k, (params, n)) in self
            .params
            .iter_mut()
            .zip(self.n_params.iter())
            .take(self.n)
            .enumerate()
        {
            for (id, ctl) in params.iter_mut().take(usize::from(*n)) {
                *ctl = if self.kinds[k] == EffectKind::Convolution
                    && Some(*id) == effects::param_ctl(EffectKind::Convolution, "ir")
                {
                    stager.resource_ctl(key, *ctl, store)?
                } else {
                    crate::dsp::arena::song::mapped_ctl(stager, key, *ctl)?
                };
            }
        }
        Ok(())
    }
    /// An empty chain.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bus: BusId::new(0),
            n: 0,
            kinds: [EffectKind::Gain; MAX_CHAIN],
            n_params: [0; MAX_CHAIN],
            params: [[(CtlId::new(0), Ctl::Const(0.0)); MAX_FX_PARAMS]; MAX_CHAIN],
        }
    }

    /// Empties the chain.
    pub fn clear(&mut self) {
        self.n = 0;
    }

    /// Appends a unit; `None` when the chain is full.
    pub fn push(&mut self, kind: EffectKind) -> Option<usize> {
        let k = self.n;
        if k >= MAX_CHAIN {
            return None;
        }
        self.kinds[k] = kind;
        self.n_params[k] = 0;
        self.n += 1;
        Some(k)
    }

    /// Appends a parameter to unit `k`; false when full.
    pub fn push_param(&mut self, k: usize, id: CtlId, c: Ctl) -> bool {
        let Some(n) = self.n_params.get(k).map(|n| usize::from(*n)) else {
            return false;
        };
        if n >= MAX_FX_PARAMS {
            return false;
        }
        self.params[k][n] = (id, c);
        self.n_params[k] += 1;
        true
    }

    /// Rejects malformed reserved selector transport before installing a chain.
    #[must_use]
    pub fn valid_sidechains(&self) -> bool {
        (0..self.n.min(MAX_CHAIN)).all(|k| {
            let mut selected = false;
            self.params[k][..usize::from(self.n_params[k]).min(MAX_FX_PARAMS)]
                .iter()
                .all(|&(id, ctl)| {
                    if id != effects::SIDECHAIN_BUS_CTL {
                        return true;
                    }
                    let valid = !selected
                        && self.kinds[k] == EffectKind::Compressor
                        && effects::sidechain_bus(ctl).is_some_and(|source| source != self.bus);
                    selected = true;
                    valid
                })
        })
    }

    /// The template of a `BusDef` (evaluator side).
    ///
    /// # Errors
    /// `TooManyEffects` / `TooManyParams` over the capacities.
    pub fn from_def(def: &BusDef) -> Result<Self, BuildError> {
        let mut t = Self::new();
        t.bus = def.id;
        for e in def.chain.iter() {
            let k = t.push(e.kind).ok_or(BuildError::TooManyEffects)?;
            for &(id, c) in e.params.iter() {
                if !t.push_param(k, id, c) {
                    return Err(BuildError::TooManyParams);
                }
            }
        }
        if !t.valid_sidechains() {
            return Err(BuildError::BadEdge);
        }
        Ok(t)
    }
}

/// A bus slot's lifecycle state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotState {
    /// Owned by a song preparation and never rendered.
    Staged,
    Free,
    Live,
    Retiring,
}

/// Immutable pre-effect accumulators, captured before any bus runs.
#[derive(Debug)]
struct KeySnapshot {
    bus: BusId,
    active: bool,
    l: Box<[f32]>,
    r: Box<[f32]>,
}

/// One preallocated bus.
#[derive(Debug)]
pub struct BusSlot {
    legacy_eligible: bool,
    song_template: Option<Box<BusTemplate>>,
    song_definition: Option<BusTemplate>,
    pub(crate) song_delay: song_delay::SongPrivateDelay,
    pub(crate) song_key: Option<crate::song::routing::SongLeaseKey>,
    pub state: SlotState,
    pub bus: BusId,
    pub master: bool,
    pub resource: u32,
    pub gen: u32,
    n: usize,
    units: [FxUnit; MAX_CHAIN],
    regions: [(usize, usize); MAX_CHAIN],
    room: FxUnit,
    room_region: (usize, usize),
    mem: Box<[f32]>,
    /// The input accumulators (stereo).
    pub l: Box<[f32]>,
    pub r: Box<[f32]>,
    /// Voices routed here.
    pub users: u32,
}

impl BusSlot {
    fn new(max_block: usize, mem: usize) -> Self {
        Self {
            legacy_eligible: true,
            song_template: None,
            song_definition: None,
            song_delay: song_delay::SongPrivateDelay::new(max_block),
            song_key: None,
            state: SlotState::Free,
            bus: BusId::new(0),
            master: false,
            resource: 0,
            gen: 0,
            n: 0,
            units: [FxUnit::empty(); MAX_CHAIN],
            regions: [(0, 0); MAX_CHAIN],
            room: FxUnit::empty(),
            room_region: (0, 0),
            mem: vec![0.0; mem].into_boxed_slice(),
            l: vec![0.0; max_block].into_boxed_slice(),
            r: vec![0.0; max_block].into_boxed_slice(),
            users: 0,
        }
    }

    /// Configures the slot for a chain (no allocation; delay memory is
    /// carved in chain order and clamped to the slot).
    fn configure<C: CellRead + ?Sized>(
        &mut self,
        t: &BusTemplate,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) {
        let total = self.mem.len();
        let mut cursor = 0;
        let room_want = effects::mem_len(EffectKind::Room, sr, caps).min(total / 4);
        self.room_region = (0, room_want);
        cursor += room_want;
        let (a, b) = self.room_region;
        let room_params = [(
            effects::param_ctl(EffectKind::Room, "mix").unwrap_or(CtlId::new(0)),
            Ctl::Const(0.0),
        )];
        self.room.configure(
            EffectKind::Room,
            &room_params,
            cells,
            &mut self.mem[a..a + b],
            sr,
            caps,
        );
        self.n = t.n.min(MAX_CHAIN);
        for k in 0..self.n {
            let kind = t.kinds[k];
            let want = effects::mem_len(kind, sr, caps).min(total - cursor);
            self.regions[k] = (cursor, want);
            let params = &t.params[k][..usize::from(t.n_params[k]).min(MAX_FX_PARAMS)];
            self.units[k].configure(
                kind,
                params,
                cells,
                &mut self.mem[cursor..cursor + want],
                sr,
                caps,
            );
            cursor += want;
        }
    }

    fn clear_len(&self) -> usize {
        let mut end = self.room_region.0.saturating_add(self.room_region.1);
        for (offset, len) in &self.regions[..self.n] {
            end = end.max(offset.saturating_add(*len));
        }
        end.min(self.mem.len())
    }

    fn clear_memory(&mut self, from: usize, max: usize) -> usize {
        let end = self.clear_len();
        let start = from.min(end);
        let count = max.min(end - start);
        self.mem[start..start + count].fill(0.0);
        if start + count >= end {
            self.l.fill(0.0);
            self.r.fill(0.0);
        }
        count
    }
    fn reinitialize_clear_region(
        &mut self,
        region: usize,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Option<usize> {
        match region {
            0 => {
                let (offset, len) = self.room_region;
                self.room
                    .clear_state(&mut self.mem[offset..offset + len], sr, caps);
                Some(len)
            }
            unit if unit <= self.n => {
                let index = unit - 1;
                let (offset, len) = self.regions[index];
                self.units[index].clear_state(&mut self.mem[offset..offset + len], sr, caps);
                Some(len)
            }
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn memory_is_clear(&self) -> bool {
        let mut samples = self.mem.iter().chain(&self.l[..]).chain(&self.r[..]);
        samples.all(|&sample| sample == 0.0)
    }
    /// Sets the built-in reverb from the `room` (mix) and `size` controls.
    pub fn set_room(&mut self, room: Option<f32>, size: Option<f32>) {
        if let (Some(v), Some(i)) = (room, effects::mix_index(EffectKind::Room)) {
            self.room.set(i, v);
        }
        if let Some(v) = size {
            self.room.set(ROOM_SIZE, v);
        }
    }

    /// The built-in reverb's `room` (mix) target.
    #[must_use]
    pub fn room(&self) -> f32 {
        effects::mix_index(EffectKind::Room).map_or(0.0, |i| self.room.target(i))
    }

    /// The chain's unit kinds.
    pub fn kinds(&self) -> impl Iterator<Item = EffectKind> + '_ {
        self.units[..self.n].iter().map(|u| u.kind)
    }

    /// Runs reverb and chain over the input accumulators (`frames`).
    #[allow(clippy::too_many_arguments)]
    fn run<C: CellRead + ?Sized>(
        &mut self,
        frames: usize,
        cells: &C,
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
        snapshots: &[KeySnapshot],
        key: &mut [f32],
    ) {
        self.run_range(0..frames, cells, dry, ctx, snapshots, key);
    }

    #[allow(clippy::too_many_arguments)]
    fn run_range<C: CellRead + ?Sized>(
        &mut self,
        range: std::ops::Range<usize>,
        cells: &C,
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
        snapshots: &[KeySnapshot],
        key: &mut [f32],
    ) {
        let frames = range.len();
        let first = range.start;
        let (l, r) = (&mut self.l[range.clone()], &mut self.r[range]);
        let (a, b) = self.room_region;
        self.room.update(cells);
        self.room.run(&mut self.mem[a..a + b], l, r, dry, ctx);
        let mut skip = 0usize;
        for k in 0..self.n {
            let unit = &mut self.units[k];
            unit.update(cells);
            if skip > 0 {
                skip -= 1;
                continue;
            }
            if unit.kind == EffectKind::Section {
                if unit.value(0) < 0.5 {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        skip = unit.value(1).max(0.0) as usize;
                    }
                }
                continue;
            }
            let (off, len) = self.regions[k];
            let detector = unit.sidechain().map(|bus| {
                // Sum channels across live and retiring generations before rectifying.
                for (frame, peak) in key[..frames].iter_mut().enumerate() {
                    let (mut left, mut right) = (0.0f32, 0.0f32);
                    for source in snapshots.iter().filter(|s| s.active && s.bus == bus) {
                        left += source.l[first + frame];
                        right += source.r[first + frame];
                    }
                    *peak = left.abs().max(right.abs());
                }
                &key[..frames]
            });
            unit.run_with_key(&mut self.mem[off..off + len], l, r, dry, ctx, detector);
        }
        for x in l.iter_mut().chain(r.iter_mut()) {
            if !x.is_finite() {
                *x = 0.0;
            }
        }
    }
}

/// Every bus plus the master.
#[derive(Debug)]
pub struct BusGraph {
    song_regions: Box<[crate::dsp::engine::SongFrameRegion]>,
    pub slots: Box<[BusSlot]>,
    snapshots: Box<[KeySnapshot]>,
    key: Box<[f32]>,
    hold_retiring: bool,
}

impl BusGraph {
    /// Preallocates `n_slots` slots of `slot_mem` floats each; slot 0 starts
    /// as the master with an empty chain.
    #[must_use]
    pub fn new<C: CellRead + ?Sized>(
        n_slots: usize,
        max_block: usize,
        slot_mem: usize,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Self {
        Self::new_with_memory_profile(n_slots, max_block, slot_mem, None, cells, sr, caps)
            .expect("uniform bus constructor")
    }

    /// Allocates heterogeneous regions once, before any callback.
    /// # Errors
    /// Invalid profile, unsupported rate, or overflowing state geometry.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_memory_profile<C: CellRead + ?Sized>(
        n_slots: usize,
        max_block: usize,
        full_mem: usize,
        profile: Option<crate::dsp::ring::SongBusMemoryProfile>,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<Self, crate::dsp::ring::ConfigError> {
        if profile.is_some()
            && !(1..=crate::dsp::ring::EngineConfig::MAX_BLOCK).contains(&max_block)
        {
            return Err(crate::dsp::ring::ConfigError::BlockSize);
        }
        n_slots
            .max(2)
            .checked_mul(max_block)
            .and_then(|n| n.checked_mul(6 * std::mem::size_of::<f32>()))
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or(crate::dsp::ring::ConfigError::StateBudget)?;
        let (full_slots, small_mem) = match profile {
            Some(p) if p.full_slots >= 2 && p.full_slots <= n_slots => (
                p.full_slots,
                small_song_region_frames(sr, caps, p.small_chain_seconds)?,
            ),
            Some(_) => return Err(crate::dsp::ring::ConfigError::StateBudget),
            None => (n_slots.max(2), full_mem),
        };
        let n_slots = n_slots.max(2);
        full_mem
            .checked_mul(full_slots)
            .and_then(|n| {
                small_mem
                    .checked_mul(n_slots - full_slots)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_mul(std::mem::size_of::<f32>()))
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or(crate::dsp::ring::ConfigError::StateBudget)?;
        let mut slots: Box<[BusSlot]> = (0..n_slots)
            .map(|index| {
                let mut slot = BusSlot::new(
                    max_block,
                    if index < full_slots {
                        full_mem
                    } else {
                        small_mem
                    },
                );
                slot.legacy_eligible = index < full_slots;
                slot
            })
            .collect();
        let m = &mut slots[0];
        m.configure(&BusTemplate::new(), cells, sr, caps);
        m.state = SlotState::Live;
        m.master = true;
        m.resource = DEFAULT_MASTER;
        let snapshots = (0..slots.len())
            .map(|_| KeySnapshot {
                bus: BusId::new(0),
                active: false,
                l: vec![0.0; max_block].into_boxed_slice(),
                r: vec![0.0; max_block].into_boxed_slice(),
            })
            .collect();
        let song_regions = slots
            .iter()
            .enumerate()
            .map(|(slot, s)| crate::dsp::engine::SongFrameRegion {
                slot: u32::try_from(slot).unwrap_or(u32::MAX),
                offset: 0,
                frames: if s.state == SlotState::Free {
                    u64::try_from(s.mem.len()).unwrap_or(0)
                } else {
                    0
                },
            })
            .collect();
        Ok(Self {
            song_regions,
            slots,
            snapshots,
            key: vec![0.0; max_block].into_boxed_slice(),
            hold_retiring: false,
        })
    }

    fn refresh_song_regions(&mut self) {
        for (region, slot) in self.song_regions.iter_mut().zip(self.slots.iter()) {
            region.frames = if slot.state == SlotState::Free {
                u64::try_from(slot.mem.len()).unwrap_or(0)
            } else {
                0
            };
        }
    }

    /// Actual maintained free per-slot state regions, in float units.
    #[must_use]
    pub fn song_regions(&self) -> &[crate::dsp::engine::SongFrameRegion] {
        &self.song_regions
    }

    /// The live master slot.
    #[must_use]
    pub fn master(&self) -> usize {
        self.slots
            .iter()
            .position(|s| s.song_key.is_none() && s.master && s.state == SlotState::Live)
            .unwrap_or(0)
    }

    /// The live slot of bus `bus` (never the master), if any.
    #[must_use]
    pub fn find(&self, bus: BusId) -> Option<usize> {
        self.slots.iter().position(|s| {
            s.song_key.is_none() && !s.master && s.state == SlotState::Live && s.bus == bus
        })
    }

    /// The first `n` frames of slot `index`'s last rendered block, left and
    /// right (read-only; the taps copy them, 14.5.9). Empty for an index or
    /// length out of range.
    #[must_use]
    pub fn frames(&self, index: usize, n: usize) -> (&[f32], &[f32]) {
        match self.slots.get(index) {
            Some(s) => (&s.l[..n.min(s.l.len())], &s.r[..n.min(s.r.len())]),
            None => (&[], &[]),
        }
    }

    /// The live slot of a bus, else the master.
    #[must_use]
    pub fn route(&self, bus: Option<BusId>) -> usize {
        bus.and_then(|b| {
            self.slots.iter().position(|s| {
                s.song_key.is_none() && !s.master && s.state == SlotState::Live && s.bus == b
            })
        })
        .unwrap_or_else(|| self.master())
    }

    /// `GraphRetire` of a bus chain: its live slot starts retiring.
    pub fn retire(&mut self, resource: u32) {
        for s in self.slots.iter_mut() {
            if s.song_key.is_none()
                && s.state == SlotState::Live
                && s.resource == resource
                && !s.master
            {
                s.state = SlotState::Retiring;
            }
        }
    }

    /// True when a live or retiring chain reads `cell` (11.3 retire check).
    #[must_use]
    pub fn reads_cell(&self, cell: crate::dsp::cells::CellId) -> bool {
        self.slots.iter().any(|s| {
            s.state != SlotState::Free
                && (s.room.reads_cell(cell) || s.units[..s.n].iter().any(|u| u.reads_cell(cell)))
        })
    }

    /// True when a live or retiring chain reads resource `id` (an impulse
    /// response).
    #[must_use]
    pub fn references(&self, id: u32) -> bool {
        self.slots.iter().any(|s| {
            s.state != SlotState::Free && s.units[..s.n].iter().any(|u| u.reads_resource(id))
        })
    }

    /// Clears every input accumulator for a block.
    pub fn clear(&mut self, frames: usize) {
        for s in self.slots.iter_mut().filter(|s| s.state != SlotState::Free) {
            s.song_delay.send_l[..frames].fill(0.0);
            s.song_delay.send_r[..frames].fill(0.0);
            s.l[..frames].fill(0.0);
            s.r[..frames].fill(0.0);
        }
    }

    /// Defers collection of retiring legacy slots while their effect tails render.
    pub(crate) fn hold_retiring(&mut self, hold: bool) {
        self.hold_retiring = hold;
    }

    pub(crate) fn clear_slot_memory(&mut self, index: usize, from: usize, max: usize) -> usize {
        self.slots
            .get_mut(index)
            .filter(|slot| {
                slot.song_key.is_none()
                    && matches!(slot.state, SlotState::Live | SlotState::Retiring)
            })
            .map_or(0, |slot| slot.clear_memory(from, max))
    }

    pub(crate) fn reinitialize_clear_region(
        &mut self,
        index: usize,
        region: usize,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Option<usize> {
        let slot = self.slots.get_mut(index)?;
        (slot.song_key.is_none() && matches!(slot.state, SlotState::Live | SlotState::Retiring))
            .then(|| slot.reinitialize_clear_region(region, sr, caps))
            .flatten()
    }

    pub(crate) fn clear_region_len(&self, index: usize, region: usize) -> Option<usize> {
        let slot = self.slots.get(index).filter(|slot| {
            slot.song_key.is_none() && matches!(slot.state, SlotState::Live | SlotState::Retiring)
        })?;
        match region {
            0 => Some(slot.room_region.1),
            region if region <= slot.n => slot.regions.get(region - 1).map(|(_, len)| *len),
            _ => None,
        }
    }

    pub(crate) fn clear_len(&self, index: usize) -> usize {
        self.slots[index].clear_len()
    }

    /// Frees retiring slots nobody uses; returns their resource ids through
    /// `out` (at most `out.len()`), and the count.
    pub fn collect(&mut self, out: &mut [u32]) -> usize {
        let mut n = 0;
        for s in self.slots.iter_mut() {
            if !self.hold_retiring
                && s.song_key.is_none()
                && s.state == SlotState::Retiring
                && s.users == 0
                && n < out.len()
            {
                s.state = SlotState::Free;
                s.master = false;
                out[n] = s.resource;
                n += 1;
            }
        }
        self.refresh_song_regions();
        n
    }

    /// Runs every bus into the master, then the master into `(l, r)`.
    pub fn render<C: CellRead + ?Sized>(
        &mut self,
        frames: usize,
        cells: &C,
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
        l: &mut [f32],
        r: &mut [f32],
    ) {
        for (source, snapshot) in self.slots.iter().zip(self.snapshots.iter_mut()) {
            snapshot.active = source.song_key.is_none()
                && !source.master
                && matches!(source.state, SlotState::Live | SlotState::Retiring);
            snapshot.bus = source.bus;
            if snapshot.active {
                snapshot.l[..frames].copy_from_slice(&source.l[..frames]);
                snapshot.r[..frames].copy_from_slice(&source.r[..frames]);
            }
        }
        let m = self.master();
        for i in 0..self.slots.len() {
            if self.slots[i].song_key.is_some()
                || i == m
                || !matches!(self.slots[i].state, SlotState::Live | SlotState::Retiring)
            {
                continue;
            }
            self.slots[i].run(frames, cells, dry, ctx, &self.snapshots, &mut self.key);
            let (src, dst) = if i < m {
                let (a, b) = self.slots.split_at_mut(m);
                (&a[i], &mut b[0])
            } else {
                let (a, b) = self.slots.split_at_mut(i);
                (&b[0], &mut a[m])
            };
            for k in 0..frames {
                dst.l[k] += src.l[k];
                dst.r[k] += src.r[k];
            }
        }
        let master = &mut self.slots[m];
        master.run(frames, cells, dry, ctx, &self.snapshots, &mut self.key);
        for k in 0..frames {
            l[k] = master.l[k];
            r[k] = master.r[k];
        }
    }
}

/// One orbit's delay (the `delay`/`delaytime`/`delayfeedback` controls,
/// SuperDirt style): voices send into it, its output feeds the master.
#[derive(Debug)]
pub struct OrbitDelay {
    mem: Box<[f32]>,
    l: DelayLine,
    r: DelayLine,
    pub time: f32,
    pub feedback: f32,
    pub in_l: Box<[f32]>,
    pub in_r: Box<[f32]>,
}

impl OrbitDelay {
    /// A stereo delay of `len` samples per channel for blocks of
    /// `max_block`.
    #[must_use]
    pub fn new(len: usize, max_block: usize) -> Self {
        let mut cursor = 0;
        let l = DelayLine::carve(&mut cursor, 2 * len, len);
        let r = DelayLine::carve(&mut cursor, 2 * len, len);
        Self {
            mem: vec![0.0; 2 * len].into_boxed_slice(),
            l,
            r,
            time: 0.25,
            feedback: 0.5,
            in_l: vec![0.0; max_block].into_boxed_slice(),
            in_r: vec![0.0; max_block].into_boxed_slice(),
        }
    }

    /// Clears at most `max` delay-memory samples, resetting line positions at completion.
    pub fn clear_memory(&mut self, from: usize, max: usize) -> usize {
        let start = from.min(self.mem.len());
        let count = max.min(self.mem.len().saturating_sub(start));
        self.mem[start..start + count].fill(0.0);
        if start + count >= self.mem.len() {
            self.l.pos = 0;
            self.r.pos = 0;
            self.in_l.fill(0.0);
            self.in_r.fill(0.0);
        }
        count
    }

    #[must_use]
    pub fn memory_len(&self) -> usize {
        self.mem.len()
    }

    #[cfg(test)]
    pub(crate) fn memory_is_clear(&self) -> bool {
        self.mem.iter().all(|sample| *sample == 0.0)
    }

    /// Sets the time (seconds) and feedback from an event's controls.
    pub fn set(&mut self, time: Option<f32>, feedback: Option<f32>) {
        if let Some(t) = time.filter(|t| t.is_finite()) {
            self.time = t.clamp(0.0, 4.0);
        }
        if let Some(f) = feedback.filter(|f| f.is_finite()) {
            self.feedback = f.clamp(0.0, 0.95);
        }
    }

    /// Clears the send accumulators.
    pub fn clear(&mut self, n: usize) {
        self.in_l[..n].fill(0.0);
        self.in_r[..n].fill(0.0);
    }

    /// Runs `n` frames, adding the delayed signal into `(l, r)`.
    pub fn run(&mut self, n: usize, sr: f32, l: &mut [f32], r: &mut [f32]) {
        let d = self.time * sr;
        for k in 0..n.min(l.len()).min(r.len()) {
            let (yl, yr) = (self.l.read(&self.mem, d), self.r.read(&self.mem, d));
            self.l
                .write(&mut self.mem, self.in_l[k] + self.feedback * yl);
            self.r
                .write(&mut self.mem, self.in_r[k] + self.feedback * yr);
            l[k] += yl;
            r[k] += yr;
        }
    }
}

use crate::dsp::arena::SongResourceStager;

impl SongResourceStager {
    pub(crate) fn validate_analyzer(
        &self,
        graph: SongLeaseKey,
        kind: crate::dsp::graph::AnalyzerKind,
        params: &[(crate::sched::slots::CtlId, Ctl)],
    ) -> Result<(), SongRejectCode> {
        let bank = self
            .bindings
            .iter()
            .find(|b| b.graph == graph)
            .and_then(|b| b.analysis)
            .ok_or(SongRejectCode::NotReady)?;
        let region = self.analysis_bank(bank)?;
        let id =
            crate::dsp::effects::param_ctl(crate::dsp::graph::EffectKind::Analyzer(kind), "id")
                .ok_or(SongRejectCode::Malformed)?;
        let value = match params
            .iter()
            .find(|(ctl, _)| *ctl == id)
            .map(|(_, value)| *value)
            .unwrap_or(Ctl::Const(0.0))
        {
            Ctl::Const(value) => value,
            Ctl::Cell(cell) => CellRead::get(self, self.bound_cell(graph, cell)?),
        };
        if !value.is_finite() {
            return Err(SongRejectCode::Malformed);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = value.clamp(0.0, 65_535.0) as u64;
        if start
            .checked_add(
                u64::try_from(crate::dsp::effects::analyzer::cells(kind))
                    .map_err(|_| SongRejectCode::Capacity)?,
            )
            .is_none_or(|end| end > region.frames)
        {
            return Err(SongRejectCode::Capacity);
        }
        Ok(())
    }
}

impl SongResourceStager {
    pub(crate) fn resource_ctl(
        &self,
        graph: SongLeaseKey,
        ctl: Ctl,
        store: &crate::dsp::arena::SampleStore,
    ) -> Result<Ctl, SongRejectCode> {
        let value = match ctl {
            Ctl::Const(value) => value,
            Ctl::Cell(cell) => CellRead::get(self, self.bound_cell(graph, cell)?),
        };
        if !value.is_finite() {
            return Err(SongRejectCode::Malformed);
        }
        if value < 0.0 {
            return Ok(Ctl::Const(value));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let logical = value as u32;
        let physical = self.sample_id(store, graph.epoch, logical)?;
        #[allow(clippy::cast_precision_loss)]
        let encoded = physical as f32;
        if f64::from(encoded) != f64::from(physical) {
            return Err(SongRejectCode::Capacity);
        }
        Ok(Ctl::Const(encoded))
    }
}

impl BusGraph {
    /// Installs a chain; the previous live slot of the same bus (or the
    /// previous master) becomes `Retiring`. `false` when no slot is free.
    #[allow(clippy::too_many_arguments)]
    pub fn install<C: CellRead + ?Sized>(
        &mut self,
        t: &BusTemplate,
        master: bool,
        resource: u32,
        gen: u32,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> bool {
        self.install_with_song_claims(t, master, resource, gen, cells, sr, caps, 0)
    }
}

/// Exact constructor geometry; uses the actual room implementation and checked scalar conversion.
pub(crate) fn small_song_region_frames(
    sr: f32,
    caps: &CapabilitySet,
    chain_seconds: f32,
) -> Result<usize, crate::dsp::ring::ConfigError> {
    use crate::dsp::ring::{ConfigError, EngineConfig};
    if !sr.is_finite()
        || !(EngineConfig::MIN_SAMPLE_RATE..=EngineConfig::MAX_SAMPLE_RATE).contains(&sr)
        || sr.fract() != 0.
        || !chain_seconds.is_finite()
        || chain_seconds <= 0.
        || chain_seconds * sr > EngineConfig::MAX_STATE_SAMPLES
    {
        return Err(ConfigError::StateBudget);
    }
    let chain = (f64::from(chain_seconds) * f64::from(sr)).ceil();
    if chain > f64::from(u32::MAX) {
        return Err(ConfigError::StateBudget);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (rate, chain) = (sr as usize, chain as usize);
    effects::mem_len(EffectKind::Room, sr, caps)
        .checked_mul(4)
        .and_then(|room| {
            rate.checked_mul(4)
                .and_then(|n| n.checked_add(4))
                .and_then(|delay| delay.checked_mul(2))
                .and_then(|delay| room.checked_add(delay))
        })
        .and_then(|n| n.checked_add(chain))
        .ok_or(ConfigError::StateBudget)
}
