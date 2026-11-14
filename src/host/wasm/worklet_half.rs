//! The worklet half: DSP only (design 12.8.10, 16, 16.1).
//!
//! `worklet_init` allocates everything the audio thread will use: the
//! engine with its arena, the inbox slots, the event ring, the ack ring,
//! the staging buffer JS copies records into, the outbox and the report,
//! and records the memory size. `processor.js` then, per render quantum,
//! copies each record it holds into the staging buffer and calls
//! `worklet_inbox` (an event goes to the ring, anything else to an inbox
//! slot; a full inbox returns 0 and JS keeps the record for the next
//! quantum), calls `process`, and posts the outbox. `process` runs
//! `Engine::process`, whose control drain applies the 16.1 install credit,
//! and turns its acks and faults into outbox records. The frame time the
//! worklet posts is `worklet_now`, the engine's rendered-frames clock (16:
//! the worklet is the timebase).
//!
//! The report is a fixed `f64` array the harness reads (`REPORT_LEN`
//! entries; the `R_*` indices below).

use std::cell::RefCell;

use crate::dsp::arena::{FaultCode, StoreKind, DEFAULT_ARENA_BYTES};
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::CellId;
use crate::dsp::engine::{Engine, EngineConfig, EngineIo};
use crate::dsp::ring::{
    AckConsumer, AckProducer, ByteInbox, ControlSource, EventConsumer, EventProducer, EventRing,
    SpscRing, EVENT_CAPACITY, INBOX_SLOTS, INBOX_SLOT_BYTES,
};
use crate::host::wasm::abi::{fix_outbox, outbox_dropped, push_record, TAG_FAULT, TAG_SIGS};
use crate::host::wasm::cells::{ProbeMirror, PROBES, READ_LOG};
use crate::host::wire::{AudioEvent, HostMsg};
use crate::ns::insts::{INST_CELL_BASE, INST_CELL_COUNT};

/// Frames per render quantum (the engine block).
pub const QUANTUM: usize = 128;
/// Largest `process(frames)` call.
const MAX_FRAMES: usize = 4 * QUANTUM;
/// Worklet outbox capacity (acks and faults of one quantum fit easily).
const OUTBOX_BYTES: usize = 64 * 1024;
/// Slice arrivals remembered for the deferred-ack count.
const ARRIVALS: usize = 16;
/// Tagged voices reported.
const TAGGED: usize = 8;
/// A block below this RMS is silent.
const SILENT: f64 = 1.0e-6;

const TAG_AUDIO_EVENT: u8 = 0x01;
const TAG_GRAPH_INSTALL: u8 = 0x16;
const TAG_SAMPLE_SLICE: u8 = 0x18;
/// A slice or graph header before its payload.
const INSTALL_HEAD: usize = 13;

pub const R_NOW: usize = 0;
pub const R_QUANTA: usize = 1;
pub const R_LATE: usize = 2;
pub const R_DROPPED: usize = 3;
pub const R_STOLEN: usize = 4;
pub const R_BYTES_QUANTUM: usize = 5;
/// Max install bytes copied by one `process()` since the last watch reset.
pub const R_BYTES_MAX: usize = 6;
pub const R_BYTES_TOTAL: usize = 7;
pub const R_HELD: usize = 8;
pub const R_DEFERRED_QUANTA: usize = 9;
pub const R_ACTIVE: usize = 10;
pub const R_RETIRED: usize = 11;
pub const R_CELL_RETIRED: usize = 12;
pub const R_MEM_INIT: usize = 13;
pub const R_MEM_NOW: usize = 14;
pub const R_INBOX_REFUSED: usize = 15;
pub const R_RING_FULL: usize = 16;
pub const R_OUTBOX_DROPPED: usize = 17;
pub const R_FAULT_ARENA: usize = 18;
pub const R_FAULT_GRAPH: usize = 19;
pub const R_FAULT_OVERFLOW: usize = 20;
pub const R_FAULT_OTHER: usize = 21;
pub const R_SOUNDING: usize = 22;
/// Silent quanta after the first sounding one since the last watch reset.
pub const R_DROPOUTS: usize = 23;
pub const R_RMS: usize = 24;
pub const R_RMS_MIN: usize = 25;
pub const R_SLICE_ACKS: usize = 26;
/// `SliceOk`s emitted in a later quantum than the slice arrived.
pub const R_DEFERRED_ACKS: usize = 27;
/// Max install payload bytes staged into the inbox between two quanta.
pub const R_STAGED_MAX: usize = 28;
pub const R_INSTALLED: usize = 29;
pub const R_RES_ID: usize = 30;
/// 0 none, 1 installing, 2 live, 3 retiring.
pub const R_RES_STATE: usize = 31;
/// `PROBES` x 8: cell, kind, epoch, value, reads, uninit reads, last read,
/// highest initialized epoch.
pub const R_PROBES: usize = 32;
pub const R_TAGGED_N: usize = R_PROBES + 8 * PROBES;
/// `TAGGED` x 4: slot, pitch, seq, released.
pub const R_TAGGED: usize = R_TAGGED_N + 1;
pub const R_LOG_N: usize = R_TAGGED + 4 * TAGGED;
/// `READ_LOG` x 4: block time, cell, value, live.
pub const R_LOG: usize = R_LOG_N + 1;
pub const REPORT_LEN: usize = R_LOG + 4 * READ_LOG;

