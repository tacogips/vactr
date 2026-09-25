//! The audio callback core (design 12.2, 12.8.9, 16.1).
//!
//! `Engine::process` is the one function both hosts call per block (the
//! cpal callback natively, the worklet's `process()` in the browser). Every
//! buffer, pool and table is allocated in `Engine::with_config`; `process`
//! and everything it calls only move POD values, never allocate, lock,
//! touch `Rc` or call a closure (proven by the `alloc_probe` render tests).
//!
//! Order per call: (1) control records under the 16.1 install credit
//! (`SlotControl`, cell records, live notes, releases, installs, retires);
//! (2) events due in the block from the ring — a start lands on frame
//! `round((time - block_start) * sr)`, a late event on frame 0 (counted), a
//! stale-generation event is dropped at dequeue; (3) voices, orbit delays,
//! buses, master; (4) retirement at refcount zero, counters and analysis
//! cells. Output is interleaved stereo (`out[2i]`, `out[2i + 1]`).

use crate::dsp::arena::{
    decode_graph, Fault, FaultCode, GraphKind, SampleStore, StoreKind, Templates, DEFERRED_MAX,
    INSTALL_BYTES_PER_QUANTUM,
};
use crate::dsp::bus::{BusGraph, BusTemplate, OrbitDelay};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::{CellId, CellRead};
use crate::dsp::effects::prim::pan_gains;
use crate::dsp::effects::FxCtx;
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::{BusId, InstId, NODE_CAP};
use crate::dsp::release::{TagMap, Tombstones};
use crate::dsp::ring::{
    push_garbage, AckProducer, Budget, ControlSource, Garbage, NativeInstall, Producer, Record,
};
use crate::dsp::ugen::{BuildEnv, RawGraph, Template, BANK};
use crate::dsp::voice::{self, event_ctl, resolve, RenderCtx, SlotGens, VoicePool, BUS};
use crate::host::caps::HostSigs;
use crate::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg, SlotControlAck, VoiceTag};
use crate::sched::slots::{CtlId, SlotId};

pub use crate::dsp::ring::{CellStore, Counters, EngineConfig, EngineIo};

const DELAYTIME: CtlId = CtlId::new(39);
const DELAYFEEDBACK: CtlId = CtlId::new(40);
const ROOM: CtlId = CtlId::new(44);
const SIZE: CtlId = CtlId::new(45);
const MAX_SLOTS: usize = 256;
const MAX_DEFERRED_STARTS: usize = 16;
const MAX_RETIRING_CELLS: usize = 64;
const FAULTS: usize = 64;
const PUBLISH_CELLS: usize = 16;

/// The audio engine.
pub struct Engine {
    cfg: EngineConfig,
    sr: f32,
    frame: u64,
    store: SampleStore,
    fft: Fft,
    templates: Templates,
    raw: Box<RawGraph>,
    bus_tmp: Box<BusTemplate>,
    pool: VoicePool,
    tags: TagMap,
    tombs: Tombstones,
    gens: SlotGens,
    buses: BusGraph,
    orbits: Box<[OrbitDelay]>,
    pending: Box<[AudioEvent]>,
    n_pending: usize,
    deferred: [(Option<AudioEvent>, Option<VoiceTag>); MAX_DEFERRED_STARTS],
    bufs: Box<[f32]>,
    vout: Box<[f32]>,
    tmp: Box<[f32]>,
    dry: Box<[f32]>,
    fx_scratch: Box<[f32]>,
    mix_l: Box<[f32]>,
    mix_r: Box<[f32]>,
    analysis: Box<[f32]>,
    shadow: Box<[f32]>,
    publish_at: usize,
    master_ring: Box<[f32]>,
    ring_pos: usize,
    sigs: HostSigs,
    counters: Counters,
    published: Counters,
    faults: [Option<Fault>; FAULTS],
    retiring_cells: [Option<(CellId, u32)>; MAX_RETIRING_CELLS],
    blocks: u64,
}

impl Engine {
    /// An engine with the default capacities for `caps` (12.8.9).
    #[must_use]
    pub fn new(caps: &CapabilitySet, sample_rate: f32, max_block: usize, store: StoreKind) -> Self {
        Self::with_config(EngineConfig::new(caps, sample_rate, max_block, store))
    }

