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
        Ok(t)
    }
}

/// A bus slot's lifecycle state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotState {
    Free,
    Live,
    Retiring,
}

/// One preallocated bus.
#[derive(Debug)]
pub struct BusSlot {
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
    fn run<C: CellRead + ?Sized>(
        &mut self,
        frames: usize,
        cells: &C,
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
    ) {
        let (l, r) = (&mut self.l[..frames], &mut self.r[..frames]);
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
            unit.run(&mut self.mem[off..off + len], l, r, dry, ctx);
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
    pub slots: Box<[BusSlot]>,
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
        let mut slots: Box<[BusSlot]> = (0..n_slots.max(2))
            .map(|_| BusSlot::new(max_block, slot_mem))
            .collect();
        let m = &mut slots[0];
        m.configure(&BusTemplate::new(), cells, sr, caps);
        m.state = SlotState::Live;
        m.master = true;
        m.resource = DEFAULT_MASTER;
        Self { slots }
    }

    /// The live master slot.
    #[must_use]
    pub fn master(&self) -> usize {
        self.slots
            .iter()
            .position(|s| s.master && s.state == SlotState::Live)
            .unwrap_or(0)
    }

    /// The live slot of bus `bus` (never the master), if any.
    #[must_use]
    pub fn find(&self, bus: BusId) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| !s.master && s.state == SlotState::Live && s.bus == bus)
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
            self.slots
                .iter()
                .position(|s| !s.master && s.state == SlotState::Live && s.bus == b)
        })
        .unwrap_or_else(|| self.master())
    }

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
        let Some(free) = self.slots.iter().position(|s| s.state == SlotState::Free) else {
            return false;
        };
        // Effects with fixed, required state reject a short region before
        // retiring the live chain. Capture effects also require capture
        // allowance; ordinary effects keep their clamped-memory behavior.
        let total = self.slots[free].mem.len();
        let room = effects::mem_len(EffectKind::Room, sr, caps).min(total / 4);
        let mut remaining = total - room;
        for kind in t.kinds.iter().take(t.n.min(MAX_CHAIN)) {
            let want = effects::mem_len(*kind, sr, caps);
            let capture_effect = matches!(
                *kind,
                EffectKind::TextureGrain
                    | EffectKind::TextureStretch
                    | EffectKind::TextureLoop
                    | EffectKind::TextureSpectral
            );
            if capture_effect && caps.max_capture_seconds < effects::texture::CAPTURE_SECONDS {
                return false;
            }
            if (capture_effect
                || matches!(
                    *kind,
                    EffectKind::CrossMod
                        | EffectKind::ResonantBank
                        | EffectKind::ElementsBank
                        | EffectKind::StreamEnvelope
                        | EffectKind::StreamVactr
                        | EffectKind::StreamFollower
                        | EffectKind::StreamCompressor
                        | EffectKind::StreamFilter
                        | EffectKind::StreamLorenz
                        | EffectKind::ShiftPair
                ))
                && want > remaining
            {
                return false;
            }
            remaining = remaining.saturating_sub(want);
        }
        for s in self.slots.iter_mut() {
            let same = if master {
                s.master
            } else {
                !s.master && s.bus == t.bus
            };
            if same && s.state == SlotState::Live {
                s.state = SlotState::Retiring;
            }
        }
        let s = &mut self.slots[free];
        s.configure(t, cells, sr, caps);
        s.state = SlotState::Live;
        s.master = master;
        s.bus = t.bus;
        s.resource = resource;
        s.gen = gen;
        s.users = 0;
        true
    }

    /// `GraphRetire` of a bus chain: its live slot starts retiring.
    pub fn retire(&mut self, resource: u32) {
        for s in self.slots.iter_mut() {
            if s.state == SlotState::Live && s.resource == resource && !s.master {
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
            s.l[..frames].fill(0.0);
            s.r[..frames].fill(0.0);
        }
    }

    /// Frees retiring slots nobody uses; returns their resource ids through
    /// `out` (at most `out.len()`), and the count.
    pub fn collect(&mut self, out: &mut [u32]) -> usize {
        let mut n = 0;
        for s in self.slots.iter_mut() {
            if s.state == SlotState::Retiring && s.users == 0 && n < out.len() {
                s.state = SlotState::Free;
                s.master = false;
                out[n] = s.resource;
                n += 1;
            }
        }
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
        let m = self.master();
        for i in 0..self.slots.len() {
            if i == m || self.slots[i].state == SlotState::Free {
                continue;
            }
            self.slots[i].run(frames, cells, dry, ctx);
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
        master.run(frames, cells, dry, ctx);
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