struct Worklet {
    engine: Engine,
    cells: ProbeMirror,
    inbox: ByteInbox,
    ev_tx: EventProducer,
    ev_rx: EventConsumer,
    ack_tx: AckProducer,
    ack_rx: AckConsumer,
    staging: Box<[u8]>,
    out: Box<[f32]>,
    input: Box<[f32]>,
    planar: Box<[f32]>,
    report: Box<[f64; REPORT_LEN]>,
    quanta: u64,
    staged_now: usize,
    arrivals: [(u32, u32, u64); ARRIVALS],
    arrival_at: usize,
    sounding: bool,
    probe_res: Option<u32>,
}

thread_local! {
    static WORKLET: RefCell<Option<Worklet>> = const { RefCell::new(None) };
}

fn memory_bytes() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        (core::arch::wasm32::memory_size(0) * 65_536) as f64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0.0
    }
}

/// Builds the worklet: `arena_bytes` 0 = the 64 MB default, `voices` 0 =
/// the browser tier's 64. Returns 1.
#[no_mangle]
pub extern "C" fn worklet_init(sample_rate: f32, arena_bytes: u32, voices: u32) -> u32 {
    init_with_outputs(sample_rate, arena_bytes, voices, 2)
}

/// Opt-in four-channel worklet initialization; the stereo ABI stays intact.
#[no_mangle]
pub extern "C" fn worklet_init_quad(sample_rate: f32, arena_bytes: u32, voices: u32) -> u32 {
    init_with_outputs(sample_rate, arena_bytes, voices, 4)
}

fn init_with_outputs(sample_rate: f32, arena_bytes: u32, voices: u32, channels: u8) -> u32 {
    let mut caps = CapabilitySet::browser();
    if voices > 0 {
        caps.max_voices = u16::try_from(voices).unwrap_or(u16::MAX);
    }
    let bytes = if arena_bytes == 0 {
        DEFAULT_ARENA_BYTES
    } else {
        arena_bytes as usize
    };
    let mut cfg = EngineConfig::new(&caps, sample_rate, QUANTUM, StoreKind::Arena { bytes });
    cfg.output_channels = channels;
    let (ev_tx, ev_rx) = EventRing::split(EVENT_CAPACITY);
    let (ack_tx, ack_rx) = SpscRing::<HostMsg>::split(1024);
    fix_outbox(OUTBOX_BYTES);
    let w = Worklet {
        engine: Engine::with_config(cfg),
        cells: ProbeMirror::new((INST_CELL_BASE + INST_CELL_COUNT) as usize),
        inbox: ByteInbox::new(),
        ev_tx,
        ev_rx,
        ack_tx,
        ack_rx,
        staging: vec![0; INBOX_SLOT_BYTES].into_boxed_slice(),
        out: vec![0.0; 4 * MAX_FRAMES].into_boxed_slice(),
        input: vec![0.0; 2 * MAX_FRAMES].into_boxed_slice(),
        planar: vec![0.0; 4 * MAX_FRAMES].into_boxed_slice(),
        report: Box::new([0.0; REPORT_LEN]),
        quanta: 0,
        staged_now: 0,
        arrivals: [(0, 0, 0); ARRIVALS],
        arrival_at: 0,
        sounding: false,
        probe_res: None,
    };
    WORKLET.with(|s| *s.borrow_mut() = Some(w));
    with(|w| {
        w.report[R_MEM_INIT] = memory_bytes();
        w.report[R_RMS_MIN] = f64::INFINITY;
    });
    1
}