    /// Allocates everything the callback will ever use.
    #[must_use]
    pub fn with_config(cfg: EngineConfig) -> Self {
        let sr = cfg.sample_rate;
        let mb = cfg.max_block;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let secs = |s: f32| (s.max(0.0) * sr) as usize;
        let bus_mem = secs(cfg.bus_seconds) + crate::dsp::granular::effect_mem_len(sr, &cfg.caps);
        let store = SampleStore::new(cfg.store);
        let arena_tier = matches!(cfg.store, StoreKind::Arena { .. });
        let voices = usize::from(cfg.caps.max_voices.max(1));
        let counters = Counters {
            memory_capacity: store.capacity_bytes(),
            ..Counters::default()
        };
        let silent = AudioEvent::new(0.0, SlotId::new(0), 0, InstId::new(0));
        Self {
            sr,
            frame: 0,
            fft: Fft::new(FFT_SIZE),
            templates: Templates::new(cfg.template_slots, arena_tier),
            raw: RawGraph::boxed(),
            bus_tmp: Box::new(BusTemplate::new()),
            pool: VoicePool::new(voices, secs(cfg.voice_seconds)),
            tags: TagMap::new(voices),
            tombs: Tombstones::new(),
            gens: SlotGens::new(MAX_SLOTS),
            buses: BusGraph::new(cfg.bus_slots, mb, bus_mem, &NoCells, sr, &cfg.caps),
            orbits: (0..cfg.orbits.max(1))
                .map(|_| OrbitDelay::new(secs(cfg.orbit_delay_seconds), mb))
                .collect(),
            pending: vec![silent; cfg.event_capacity.max(1)].into_boxed_slice(),
            n_pending: 0,
            deferred: [(None, None); MAX_DEFERRED_STARTS],
            bufs: vec![0.0; NODE_CAP * mb].into_boxed_slice(),
            vout: vec![0.0; mb].into_boxed_slice(),
            tmp: vec![0.0; 2 * mb].into_boxed_slice(),
            dry: vec![0.0; 2 * mb].into_boxed_slice(),
            fx_scratch: vec![0.0; (4 * mb).max(4 * FFT_SIZE)].into_boxed_slice(),
            mix_l: vec![0.0; mb].into_boxed_slice(),
            mix_r: vec![0.0; mb].into_boxed_slice(),
            analysis: vec![0.0; cfg.analysis_cells].into_boxed_slice(),
            shadow: vec![0.0; cfg.analysis_cells].into_boxed_slice(),
            publish_at: 0,
            master_ring: vec![0.0; FFT_SIZE].into_boxed_slice(),
            ring_pos: 0,
            sigs: HostSigs::default(),
            counters,
            published: counters,
            faults: [None; FAULTS],
            retiring_cells: [None; MAX_RETIRING_CELLS],
            blocks: 0,
            store,
            cfg,
        }
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }

    /// The counters.
    #[must_use]
    pub fn counters(&self) -> &Counters {
        &self.counters
    }

    /// Seconds rendered so far (the native `host_now`).
    #[must_use]
    pub fn now(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let t = self.frame as f64 / f64::from(self.sr);
        t
    }

    /// The resource store.
    #[must_use]
    pub fn store(&self) -> &SampleStore {
        &self.store
    }

    /// The analysis cell table analyzers write.
    #[must_use]
    pub fn analysis(&self) -> &[f32] {
        &self.analysis
    }

    /// The host signals (`amp`, 8 `fft` bands of the master).
    #[must_use]
    pub fn host_sigs(&self) -> HostSigs {
        self.sigs
    }

    /// Sounding voices.
    #[must_use]
    pub fn active_voices(&self) -> usize {
        self.pool.active()
    }

    /// The voice pool (tests and probes).
    #[must_use]
    pub fn voices(&self) -> &VoicePool {
        &self.pool
    }

    /// The bus graph (tests and probes).
    #[must_use]
    pub fn buses(&self) -> &BusGraph {
        &self.buses
    }

    /// The live template of an instrument.
    #[must_use]
    pub fn template(&self, inst: InstId) -> Option<&Template> {
        self.templates
            .live(inst)
            .and_then(|i| self.templates.get(i))
    }

    /// The compilation environment of this engine's templates.
    #[must_use]
    pub fn build_env(&self) -> BuildEnv {
        BuildEnv {
            sr: self.sr,
            caps: self.cfg.caps,
            voice_mem: self.pool.voices.first().map_or(0, voice::Voice::mem_len),
        }
    }

