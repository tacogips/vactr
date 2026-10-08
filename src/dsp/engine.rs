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
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::{BusId, InstId};
use crate::dsp::ramp::{CellRamps, RampedCells};
use crate::dsp::release::{TagMap, Tombstones};
use crate::dsp::ring::{
    push_garbage, AckProducer, Budget, ControlSource, Garbage, NativeInstall, Producer, Record,
};
use crate::dsp::ugen::{BuildEnv, RawGraph, Template, BANK};
use crate::dsp::voice::{self, event_ctl, resolve, SlotGens, VoicePool, BUS};
use crate::host::caps::HostSigs;
use crate::host::wire::{AudioEvent, Ctl, CtlMsg, HostMsg, SlotControlAck, VoiceTag};
use crate::sched::slots::{CtlId, SlotId};

pub use crate::dsp::ring::{CellStore, Counters, EngineConfig, EngineIo};

mod occupancy;
mod output;
mod render;
mod song;
mod song_queue;
mod song_runtime;
pub use song::{SongFrameRegion, SongLeaseState, SongResourceStager, SongStagingConfig};

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
    output: output::OutputStage,
    ramps: CellRamps,
    pending: Box<[AudioEvent]>,
    n_pending: usize,
    deferred: [(Option<AudioEvent>, Option<VoiceTag>); MAX_DEFERRED_STARTS],
    bufs: Box<[f32]>,
    vout: Box<[f32]>,
    vout_r: Box<[f32]>,
    vout_3: Box<[f32]>,
    vout_4: Box<[f32]>,
    tmp: Box<[f32]>,
    dry: Box<[f32]>,
    fx_scratch: Box<[f32]>,
    mix_l: Box<[f32]>,
    mix_r: Box<[f32]>,
    mix_3: Box<[f32]>,
    mix_4: Box<[f32]>,
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
    song_validation: Option<(crate::dsp::ring::NativeSongInstall, usize)>,
    song_return_key: Option<(crate::song::routing::SongLeaseKey, bool)>,
    song_stager: Option<SongResourceStager>,
    song_runtime: Option<crate::dsp::song::SongRuntime>,
    song_followup: Option<HostMsg>,
    song_ack: Option<HostMsg>,
    pending_legacy_install: Option<Garbage>,
    pending_song_install: Option<crate::dsp::ring::NativeSongInstall>,
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
        assert!(cfg.validate().is_ok(), "unsupported DSP host configuration");
        Self::allocate(cfg)
    }

    /// Validates a host configuration before allocating callback state.
    ///
    /// # Errors
    /// An unsupported rate, block size or state budget.
    pub fn try_with_config(cfg: EngineConfig) -> Result<Self, crate::dsp::ring::ConfigError> {
        cfg.validate()?;
        Ok(Self::allocate(cfg))
    }

    fn allocate(cfg: EngineConfig) -> Self {
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
            song_validation: None,
            song_return_key: None,
            song_stager: None,
            song_runtime: None,
            song_followup: None,
            song_ack: None,
            pending_legacy_install: None,
            pending_song_install: None,
            fft: Fft::new(FFT_SIZE),
            templates: Templates::new(cfg.template_slots, arena_tier),
            raw: RawGraph::boxed(),
            bus_tmp: Box::new(BusTemplate::new()),
            pool: VoicePool::new(voices, secs(cfg.voice_seconds)),
            tags: TagMap::new(voices),
            tombs: Tombstones::new(),
            gens: SlotGens::new(MAX_SLOTS),
            buses: BusGraph::new_with_memory_profile(
                cfg.bus_slots,
                mb,
                bus_mem,
                cfg.song_bus_memory,
                &NoCells,
                sr,
                &cfg.caps,
            )
            .expect("validated constructor bus geometry"),
            orbits: (0..cfg.orbits.max(1))
                .map(|_| OrbitDelay::new(secs(cfg.orbit_delay_seconds), mb))
                .collect(),
            output: output::OutputStage::new(),
            ramps: CellRamps::new(cfg.analysis_cells),
            pending: vec![silent; cfg.event_capacity.max(1)].into_boxed_slice(),
            n_pending: 0,
            deferred: [(None, None); MAX_DEFERRED_STARTS],
            bufs: vec![
                0.0;
                (crate::dsp::graph::MAX_AUDIO_BUFFERS + crate::dsp::graph::DISCARD_SLICES)
                    * mb
            ]
            .into_boxed_slice(),
            vout: vec![0.0; mb].into_boxed_slice(),
            vout_r: vec![0.0; mb].into_boxed_slice(),
            vout_3: vec![0.0; mb].into_boxed_slice(),
            vout_4: vec![0.0; mb].into_boxed_slice(),
            tmp: vec![0.0; 2 * mb].into_boxed_slice(),
            dry: vec![0.0; 2 * mb].into_boxed_slice(),
            fx_scratch: vec![0.0; (4 * mb).max(4 * FFT_SIZE)].into_boxed_slice(),
            mix_l: vec![0.0; mb].into_boxed_slice(),
            mix_r: vec![0.0; mb].into_boxed_slice(),
            mix_3: vec![0.0; mb].into_boxed_slice(),
            mix_4: vec![0.0; mb].into_boxed_slice(),
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

    #[cfg(test)]
    pub(crate) fn output_memory_is_clear(&self) -> bool {
        let (orbits, buses) = self.output_memory_status();
        orbits.into_iter().all(|clear| clear) && buses.into_iter().all(|clear| clear)
    }

    #[cfg(test)]
    pub(crate) fn output_memory_status(&self) -> (Vec<bool>, Vec<bool>) {
        (
            self.orbits
                .iter()
                .map(crate::dsp::bus::OrbitDelay::memory_is_clear)
                .collect(),
            self.buses
                .slots
                .iter()
                .filter(|slot| slot.song_key.is_none())
                .map(crate::dsp::bus::BusSlot::memory_is_clear)
                .collect(),
        )
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

    #[cfg(test)]
    pub(crate) fn has_template(&self, id: crate::dsp::graph::InstId) -> bool {
        self.templates.live(id).is_some()
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
        self.process_channels(io, None, out, frames, 2);
    }

    /// Renders stereo output while adding exact interleaved L/R input to the
    /// master bus before its effects. Invalid input is silenced and faulted.
    pub fn process_with_input<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        input_interleaved: &[f32],
        out: &mut [f32],
        frames: usize,
    ) {
        self.process_channels(io, Some(input_interleaved), out, frames, 2);
    }

    /// Renders four interleaved output lanes. Channels one/two pass through
    /// the stereo bus/master; channels three/four are direct post-voice stems.
    pub fn process_four<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        out: &mut [f32],
        frames: usize,
    ) {
        self.process_channels(io, None, out, frames, 4);
    }

    /// Renders stereo master plus two direct stems with stereo input routed
    /// only through the master bus, never into direct stems.
    pub fn process_four_with_input<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        input_interleaved: &[f32],
        out: &mut [f32],
        frames: usize,
    ) {
        self.process_channels(io, Some(input_interleaved), out, frames, 4);
    }

    fn process_channels<C: CellStore, S: ControlSource>(
        &mut self,
        io: &mut EngineIo<'_, C, S>,
        input: Option<&[f32]>,
        out: &mut [f32],
        frames: usize,
        channels: usize,
    ) {
        if usize::from(self.cfg.output_channels) != channels {
            out.fill(0.0);
            self.fault(FaultCode::OutputChannels, 0);
            return;
        }
        let frames = frames.min(out.len() / channels);
        let input = input.and_then(|samples| {
            if samples.len() == frames * 2 && samples.iter().all(|v| v.is_finite()) {
                Some(samples)
            } else {
                self.fault(FaultCode::InputBuffer, 0);
                None
            }
        });
        self.controls(io);
        let mut done = 0;
        while done < frames {
            let _ = self.apply_song_runtime_frame(self.frame, io.acks);
            let mut n = (frames - done).min(self.cfg.max_block);
            if let Some(frame) = self.next_song_runtime_frame(self.frame.saturating_add(n as u64)) {
                if frame > self.frame {
                    n = usize::try_from(frame - self.frame).unwrap_or(n).min(n);
                }
            }
            let input_block = input.map(|samples| &samples[2 * done..2 * (done + n)]);
            self.block(
                io,
                input_block,
                &mut out[channels * done..channels * (done + n)],
                n,
                channels,
            );
            done += n;
        }
        self.finish_song_deadlines(self.frame);
        self.retire_song_runtime(self.frame);
        self.reap_returned_song_epochs();
        self.retire(io);
        if let Some((phase, frame)) = self.output.pending_report {
            if io
                .acks
                .push_critical(HostMsg::OutputState { phase, frame })
                .is_ok()
            {
                self.output.last_reported = phase;
                self.output.pending_report = None;
            }
        }
        self.publish(io);
        self.blocks += 1;
    }

    fn controls<C: CellStore, S: ControlSource>(&mut self, io: &mut EngineIo<'_, C, S>) {
        if !self.return_legacy_install(io.garbage.as_deref_mut()) {
            return;
        }
        if !self.return_song_install(io.garbage.as_deref_mut()) {
            return;
        }
        if let Some(ack) = self.song_ack.take() {
            if let Err(ack) = io.acks.push_critical(ack) {
                self.song_ack = Some(ack);
                return;
            }
        }
        if let Some(message) = self.song_followup.take() {
            if let Err(message) = io.acks.push_critical(message) {
                self.song_followup = Some(message);
                return;
            }
        }
        self.flush_runtime_receipts(io.acks);
        if self
            .song_runtime
            .as_ref()
            .is_some_and(|r| r.has_pending_receipts())
        {
            return;
        }
        if !self.pump_song_staging(io.acks, io.garbage.as_deref_mut()) {
            return;
        }
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
            if self.pending_legacy_install.is_some()
                || self.song_ack.is_some()
                || self.pending_song_install.is_some()
                || self.song_validation.is_some()
                || self
                    .song_runtime
                    .as_ref()
                    .is_some_and(|r| r.has_pending_receipts())
            {
                break;
            }
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

    fn return_song_install(&mut self, garbage: Option<&mut Producer<Garbage>>) -> bool {
        let Some(install) = self.pending_song_install.take() else {
            return true;
        };
        let Some(queue) = garbage else {
            self.pending_song_install = Some(install);
            return false;
        };
        match queue.push(Garbage::SongInstall(install)) {
            Ok(()) => true,
            Err(Garbage::SongInstall(install)) => {
                self.pending_song_install = Some(install);
                false
            }
            Err(_) => unreachable!("song install garbage identity"),
        }
    }

    fn record<C: CellStore>(
        &mut self,
        rec: Record<'_>,
        cells: &mut C,
        acks: &mut AckProducer,
        garbage: Option<&mut Producer<Garbage>>,
    ) {
        if let Record::Msg(CtlMsg::Song(command)) = rec {
            if self.song_runtime_record(command, acks) {
                return;
            }
        }
        let mut ack = |m: HostMsg| {
            let _ = acks.push(m);
        };
        match rec {
            rec @ (Record::SongNative(_)
            | Record::SongGraph { .. }
            | Record::SongSampleBegin { .. }
            | Record::SongSlice { .. }
            | Record::Msg(CtlMsg::Song(_))) => self.song_record(rec, acks, garbage),
            Record::Msg(CtlMsg::SlotControl(c)) => {
                self.gens.control(c, &mut [], self.sr);
                for (index, voice) in self.pool.voices.iter_mut().enumerate() {
                    if self
                        .song_runtime
                        .as_ref()
                        .is_none_or(|r| r.voices[index].is_none())
                    {
                        self.gens.control(c, std::slice::from_mut(voice), self.sr);
                    }
                }
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
                    self.ramps.drop_cell(cell);
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
            Record::Msg(CtlMsg::OutputStop { mode }) => {
                self.output
                    .stop(mode, self.frame, self.sr, self.cfg.cut_fade_seconds);
            }
            Record::Msg(CtlMsg::CellRamp {
                cell,
                epoch,
                seq,
                target,
                frames,
                release,
            }) => {
                if cells.live_epoch(cell).is_some_and(|live| live != epoch) {
                    return;
                }
                let current = RampedCells::new(&self.ramps, cells, self.frame).get(cell);
                if self.ramps.apply(
                    cell, epoch, seq, target, frames, release, self.frame, current,
                ) {
                    ack(HostMsg::CellRampAck { cell, seq });
                } else {
                    self.counters.dropped = self.counters.dropped.saturating_add(1);
                }
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
                if let Err(code) =
                    self.begin_legacy_sample(resource, gen, frames as usize, channels, rate)
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

use crate::song::routing::{SongHostCapacities, SongRejectCode};
impl Engine {
    pub(crate) fn configure_song_staging_with_transport(
        &mut self,
        config: SongStagingConfig,
        measured_ack_slots: u32,
    ) -> Result<(), SongRejectCode> {
        if self.blocks != 0
            || self
                .song_stager
                .as_ref()
                .is_some_and(|s| !s.preparations.is_empty())
        {
            return Err(SongRejectCode::NotReady);
        }
        if measured_ack_slots == 0 || self.cfg.output_channels != 2 {
            return Err(SongRejectCode::Capacity);
        }
        let bus_regions = self.buses.song_regions();
        let voice_regions: Vec<_> = self
            .pool
            .voices
            .iter()
            .enumerate()
            .map(|(slot, _)| {
                let frames = u64::try_from(self.pool.voices[slot].mem_len()).unwrap_or(0);
                SongFrameRegion {
                    slot: u32::try_from(slot).unwrap_or(u32::MAX),
                    offset: 0,
                    frames,
                }
            })
            .collect();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sample_rate = self.sr as u32;
        let actual = SongHostCapacities {
            sample_rate,
            cell_slots: config.control_slots,
            voice_slots: u32::try_from(self.pool.voices.len())
                .map_err(|_| SongRejectCode::Capacity)?,
            template_slots: u32::try_from(
                self.templates
                    .slots
                    .iter()
                    .filter(|s| s.state == crate::dsp::bus::SlotState::Free)
                    .count(),
            )
            .map_err(|_| SongRejectCode::Capacity)?,
            bus_slots: u32::try_from(
                self.buses
                    .slots
                    .iter()
                    .filter(|s| s.state == crate::dsp::bus::SlotState::Free)
                    .count(),
            )
            .map_err(|_| SongRejectCode::Capacity)?,
            sample_resources: self.store.song_free_sample_slots(),
            pcm_bytes: self
                .store
                .song_remaining_pcm_with_limit(config.native_pcm_bytes)
                .ok_or(SongRejectCode::Capacity)?,
            voice_frames: voice_regions
                .iter()
                .try_fold(0_u64, |n, r| n.checked_add(r.frames))
                .ok_or(SongRejectCode::Capacity)?,
            bus_frames: bus_regions
                .iter()
                .try_fold(0_u64, |n, r| n.checked_add(r.frames))
                .ok_or(SongRejectCode::Capacity)?,
            ack_slots: measured_ack_slots,
        };
        let stager = SongResourceStager::new(
            config,
            actual,
            bus_regions,
            &voice_regions,
            self.store.song_free_extents(),
        )?;
        let runtime = crate::dsp::song::SongRuntime::new(
            config.preparations as usize,
            config.branches as usize,
            config.leases as usize,
            self.pool.voices.len(),
            self.cfg.event_capacity.max(1),
            config.critical_receipts as usize,
        )?;
        self.store
            .song_configure_native_limit(config.native_pcm_bytes);
        self.song_stager = Some(stager);
        self.song_runtime = Some(runtime);
        Ok(())
    }
}

impl Engine {
    /// Configure before processing using the actual bounded acknowledgment producer.
    /// # Errors
    /// Late construction, missing physical capacity or insufficient transport.
    pub fn configure_song_staging_for_transport(
        &mut self,
        config: SongStagingConfig,
        acknowledgments: &AckProducer,
    ) -> Result<(), SongRejectCode> {
        self.configure_song_staging_with_transport(
            config,
            u32::try_from(acknowledgments.capacity()).map_err(|_| SongRejectCode::Capacity)?,
        )
    }
}

use crate::song::routing::{SongCommand, SongHostAck};
impl Engine {
    pub(super) fn next_song_runtime_frame(&self, end: u64) -> Option<u64> {
        let r = self.song_runtime.as_ref()?;
        r.commands
            .iter()
            .map(|c| c.frame)
            .chain(r.branches.iter().filter(|b| b.muted).map(|b| b.gate.end))
            .chain(
                r.branches
                    .iter()
                    .filter(|b| b.ended)
                    .map(|b| b.config.tail_deadline),
            )
            .chain(r.endpoints.iter().map(|e| e.deadline))
            .chain(r.replacements.iter().flat_map(|x| {
                let a = x.command.activation.frame;
                [a - 64, a, a + 64]
            }))
            .filter(|f| *f > self.frame && *f < end)
            .min()
    }

    /// Schedules a checked activation; Applied is emitted only at its actual frame.
    /// # Errors
    /// Unready preparation, stale epoch, past frame or exhausted bounded queue.
    pub fn activate_song(
        &mut self,
        command: crate::song::routing::SongActivation,
    ) -> Result<(), SongRejectCode> {
        self.queue_song_runtime(SongCommand::Activate(command))
    }
    /// Schedules an instrument gate; actual application is acknowledged separately.
    /// # Errors
    /// Unready preparation, stale epoch, past frame or exhausted bounded queue.
    pub fn apply_song_mute(
        &mut self,
        command: crate::song::routing::SongMute,
    ) -> Result<(), SongRejectCode> {
        self.queue_song_runtime(SongCommand::Mute(command))
    }
}

#[cfg(test)]
impl Engine {
    pub(crate) fn song_analysis_test_value(
        &self,
        bank: crate::song::routing::SongLeaseKey,
        logical: usize,
    ) -> Result<f32, SongRejectCode> {
        let stager = self.song_stager.as_ref().ok_or(SongRejectCode::NotReady)?;
        let region = stager.analysis_bank(bank)?;
        if logical >= region.frames as usize {
            return Err(SongRejectCode::Malformed);
        }
        stager
            .analysis
            .get(region.offset as usize + logical)
            .copied()
            .ok_or(SongRejectCode::Malformed)
    }
}