fn with<R: Default>(f: impl FnOnce(&mut Worklet) -> R) -> R {
    WORKLET.with(|s| s.borrow_mut().as_mut().map(f).unwrap_or_default())
}

/// The buffer JS copies one record into before `worklet_inbox`.
#[no_mangle]
pub extern "C" fn staging_ptr() -> *mut u8 {
    WORKLET.with(|s| {
        s.borrow_mut()
            .as_mut()
            .map_or(std::ptr::null_mut(), |w| w.staging.as_mut_ptr())
    })
}

/// Fixed interleaved L/R input staging, filled from planar worklet channels.
#[no_mangle]
pub extern "C" fn input_ptr() -> *mut f32 {
    WORKLET.with(|s| {
        s.borrow_mut()
            .as_mut()
            .map_or(std::ptr::null_mut(), |w| w.input.as_mut_ptr())
    })
}

/// Queues the record in the first `len` staging bytes: 1 accepted (or
/// dropped as malformed, counted), 0 when the inbox is full (JS keeps it).
#[no_mangle]
pub extern "C" fn worklet_inbox(len: u32) -> u32 {
    with(|w| w.accept(len as usize))
}

/// Renders `frames` (at most 512); returns the planar output, `frames` left
/// samples then `frames` right samples.
#[no_mangle]
pub extern "C" fn process(frames: u32) -> *const f32 {
    WORKLET.with(|s| {
        s.borrow_mut().as_mut().map_or(std::ptr::null(), |w| {
            w.process((frames as usize).min(MAX_FRAMES))
        })
    })
}

/// The engine clock in seconds (the browser `host_now`).
#[no_mangle]
pub extern "C" fn worklet_now() -> f64 {
    with(|w| w.engine.now())
}

/// The report array.
#[no_mangle]
pub extern "C" fn report_ptr() -> *const f64 {
    WORKLET.with(|s| {
        s.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |w| w.report.as_ptr())
    })
}

/// The report length in `f64`s.
#[no_mangle]
pub extern "C" fn report_len() -> u32 {
    REPORT_LEN as u32
}

/// Probes cell `cell` in probe slot `k` (a negative cell clears it).
#[no_mangle]
pub extern "C" fn worklet_probe_cell(k: u32, cell: i32) {
    with(|w| {
        let c = u32::try_from(cell).ok().map(CellId::new);
        w.cells.set_probe(k as usize, c);
    });
}

/// Reports the store state of sample resource `id` (negative: none).
#[no_mangle]
pub extern "C" fn worklet_probe_resource(id: i32) {
    with(|w| w.probe_res = u32::try_from(id).ok());
}

/// Resets the watch counters: max bytes per quantum, staged max, dropouts,
/// deferred counts and the minimum RMS.
#[no_mangle]
pub extern "C" fn worklet_reset_watch() {
    with(|w| {
        for i in [
            R_BYTES_MAX,
            R_STAGED_MAX,
            R_DROPOUTS,
            R_SOUNDING,
            R_DEFERRED_QUANTA,
            R_DEFERRED_ACKS,
        ] {
            w.report[i] = 0.0;
        }
        w.report[R_RMS_MIN] = f64::INFINITY;
        w.sounding = false;
    });
}