    /// Takes the oldest install fault (the host reports its diagnostic).
    pub fn pop_fault(&mut self) -> Option<Fault> {
        let i = self.faults.iter().position(Option::is_some)?;
        let f = self.faults[i].take();
        self.faults[i..].rotate_left(1);
        f
    }

    fn fault(&mut self, code: FaultCode, resource: u32) {
        if let Some(slot) = self.faults.iter_mut().find(|f| f.is_none()) {
            *slot = Some(Fault { code, resource });
        }
    }

    /// Renders one callback: `frames` interleaved stereo frames into `out`
    /// (longer callbacks run in `max_block` pieces).
    pub fn process<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        out: &mut [f32],
        frames: usize,
    ) {
        let frames = frames.min(out.len() / 2);
        self.controls(io);
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(self.cfg.max_block);
            self.block(io, &mut out[2 * done..2 * (done + n)], n);
            done += n;
        }
        self.retire(io);
        self.publish(io);
        self.blocks += 1;
    }

    fn controls<C: CellStore, S: ControlSource>(&mut self, io: &mut EngineIo<'_, C, S>) {
        let mut credit = INSTALL_BYTES_PER_QUANTUM;
        let mut batch = true;
        let mut copied = 0;
        while let Some(rec) = io.controls.next(Budget {
            install_bytes: credit,
            batch,
        }) {
            let cost = rec.install_bytes().min(credit);
            credit -= cost;
            copied += cost;
            if matches!(rec, Record::Batch(_)) {
                batch = false;
            }
            self.record(rec, io.cells, io.acks, io.garbage.as_deref_mut());
        }
        while io.controls.held() > DEFERRED_MAX {
            match io.controls.drop_held() {
                Some(res) => self.fault(FaultCode::InstallQueueOverflow, res),
                None => break,
            }
        }
        self.counters.bytes_copied_quantum = copied;
        self.counters.bytes_copied_max = self.counters.bytes_copied_max.max(copied);
        self.counters.bytes_copied_total += copied as u64;
    }

    fn record<C: CellStore>(
        &mut self,
        rec: Record<'_>,
        cells: &mut C,
        acks: &mut AckProducer,
        garbage: Option<&mut Producer<Garbage>>,
    ) {
        let mut ack = |m: HostMsg| {
            let _ = acks.push(m);
        };
        match rec {
            Record::Msg(CtlMsg::SlotControl(c)) => {
                self.gens.control(c, &mut self.pool.voices, self.sr);
                ack(HostMsg::SlotControlAck(SlotControlAck {
                    slot: c.slot,
                    gen: c.new_gen,
                }));
            }
            Record::Msg(CtlMsg::CellInit { cell, epoch, value }) => {
                if let Some(m) = cells.init(cell, epoch, value) {
                    ack(m);
                }
            }
            Record::Batch(view) => {
                if let Some(m) = cells.batch(view.seq, &view) {
                    ack(m);
                }
            }
            Record::Msg(CtlMsg::CellRetire { cell, epoch }) => {
                if cells.retire(cell, epoch) {
                    if let Some(s) = self.retiring_cells.iter_mut().find(|s| s.is_none()) {
                        *s = Some((cell, epoch));
                    }
                }
            }
            Record::Msg(CtlMsg::LiveNoteOn { tag, ev }) => {
                if self.tombs.take(tag) {
                    self.counters.dropped += 1;
                } else {
                    self.start(&ev, Some(tag), 0, cells);
                }
            }
            Record::Msg(CtlMsg::VoiceRelease { tag }) => {
                if let Some(v) = self.tags.release(tag) {
                    self.pool.voices[v as usize].release();
                } else if let Some(d) = self.deferred.iter_mut().find(|d| d.1 == Some(tag)) {
                    *d = (None, None);
                } else {
                    self.tombs.push(tag);
                }
            }
            Record::Msg(CtlMsg::GraphRetire { id }) => {
                self.templates.retire(id);
                self.buses.retire(id);
            }
            Record::Msg(CtlMsg::SampleRetire { resource }) => {
                self.store.retire(resource);
            }
            Record::Msg(
                CtlMsg::CellBatch { .. } | CtlMsg::GraphInstall { .. } | CtlMsg::SampleSlice { .. },
            ) => {}
            Record::SampleBegin {
                resource,
                gen,
                frames,
                channels,
                rate,
            } => {
                if let Err(code) = self
                    .store
                    .begin(resource, gen, frames as usize, channels, rate)
                {
                    self.fault(code, resource);
                }
            }
            Record::Slice {
                resource,
                offset,
                data,
            } => match self.store.write_slice(resource, offset as usize, data) {
                Ok(done) => {
                    ack(HostMsg::SliceOk { resource, offset });
                    if let Some(gen) = done {
                        ack(HostMsg::Installed { resource, gen });
                    }
                }
                Err(code) => self.fault(code, resource),
            },
            Record::Graph { id, gen, bytes } => match self.install_bytes(id, gen, bytes, cells) {
                Ok(()) => ack(HostMsg::Installed { resource: id, gen }),
                Err(code) => self.fault(code, id),
            },
            Record::Native(n) => {
                if let Some(m) = self.install_native(n, cells, garbage) {
                    ack(m);
                }
            }
        }
    }

    fn install_bytes<C: CellStore>(
        &mut self,
        id: u32,
        gen: u32,
        bytes: &[u8],
        cells: &C,
    ) -> Result<(), FaultCode> {
        match decode_graph(bytes, &mut self.raw, &mut self.bus_tmp)? {
            GraphKind::Inst => {
                let env = self.build_env();
                self.templates.build(&self.raw, &env, id, gen)
            }
            kind => {
                let t = *self.bus_tmp;
                let master = kind == GraphKind::Master;
                self.buses
                    .install(&t, master, id, gen, cells, self.sr, &self.cfg.caps)
                    .then_some(())
                    .ok_or(FaultCode::BadResource)
            }
        }
    }

    fn install_native<C: CellStore>(
        &mut self,
        n: NativeInstall,
        cells: &C,
        garbage: Option<&mut Producer<Garbage>>,
    ) -> Option<HostMsg> {
        let (resource, gen, ok) = match n {
            NativeInstall::Inst {
                resource,
                gen,
                template,
            } => match self.templates.adopt(template, resource, gen) {
                Ok(()) => (resource, gen, true),
                Err(t) => {
                    push_garbage(garbage, Garbage::Template(t));
                    (resource, gen, false)
                }
            },
            NativeInstall::Bus {
                resource,
                gen,
                master,
                template,
            } => {
                let caps = self.cfg.caps;
                let ok = self
                    .buses
                    .install(&template, master, resource, gen, cells, self.sr, &caps);
                push_garbage(garbage, Garbage::Bus(template));
                (resource, gen, ok)
            }
            NativeInstall::Sample {
                resource,
                gen,
                data,
            } => match self.store.install_arc(resource, gen, data) {
                Ok(()) => (resource, gen, true),
                Err(data) => {
                    push_garbage(garbage, Garbage::Sample(data));
                    (resource, gen, false)
                }
            },
        };
        if ok {
            Some(HostMsg::Installed { resource, gen })
        } else {
            self.fault(FaultCode::BadResource, resource);
            None
        }
    }

    /// Starts a voice for `ev` at frame `delay` of the next block; on a full
    /// pool the oldest open input voice is stolen (short gate, counted) and
    /// the start waits for its slot.
    fn start<C: CellRead + ?Sized>(
        &mut self,
        ev: &AudioEvent,
        tag: Option<VoiceTag>,
        delay: usize,
        cells: &C,
    ) {
        let Some(ts) = self.templates.live(ev.inst) else {
            self.counters.dropped += 1;
            return;
        };
        let Some(vi) = self.pool.free() else {
            let free_wait = self.deferred.iter().position(|d| d.0.is_none());
            match (self.tags.oldest(), free_wait) {
                (Some((_, victim)), Some(w)) => {
                    self.pool.voices[victim as usize].short_gate(self.sr);
                    self.tags.remove_voice(victim);
                    self.counters.stolen += 1;
                    self.deferred[w] = (Some(*ev), tag);
                }
                _ => self.counters.dropped += 1,
            }
            return;
        };
        let get = |id| event_ctl(ev, id).map(|c| resolve(c, cells));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bus = self
            .buses
            .route(get(BUS).map(|b| BusId::new(b.max(0.0) as u32)));
        let seed = self.pool.seed();
        let Some(t) = self.templates.get(ts) else {
            return;
        };
        #[allow(clippy::cast_precision_loss)]
        let start = self.now() + delay as f64 / f64::from(self.sr);
        let v = &mut self.pool.voices[vi];
        v.start(
            t,
            ts,
            ev,
            tag,
            delay,
            start,
            bus,
            cells,
            self.sr,
            &self.cfg.caps,
            seed,
        );
        v.orbit = v.orbit.min(self.orbits.len() - 1);
        self.orbits[v.orbit].set(get(DELAYTIME), get(DELAYFEEDBACK));
        let slot = &mut self.buses.slots[bus];
        slot.users += 1;
        slot.set_room(get(ROOM), get(SIZE));
        if let Some(tag) = tag {
            let _ = self.tags.insert(tag, u32::try_from(vi).unwrap_or(u32::MAX));
        }
    }

    /// Dequeues due events and renders one block of `n <= max_block`.
    fn block<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        out: &mut [f32],
        n: usize,
    ) {
        for i in 0..MAX_DEFERRED_STARTS {
            if self.pool.free().is_none() {
                break;
            }
            if let (Some(ev), tag) = self.deferred[i] {
                self.deferred[i] = (None, None);
                self.start(&ev, tag, 0, io.cells);
            }
        }
        while self.n_pending < self.pending.len() {
            let Some(ev) = io.events.pop() else {
                break;
            };
            self.pending[self.n_pending] = ev;
            self.n_pending += 1;
        }
        let start = self.now();
        #[allow(clippy::cast_precision_loss)]
        let end = start + n as f64 / f64::from(self.sr);
        let mut k = 0;
        while k < self.n_pending {
            let ev = self.pending[k];
            if ev.time >= end {
                k += 1;
                continue;
            }
            self.pending.copy_within(k + 1..self.n_pending, k);
            self.n_pending -= 1;
            if !self.gens.admit(&ev) {
                self.counters.dropped += 1;
                continue;
            }
            let delay = if ev.time < start {
                self.counters.late += 1;
                0
            } else {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let f = ((ev.time - start) * f64::from(self.sr)).round() as usize;
                f.min(n - 1)
            };
            self.start(&ev, None, delay, io.cells);
        }
        self.render(io.cells, out, n);
        self.frame += n as u64;
    }

    fn render<C: CellRead + ?Sized>(&mut self, cells: &C, out: &mut [f32], n: usize) {
        self.buses.clear(n);
        for o in self.orbits.iter_mut() {
            o.clear(n);
        }
        let mut stats = self.counters.fx;
        let Self {
            pool,
            templates,
            buses,
            orbits,
            tags,
            bufs,
            vout,
            tmp,
            dry,
            fx_scratch,
            analysis,
            store,
            fft,
            cfg,
            sr,
            mix_l,
            mix_r,
            ..
        } = self;
        let mut rc = RenderCtx {
            sr: *sr,
            max_block: cfg.max_block,
            bufs,
            out: vout,
            tmp,
            dry,
            cells,
            store,
            fx: FxCtx {
                sr: *sr,
                store,
                fft,
                caps: &cfg.caps,
                scratch: fx_scratch,
                analysis,
                stats: &mut stats,
            },
        };
        for (vi, v) in pool.voices.iter_mut().enumerate() {
            if !v.active {
                continue;
            }
            let Some(t) = templates.get(v.tmpl) else {
                v.active = false;
                continue;
            };
            let (off, ended) = voice::render(v, t, n, &mut rc);
            let (gl, gr) = pan_gains(v.pan);
            let bus = &mut buses.slots[v.bus];
            let orbit = &mut orbits[v.orbit.min(orbits.len() - 1)];
            for (k, y) in rc.out[..n - off].iter().enumerate() {
                bus.l[off + k] += y * gl;
                bus.r[off + k] += y * gr;
                orbit.in_l[off + k] += y * gl * v.delay_send;
                orbit.in_r[off + k] += y * gr * v.delay_send;
            }
            if ended {
                v.active = false;
                bus.users = bus.users.saturating_sub(1);
                tags.remove_voice(u32::try_from(vi).unwrap_or(u32::MAX));
            }
        }
        let m = buses.master();
        for o in orbits.iter_mut() {
            let slot = &mut buses.slots[m];
            o.run(n, *sr, &mut slot.l, &mut slot.r);
        }
        buses.render(
            n,
            cells,
            rc.dry,
            &mut rc.fx,
            &mut mix_l[..n],
            &mut mix_r[..n],
        );
        self.counters.fx = stats;
        let mut energy = 0.0;
        for k in 0..n {
            let (l, r) = (guard(self.mix_l[k]), guard(self.mix_r[k]));
            if let Some(o) = out.get_mut(2 * k) {
                *o = l;
            }
            if let Some(o) = out.get_mut(2 * k + 1) {
                *o = r;
            }
            let mono = 0.5 * (l + r);
            energy += mono * mono;
            self.master_ring[self.ring_pos] = mono;
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;
        }
        #[allow(clippy::cast_precision_loss)]
        let rms = (energy / n.max(1) as f32).sqrt();
        self.sigs.amp += 0.2 * (rms - self.sigs.amp);
    }

    /// Frees resources nobody references and acks `Retired` (16.1).
    fn retire<C: CellStore, S: ControlSource>(&mut self, io: &mut EngineIo<'_, C, S>) {
        while let Some((resource, boxed)) = self.templates.collect(&self.pool.voices) {
            if let Some(t) = boxed {
                push_garbage(io.garbage.as_deref_mut(), Garbage::Template(t));
            }
            let _ = io.acks.push(HostMsg::Retired { resource });
        }
        let mut freed = [0u32; 8];
        let n = self.buses.collect(&mut freed);
        for &resource in &freed[..n] {
            let _ = io.acks.push(HostMsg::Retired { resource });
        }
        let mut ids = [0u32; 16];
        let mut n = 0;
        for id in self.store.retiring().take(ids.len()) {
            ids[n] = id;
            n += 1;
        }
        let pending = &self.pending[..self.n_pending];
        for &id in &ids[..n] {
            let by_voice = self
                .pool
                .voices
                .iter()
                .any(|v| self.templates.get(v.tmpl).is_some_and(|t| v.uses(t, id)));
            #[allow(clippy::cast_precision_loss)]
            let by_event = pending
                .iter()
                .any(|e| matches!(event_ctl(e, BANK), Some(Ctl::Const(b)) if b == id as f32));
            if by_voice || by_event || self.templates.references(id) || self.buses.references(id) {
                continue;
            }
            if let Some(arc) = self.store.release(id) {
                if let Some(a) = arc {
                    push_garbage(io.garbage.as_deref_mut(), Garbage::Sample(a));
                }
                let _ = io.acks.push(HostMsg::Retired { resource: id });
            }
        }
        for i in 0..MAX_RETIRING_CELLS {
            let Some((cell, epoch)) = self.retiring_cells[i] else {
                continue;
            };
            let in_use = pending
                .iter()
                .any(|e| e.controls().iter().any(|(_, c)| *c == Ctl::Cell(cell)))
                || self.templates.reads_cell(cell)
                || self.buses.reads_cell(cell);
            if !in_use && io.cells.retired(cell) {
                self.retiring_cells[i] = None;
                let _ = io.acks.push(HostMsg::CellRetired { cell, epoch });
            }
        }
    }

    /// Publishes changed counters, a bounded slice of changed analysis
    /// cells, and every 8 calls the master `fft` bands.
    fn publish<C: CellStore, S: ControlSource>(&mut self, io: &mut EngineIo<'_, C, S>) {
        let (c, p) = (&self.counters, &self.published);
        let changed = (c.late, c.dropped, c.stolen, c.fx.grains_skipped)
            != (p.late, p.dropped, p.stolen, p.fx.grains_skipped);
        let msg = HostMsg::Counters {
            late: c.late,
            dropped: c.dropped,
            stolen: c.stolen,
            skipped: c.fx.grains_skipped,
        };
        if changed && io.acks.push(msg).is_ok() {
            self.published = self.counters;
        }
        let len = self.analysis.len();
        for _ in 0..PUBLISH_CELLS.min(len) {
            let i = self.publish_at;
            self.publish_at = (self.publish_at + 1) % len;
            if self.analysis[i].to_bits() != self.shadow[i].to_bits() {
                let id = u32::try_from(i).unwrap_or(u32::MAX);
                let value = self.analysis[i];
                if io.acks.push(HostMsg::AnalysisCell { id, value }).is_ok() {
                    self.shadow[i] = value;
                }
            }
        }
        if self.blocks % 8 == 0 {
            self.sigs.fft = self
                .fft
                .octave_bands(&self.master_ring, &mut self.fx_scratch);
        }
    }
}

fn guard(x: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        0.0
    }
}

/// A cell table with no cells (construction-time configuration).
struct NoCells;

impl CellRead for NoCells {
    fn get(&self, _cell: CellId) -> f32 {
        0.0
    }
}