impl Worklet {
    fn accept(&mut self, len: usize) -> u32 {
        let Some(rec) = self.staging.get(..len.min(INBOX_SLOT_BYTES)) else {
            return 1;
        };
        match rec.first().copied() {
            None => 1,
            Some(TAG_AUDIO_EVENT) => {
                match AudioEvent::decode(rec) {
                    Ok((ev, _)) => {
                        if self.ev_tx.push(ev).is_err() {
                            self.report[R_RING_FULL] += 1.0;
                        }
                    }
                    Err(_) => self.report[R_INBOX_REFUSED] += 1.0,
                }
                1
            }
            Some(tag) => {
                if self.inbox.len() >= INBOX_SLOTS {
                    return 0;
                }
                let install = matches!(tag, TAG_SAMPLE_SLICE | TAG_GRAPH_INSTALL);
                let rec_len = rec.len();
                let word = |at: usize| {
                    rec.get(at..at + 4)
                        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                };
                let (resource, offset) = (word(1), word(5));
                if !self.inbox.push(rec) {
                    return 1;
                }
                if install {
                    self.staged_now += rec_len.saturating_sub(INSTALL_HEAD);
                }
                if tag == TAG_SAMPLE_SLICE {
                    self.arrivals[self.arrival_at] = (resource, offset, self.quanta);
                    self.arrival_at = (self.arrival_at + 1) % ARRIVALS;
                }
                1
            }
        }
    }

    fn process(&mut self, frames: usize) -> *const f32 {
        let r = &mut self.report;
        r[R_STAGED_MAX] = r[R_STAGED_MAX].max(self.staged_now as f64);
        self.staged_now = 0;
        self.cells.now = self.engine.now();
        let mut io = EngineIo {
            events: &mut self.ev_rx,
            controls: &mut self.inbox,
            acks: &mut self.ack_tx,
            cells: &mut self.cells,
            garbage: None,
        };
        let channels = usize::from(self.engine.config().output_channels);
        let out = &mut self.out[..channels * frames];
        if channels == 4 {
            self.engine
                .process_four_with_input(&mut io, &self.input[..2 * frames], out, frames);
        } else {
            self.engine
                .process_with_input(&mut io, &self.input[..2 * frames], out, frames);
        }
        self.input[..2 * frames].fill(0.0);
        self.forward();
        self.observe(frames);
        self.quanta += 1;
        self.planar.as_ptr()
    }

    /// Faults and acks into the outbox.
    fn forward(&mut self) {
        while let Some(f) = self.engine.pop_fault() {
            let k = match f.code {
                FaultCode::ArenaExhausted => R_FAULT_ARENA,
                FaultCode::GraphTooLarge => R_FAULT_GRAPH,
                FaultCode::InstallQueueOverflow => R_FAULT_OVERFLOW,
                _ => R_FAULT_OTHER,
            };
            self.report[k] += 1.0;
            let mut rec = [0u8; 6];
            rec[0] = TAG_FAULT;
            rec[1] = f.code as u8;
            rec[2..6].copy_from_slice(&f.resource.to_le_bytes());
            push_record(&rec);
        }
        while let Some(m) = self.ack_rx.pop() {
            match m {
                HostMsg::Retired { .. } => self.report[R_RETIRED] += 1.0,
                HostMsg::CellRetired { .. } => self.report[R_CELL_RETIRED] += 1.0,
                HostMsg::Installed { .. } => self.report[R_INSTALLED] += 1.0,
                HostMsg::SliceOk { resource, offset } => {
                    self.report[R_SLICE_ACKS] += 1.0;
                    let late = self
                        .arrivals
                        .iter()
                        .find(|a| a.0 == resource && a.1 == offset)
                        .is_some_and(|a| a.2 < self.quanta);
                    if late {
                        self.report[R_DEFERRED_ACKS] += 1.0;
                    }
                }
                _ => {}
            }
            let mut rec = [0u8; HostMsg::MAX_LEN];
            let n = m.encode(&mut rec);
            push_record(&rec[..n]);
        }
        if self.quanta % 8 == 0 {
            let sigs = self.engine.host_sigs();
            let mut rec = [0u8; 1 + 4 * 9];
            rec[0] = TAG_SIGS;
            rec[1..5].copy_from_slice(&sigs.amp.to_le_bytes());
            for (i, x) in sigs.fft.iter().enumerate() {
                rec[5 + 4 * i..9 + 4 * i].copy_from_slice(&x.to_le_bytes());
            }
            push_record(&rec);
        }
    }

    /// Planar output, the level watch and the report.
    fn observe(&mut self, frames: usize) {
        let mut energy = 0.0f64;
        let channels = usize::from(self.engine.config().output_channels);
        for k in 0..frames {
            let (l, rr) = (self.out[channels * k], self.out[channels * k + 1]);
            for ch in 0..channels {
                self.planar[ch * frames + k] = self.out[channels * k + ch];
            }
            energy += f64::from(0.5 * (l + rr)).powi(2);
        }
        let rms = (energy / frames.max(1) as f64).sqrt();
        let r = &mut self.report;
        r[R_RMS] = rms;
        if rms > SILENT {
            self.sounding = true;
            r[R_SOUNDING] += 1.0;
            r[R_RMS_MIN] = r[R_RMS_MIN].min(rms);
        } else if self.sounding {
            r[R_DROPOUTS] += 1.0;
        }
        let c = self.engine.counters();
        r[R_NOW] = self.engine.now();
        r[R_QUANTA] = self.quanta as f64 + 1.0;
        r[R_LATE] = f64::from(c.late);
        r[R_DROPPED] = f64::from(c.dropped);
        r[R_STOLEN] = f64::from(c.stolen);
        r[R_BYTES_QUANTUM] = c.bytes_copied_quantum as f64;
        r[R_BYTES_MAX] = r[R_BYTES_MAX].max(c.bytes_copied_quantum as f64);
        r[R_BYTES_TOTAL] = c.bytes_copied_total as f64;
        let held = self.inbox.held();
        r[R_HELD] = held as f64;
        if held > 0 {
            r[R_DEFERRED_QUANTA] += 1.0;
        }
        r[R_ACTIVE] = self.engine.active_voices() as f64;
        r[R_MEM_NOW] = memory_bytes();
        r[R_INBOX_REFUSED] = r[R_INBOX_REFUSED].max(self.inbox.refused() as f64);
        r[R_OUTBOX_DROPPED] = outbox_dropped() as f64;
        r[R_RES_ID] = self.probe_res.map_or(-1.0, f64::from);
        r[R_RES_STATE] = self.probe_res.map_or(0.0, |id| {
            use crate::dsp::arena::ResState;
            match self.engine.store().state(id) {
                None => 0.0,
                Some(ResState::Installing) => 1.0,
                Some(ResState::Live) => 2.0,
                Some(ResState::Retiring) => 3.0,
            }
        });
        for k in 0..PROBES {
            let p = self.cells.probe(k);
            let base = R_PROBES + 8 * k;
            let Some(cell) = p.cell else {
                r[base] = -1.0;
                continue;
            };
            let (kind, epoch, value) = self.cells.describe(cell);
            r[base] = f64::from(cell.get());
            r[base + 1] = f64::from(kind);
            r[base + 2] = f64::from(epoch);
            r[base + 3] = f64::from(value);
            r[base + 4] = f64::from(p.reads);
            r[base + 5] = f64::from(p.uninit);
            r[base + 6] = f64::from(p.last);
            r[base + 7] = f64::from(self.cells.mirror.last_epoch(cell));
        }
        let mut n = 0;
        for v in self.engine.voices().voices.iter() {
            let Some(tag) = v.tag.filter(|_| v.active) else {
                continue;
            };
            if n == TAGGED {
                break;
            }
            let base = R_TAGGED + 4 * n;
            r[base] = f64::from(tag.slot.get());
            r[base + 1] = f64::from(tag.pitch);
            r[base + 2] = f64::from(tag.seq);
            r[base + 3] = if v.released() { 1.0 } else { 0.0 };
            n += 1;
        }
        r[R_TAGGED_N] = n as f64;
        let logged = self.cells.logged();
        r[R_LOG_N] = logged as f64;
        for i in 0..READ_LOG {
            let (t, cell, value, live) = self.cells.log(i);
            let base = R_LOG + 4 * i;
            r[base] = t;
            r[base + 1] = f64::from(cell);
            r[base + 2] = f64::from(value);
            r[base + 3] = if live { 1.0 } else { 0.0 };
        }
    }
}
