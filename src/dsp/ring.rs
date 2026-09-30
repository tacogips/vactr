//! Lock-free single-producer single-consumer rings (design 12.2, 12.8.5).
//!
//! `SpscRing<T>` is the one transport between the evaluator side and the
//! audio callback: the event ring (`AudioEvent`, 1024 by default), and the
//! control and ack channels (256 records each). Every slot is allocated in
//! `split`; `push` and `pop` only move values and touch two atomics, so the
//! callback never allocates, locks or blocks. A push onto a full ring hands
//! the value back and counts a drop; the host reports `ring-overflow`.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::host::wire::{AudioEvent, CtlMsg, HostMsg};

/// The default event ring capacity (12.8.9).
pub const EVENT_CAPACITY: usize = 1024;
/// The default control and ack channel capacity per sink (12.8.9).
pub const CHANNEL_CAPACITY: usize = 256;

struct Shared<T> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    /// The next slot to read; written only by the consumer.
    head: AtomicUsize,
    /// The next slot to write; written only by the producer.
    tail: AtomicUsize,
    /// Pushes refused because the ring was full.
    dropped: AtomicUsize,
}

// SAFETY: a slot is written only by the single producer before it publishes
// `tail` (release) and read only by the single consumer after it observes
// that `tail` (acquire), and vice versa for `head`; no slot is ever accessed
// by both sides at once. `T: Send` moves values across the thread boundary.
unsafe impl<T: Send> Sync for Shared<T> {}
// SAFETY: see above; the ring owns its values and is safe to move.
unsafe impl<T: Send> Send for Shared<T> {}

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        let mut head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        while head != tail {
            let i = head % self.slots.len();
            // SAFETY: slots in `[head, tail)` hold initialized values that
            // were never read; each is dropped exactly once here.
            unsafe { (*self.slots[i].get()).assume_init_drop() };
            head = head.wrapping_add(1);
        }
    }
}

/// A fixed-capacity SPSC ring; `split` yields its two ends.
pub struct SpscRing<T>(std::marker::PhantomData<T>);

impl<T: Send> SpscRing<T> {
    /// Allocates a ring of `capacity` slots (at least 1) and returns its
    /// producer and consumer ends.
    #[must_use]
    pub fn split(capacity: usize) -> (Producer<T>, Consumer<T>) {
        let capacity = capacity.max(1);
        let shared = Arc::new(Shared {
            slots: (0..capacity)
                .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
                .collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        });
        (
            Producer {
                shared: Arc::clone(&shared),
            },
            Consumer { shared },
        )
    }
}

/// The writing end of an `SpscRing`.
pub struct Producer<T> {
    shared: Arc<Shared<T>>,
}

/// The reading end of an `SpscRing`.
pub struct Consumer<T> {
    shared: Arc<Shared<T>>,
}

impl<T: Send> Producer<T> {
    /// Appends a value; on a full ring the value comes back and a drop is
    /// counted.
    ///
    /// # Errors
    /// The value itself when the ring is full.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        let s = &*self.shared;
        let tail = s.tail.load(Ordering::Relaxed);
        let head = s.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) >= s.slots.len() {
            s.dropped.fetch_add(1, Ordering::Relaxed);
            return Err(value);
        }
        let i = tail % s.slots.len();
        // SAFETY: the slot at `tail` is outside `[head, tail)`, so the
        // consumer does not read it until `tail` is published below.
        unsafe { (*s.slots[i].get()).write(value) };
        s.tail.store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Pushes refused so far (the host's `ring-overflow` source).
    #[must_use]
    pub fn dropped(&self) -> usize {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    /// The number of queued values.
    #[must_use]
    pub fn len(&self) -> usize {
        len(&self.shared)
    }

    /// True when nothing is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The fixed capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.shared.slots.len()
    }
}

impl<T: Send> Consumer<T> {
    /// Removes the oldest value.
    pub fn pop(&mut self) -> Option<T> {
        let s = &*self.shared;
        let head = s.head.load(Ordering::Relaxed);
        let tail = s.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let i = head % s.slots.len();
        // SAFETY: the slot at `head` lies in `[head, tail)`, initialized by
        // the producer before it published `tail`; it is read exactly once.
        let value = unsafe { (*s.slots[i].get()).assume_init_read() };
        s.head.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// The oldest value, without removing it.
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        let s = &*self.shared;
        let head = s.head.load(Ordering::Relaxed);
        let tail = s.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let i = head % s.slots.len();
        // SAFETY: as in `pop`; the value stays in place and only the
        // consumer (the caller, borrowed) can remove it.
        Some(unsafe { (*s.slots[i].get()).assume_init_ref() })
    }

    /// The number of queued values.
    #[must_use]
    pub fn len(&self) -> usize {
        len(&self.shared)
    }

    /// True when nothing is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn len<T>(s: &Shared<T>) -> usize {
    let tail = s.tail.load(Ordering::Acquire);
    let head = s.head.load(Ordering::Acquire);
    tail.wrapping_sub(head)
}

/// The time-ordered event ring (design 11.4).
pub type EventProducer = Producer<AudioEvent>;
/// The audio side of the event ring.
pub type EventConsumer = Consumer<AudioEvent>;

/// The event ring constructor.
pub struct EventRing;

impl EventRing {
    /// An event ring of `capacity` events (`EVENT_CAPACITY` by default).
    #[must_use]
    pub fn split(capacity: usize) -> (EventProducer, EventConsumer) {
        SpscRing::split(capacity)
    }
}

/// A priority control channel of POD records (the native tier).
pub type CtlProducer = Producer<CtlMsg>;
/// The audio side of a control channel.
pub type CtlConsumer = Consumer<CtlMsg>;
/// The ack channel back to the evaluator.
pub type AckProducer = Producer<HostMsg>;
/// The evaluator side of the ack channel.
pub type AckConsumer = Consumer<HostMsg>;

// ---- control records -------------------------------------------------------

use crate::dsp::arena::{StoreKind, SLICE_BYTES};
use crate::dsp::bus::BusTemplate;
use crate::dsp::bus::DEFAULT_BUS_SLOTS;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::{AtomicCells, CellId, CellRead, Mirror};
use crate::dsp::effects::FxStats;
use crate::dsp::ugen::Template;
use crate::host::caps::SampleData;
use crate::host::wire::{decode_batch, BatchView};

/// What the engine may take from a control source in one `process()`:
/// install records only up to the remaining copy credit (16.1), and at most
/// one `CellBatch` (12.8.9). Records not taken stay queued in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Budget {
    pub install_bytes: usize,
    pub batch: bool,
}

/// A native install: fully built structures handed over by pointer; the
/// engine returns them (or the ones they replace) as `Garbage`.
pub enum NativeInstall {
    Inst {
        resource: u32,
        gen: u32,
        template: Box<Template>,
    },
    Bus {
        resource: u32,
        gen: u32,
        master: bool,
        template: Box<BusTemplate>,
    },
    Sample {
        resource: u32,
        gen: u32,
        data: Arc<SampleData>,
    },
}

/// A structure retired on the audio side, to be dropped on the evaluator
/// thread (12.2 "garbage queue").
pub enum Garbage {
    Template(Box<Template>),
    Bus(Box<BusTemplate>),
    Sample(Arc<SampleData>),
}

/// One record on the priority control channel. Inline, never boxed: a
/// box would allocate on the audio path.
#[allow(clippy::large_enum_variant)]
pub enum Record<'a> {
    Msg(CtlMsg),
    /// A `CellBatch` with its entries (browser).
    Batch(BatchView<'a>),
    /// The start of an arena sample install (browser).
    SampleBegin {
        resource: u32,
        gen: u32,
        frames: u32,
        channels: u8,
        rate: u32,
    },
    /// A slice of little-endian `f32` samples at float `offset` (browser).
    Slice {
        resource: u32,
        offset: u32,
        data: &'a [u8],
    },
    /// Encoded graph bytes (`arena::encode_inst`/`encode_bus`, browser).
    Graph {
        id: u32,
        gen: u32,
        bytes: &'a [u8],
    },
    Native(NativeInstall),
}

impl Record<'_> {
    /// The copy work this record costs against the install credit.
    #[must_use]
    pub fn install_bytes(&self) -> usize {
        match self {
            Record::Slice { data, .. } => data.len(),
            Record::Graph { bytes, .. } => bytes.len(),
            _ => 0,
        }
    }
}

/// The engine's view of its priority control channel.
pub trait ControlSource {
    /// The oldest record the budget allows (see `Budget`).
    fn next(&mut self, budget: Budget) -> Option<Record<'_>>;
    /// Install records waiting for credit.
    fn held(&self) -> usize {
        0
    }
    /// Drops the newest waiting install record (deferred-queue overflow);
    /// returns its resource id.
    fn drop_held(&mut self) -> Option<u32> {
        None
    }
}

/// No control records.
#[derive(Clone, Copy, Default, Debug)]
pub struct NoControls;

impl ControlSource for NoControls {
    fn next(&mut self, _budget: Budget) -> Option<Record<'_>> {
        None
    }
}

impl ControlSource for Consumer<CtlMsg> {
    fn next(&mut self, _budget: Budget) -> Option<Record<'_>> {
        self.pop().map(Record::Msg)
    }
}

/// A native channel record: a POD message or an install handover (inline,
/// like `CtlMsg`, so the ring slot never allocates).
#[allow(clippy::large_enum_variant)]
pub enum NativeRecord {
    Msg(CtlMsg),
    Install(NativeInstall),
}

impl ControlSource for Consumer<NativeRecord> {
    fn next(&mut self, _budget: Budget) -> Option<Record<'_>> {
        self.pop().map(|r| match r {
            NativeRecord::Msg(m) => Record::Msg(m),
            NativeRecord::Install(i) => Record::Native(i),
        })
    }
}

/// Slots of the browser inbox.
pub const INBOX_SLOTS: usize = 16;
/// The largest record one inbox slot holds (a full slice plus header).
pub const INBOX_SLOT_BYTES: usize = 64 + SLICE_BYTES;

const TAG_CELL_BATCH: u8 = 0x12;
const TAG_GRAPH_INSTALL: u8 = 0x16;
const TAG_SAMPLE_SLICE: u8 = 0x18;

/// The worklet's inbox: encoded records copied into preallocated slots as
/// they arrive (a bounded O(record) copy), handed to the engine in order.
pub struct ByteInbox {
    bytes: Box<[u8]>,
    lens: [usize; INBOX_SLOTS],
    seqs: [u64; INBOX_SLOTS],
    used: [bool; INBOX_SLOTS],
    next_seq: u64,
    current: Option<usize>,
    refused: std::cell::Cell<usize>,
}

impl Default for ByteInbox {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteInbox {
    /// Allocates every slot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes: vec![0; INBOX_SLOTS * INBOX_SLOT_BYTES].into_boxed_slice(),
            lens: [0; INBOX_SLOTS],
            seqs: [0; INBOX_SLOTS],
            used: [false; INBOX_SLOTS],
            next_seq: 0,
            current: None,
            refused: std::cell::Cell::new(0),
        }
    }

    /// Queues one encoded record; false (and counted) when full.
    pub fn push(&mut self, rec: &[u8]) -> bool {
        let free = self.used.iter().position(|u| !u);
        match free {
            Some(i) if !rec.is_empty() && rec.len() <= INBOX_SLOT_BYTES => {
                self.bytes[i * INBOX_SLOT_BYTES..i * INBOX_SLOT_BYTES + rec.len()]
                    .copy_from_slice(rec);
                self.lens[i] = rec.len();
                self.seqs[i] = self.next_seq;
                self.next_seq += 1;
                self.used[i] = true;
                true
            }
            _ => {
                self.refused.set(self.refused.get() + 1);
                false
            }
        }
    }

    /// Records refused (inbox full, too large or undecodable).
    #[must_use]
    pub fn refused(&self) -> usize {
        self.refused.get()
    }

    /// Queued records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.used.iter().filter(|u| **u).count()
    }

    /// True when nothing is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn slot(&self, i: usize) -> &[u8] {
        &self.bytes[i * INBOX_SLOT_BYTES..i * INBOX_SLOT_BYTES + self.lens[i]]
    }

    /// `(install bytes, is batch, resource)` of a queued record.
    fn class(&self, i: usize) -> (usize, bool, u32) {
        let b = self.slot(i);
        let word = |at: usize| {
            b.get(at..at + 4)
                .map_or(0, |w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        };
        match b.first().copied() {
            Some(TAG_SAMPLE_SLICE) => (b.len().saturating_sub(13), false, word(1)),
            Some(TAG_GRAPH_INSTALL) => (b.len().saturating_sub(13), false, word(1)),
            Some(TAG_CELL_BATCH) => (0, true, 0),
            _ => (0, false, 0),
        }
    }

    fn release_current(&mut self) {
        if let Some(i) = self.current.take() {
            self.used[i] = false;
        }
    }
}

impl ControlSource for ByteInbox {
    fn next(&mut self, budget: Budget) -> Option<Record<'_>> {
        self.release_current();
        let mut pick: Option<usize> = None;
        for i in 0..INBOX_SLOTS {
            if !self.used[i] || pick.is_some_and(|p| self.seqs[p] < self.seqs[i]) {
                continue;
            }
            let (install, batch, _) = self.class(i);
            let allowed = if batch {
                budget.batch
            } else {
                install == 0 || install <= budget.install_bytes
            };
            if allowed {
                pick = Some(i);
            }
        }
        let i = pick?;
        self.current = Some(i);
        let b = self.slot(i);
        let parsed = match b[0] {
            TAG_CELL_BATCH => decode_batch(b).ok().map(|(v, _)| Record::Batch(v)),
            TAG_SAMPLE_SLICE => match CtlMsg::decode(b) {
                Ok((
                    CtlMsg::SampleSlice {
                        resource,
                        offset,
                        len,
                    },
                    head,
                )) => {
                    let end = head + (len as usize) * 4;
                    b.get(head..end).map(|data| Record::Slice {
                        resource,
                        offset,
                        data,
                    })
                }
                _ => None,
            },
            TAG_GRAPH_INSTALL => match CtlMsg::decode(b) {
                Ok((CtlMsg::GraphInstall { id, gen }, head)) => b
                    .get(head..head + 4)
                    .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]) as usize)
                    .and_then(|n| b.get(head + 4..head + 4 + n))
                    .map(|bytes| Record::Graph { id, gen, bytes }),
                _ => None,
            },
            TAG_SAMPLE_BEGIN => {
                decode_sample_begin(b).map(|(resource, gen, frames, channels, rate)| {
                    Record::SampleBegin {
                        resource,
                        gen,
                        frames,
                        channels,
                        rate,
                    }
                })
            }
            _ => CtlMsg::decode(b).ok().map(|(m, _)| Record::Msg(m)),
        };
        // An undecodable record is dropped (the sender re-sends on no ack).
        if parsed.is_none() {
            self.refused.set(self.refused.get() + 1);
        }
        parsed
    }

    fn held(&self) -> usize {
        (0..INBOX_SLOTS)
            .filter(|&i| self.used[i] && Some(i) != self.current && self.class(i).0 > 0)
            .count()
    }

    fn drop_held(&mut self) -> Option<u32> {
        let i = (0..INBOX_SLOTS)
            .filter(|&i| self.used[i] && Some(i) != self.current && self.class(i).0 > 0)
            .max_by_key(|&i| self.seqs[i])?;
        let (_, _, resource) = self.class(i);
        self.used[i] = false;
        Some(resource)
    }
}

// ---- the engine's I/O -------------------------------------------------------

/// A cell table the engine can apply cell records to (11.3, 12.8.5).
pub trait CellStore: CellRead {
    /// `CellInit`; the ack to send, if any.
    fn init(&mut self, cell: CellId, epoch: u32, value: f32) -> Option<HostMsg>;
    /// `CellBatch` with its entries; the ack to send, if any.
    fn batch(&mut self, seq: u32, view: &BatchView<'_>) -> Option<HostMsg>;
    /// `CellRetire`; true when the cell started retiring.
    fn retire(&mut self, cell: CellId, epoch: u32) -> bool;
    /// No use of a retiring cell is left; true when it was vacated.
    fn retired(&mut self, cell: CellId) -> bool;
}

impl CellStore for Mirror {
    fn init(&mut self, cell: CellId, epoch: u32, value: f32) -> Option<HostMsg> {
        self.apply_init(cell, epoch, value)
    }
    fn batch(&mut self, seq: u32, view: &BatchView<'_>) -> Option<HostMsg> {
        self.apply_batch(seq, view.iter())
    }
    fn retire(&mut self, cell: CellId, epoch: u32) -> bool {
        Mirror::retire(self, cell, epoch)
    }
    fn retired(&mut self, cell: CellId) -> bool {
        Mirror::retired(self, cell).is_some()
    }
}

/// Native cells are shared memory: cell records are never sent natively,
/// and are applied directly if they are.
impl CellStore for AtomicCells {
    fn init(&mut self, cell: CellId, epoch: u32, value: f32) -> Option<HostMsg> {
        self.set(cell, value)
            .then_some(HostMsg::CellInitAck { cell, epoch })
    }
    fn batch(&mut self, seq: u32, view: &BatchView<'_>) -> Option<HostMsg> {
        for (cell, _, value) in view.iter() {
            self.set(cell, value);
        }
        Some(HostMsg::CellBatchAck { seq })
    }
    fn retire(&mut self, _cell: CellId, _epoch: u32) -> bool {
        true
    }
    fn retired(&mut self, _cell: CellId) -> bool {
        true
    }
}

/// Everything `process` reads and writes outside the engine.
pub struct EngineIo<'a, C: CellStore, S: ControlSource> {
    pub events: &'a mut EventConsumer,
    pub controls: &'a mut S,
    pub acks: &'a mut AckProducer,
    pub cells: &'a mut C,
    /// Natively, retired structures go back here for dropping.
    pub garbage: Option<&'a mut Producer<Garbage>>,
}

// ---- browser install records (the stream `ByteInbox` parses) ---------------

/// The arena record that starts a sample install (browser stream).
pub const TAG_SAMPLE_BEGIN: u8 = 0x1A;

/// Encodes a `SampleBegin` record.
#[must_use]
pub fn encode_sample_begin(
    resource: u32,
    gen: u32,
    frames: u32,
    channels: u8,
    rate: u32,
) -> [u8; 18] {
    let mut b = [0u8; 18];
    b[0] = TAG_SAMPLE_BEGIN;
    b[1..5].copy_from_slice(&resource.to_le_bytes());
    b[5..9].copy_from_slice(&gen.to_le_bytes());
    b[9..13].copy_from_slice(&frames.to_le_bytes());
    b[13] = channels;
    b[14..18].copy_from_slice(&rate.to_le_bytes());
    b
}

/// Decodes a `SampleBegin` record: `(resource, gen, frames, channels, rate)`.
#[must_use]
pub fn decode_sample_begin(b: &[u8]) -> Option<(u32, u32, u32, u8, u32)> {
    if b.len() < 18 || b[0] != TAG_SAMPLE_BEGIN {
        return None;
    }
    let w = |at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    Some((w(1), w(5), w(9), b[13], w(14)))
}

/// Encodes a `SampleSlice` record with its samples.
pub fn encode_slice(resource: u32, offset: u32, samples: &[f32], out: &mut Vec<u8>) {
    out.clear();
    let len = u32::try_from(samples.len()).unwrap_or(u32::MAX);
    let mut head = [0u8; 16];
    let n = CtlMsg::SampleSlice {
        resource,
        offset,
        len,
    }
    .encode(&mut head);
    out.extend_from_slice(&head[..n]);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
}

/// Encodes a `GraphInstall` record followed by the graph bytes.
pub fn encode_graph_record(id: u32, gen: u32, graph: &[u8], out: &mut Vec<u8>) {
    out.clear();
    let mut head = [0u8; 16];
    let n = CtlMsg::GraphInstall { id, gen }.encode(&mut head);
    out.extend_from_slice(&head[..n]);
    out.extend_from_slice(&u32::try_from(graph.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(graph);
}

// ---- engine configuration and counters -------------------------------------

/// Engine counters (12.8.9, 16.1).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Counters {
    pub late: u32,
    pub dropped: u32,
    pub stolen: u32,
    pub fx: FxStats,
    /// Install bytes copied in the last `process()`, the maximum over all
    /// calls, and the total.
    pub bytes_copied_quantum: usize,
    pub bytes_copied_max: usize,
    pub bytes_copied_total: u64,
    /// Arena bytes allocated at construction (never changes).
    pub memory_capacity: usize,
}

/// Construction parameters; `Engine::new` uses the defaults.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub caps: CapabilitySet,
    pub sample_rate: f32,
    pub max_block: usize,
    /// Two-channel master or opt-in four-channel direct-stem output.
    pub output_channels: u8,
    pub store: StoreKind,
    pub template_slots: usize,
    pub bus_slots: usize,
    /// Bus delay memory beyond the capture buffer, seconds.
    pub bus_seconds: f32,
    /// Per-voice state budget: this value times sample rate gives float samples.
    /// The default four seconds costs 768 KB per voice at 48 kHz; it reserves
    /// 196.6 MB for 256 native voices or 49.2 MB for 64 browser voices.
    /// Stereo rings share this budget; it is not a maximum delay duration.
    pub voice_seconds: f32,
    pub orbits: usize,
    pub orbit_delay_seconds: f32,
    pub analysis_cells: usize,
    pub event_capacity: usize,
}

/// A host configuration that cannot be represented by the preallocated DSP core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    SampleRate,
    BlockSize,
    StateBudget,
    OutputChannels,
}

impl EngineConfig {
    /// Supported host rates and callback quantum sizes. Longer callbacks are
    /// processed in `max_block` chunks, without growing callback buffers.
    pub const MIN_SAMPLE_RATE: f32 = 8_000.0;
    pub const MAX_SAMPLE_RATE: f32 = 192_000.0;
    pub const MAX_BLOCK: usize = 8_192;
    /// Per-voice and per-bus delay-state ceiling, in float samples.
    pub const MAX_STATE_SAMPLES: f32 = 2_000_000.0;

    /// The defaults of 12.8.9 for a tier, with full-state creative effects.
    /// At 48 kHz, voice state costs `max_voices * 768_000` bytes and each
    /// bus reserves 1.92 MB plus the granular capture region. Costs scale
    /// linearly with sample rate; custom smaller budgets reject oversized
    /// effect graphs at installation rather than reducing their algorithms.
    #[must_use]
    pub fn new(caps: &CapabilitySet, sample_rate: f32, max_block: usize, store: StoreKind) -> Self {
        Self {
            caps: *caps,
            sample_rate,
            max_block,
            output_channels: 2,
            store,
            // The core prelude currently installs 67 definitions. Leave
            // bounded room for local song voices and opt-in instruments.
            template_slots: 96,
            bus_slots: DEFAULT_BUS_SLOTS,
            // Full stereo FDN/shimmer state fits one voice; buses have room
            // for useful combinations. These buffers are reserved at startup.
            bus_seconds: 10.0,
            voice_seconds: 4.0,
            orbits: 4,
            orbit_delay_seconds: 4.0,
            analysis_cells: 4096,
            event_capacity: EVENT_CAPACITY,
        }
    }

    /// Validates the rate, quantum and per-voice state before any buffers are
    /// allocated. The per-voice budget is measured in `f32` samples.
    ///
    /// # Errors
    /// A rate, block size or state budget outside the supported range.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.sample_rate.is_finite()
            || !(Self::MIN_SAMPLE_RATE..=Self::MAX_SAMPLE_RATE).contains(&self.sample_rate)
        {
            return Err(ConfigError::SampleRate);
        }
        if !(1..=Self::MAX_BLOCK).contains(&self.max_block) {
            return Err(ConfigError::BlockSize);
        }
        if !matches!(self.output_channels, 2 | 4) {
            return Err(ConfigError::OutputChannels);
        }
        let valid_seconds = |seconds: f32| {
            seconds.is_finite()
                && seconds >= 0.0
                && seconds * self.sample_rate <= Self::MAX_STATE_SAMPLES
        };
        if !valid_seconds(self.voice_seconds)
            || !valid_seconds(self.bus_seconds)
            || !valid_seconds(self.orbit_delay_seconds)
        {
            return Err(ConfigError::StateBudget);
        }
        Ok(())
    }
}

/// Hands a retired structure back for dropping on the evaluator thread. A
/// full (or absent) garbage ring leaks it rather than freeing on the audio
/// thread.
pub fn push_garbage(q: Option<&mut Producer<Garbage>>, g: Garbage) {
    match q {
        Some(q) => {
            if let Err(g) = q.push(g) {
                std::mem::forget(g);
            }
        }
        None => std::mem::forget(g),
    }
}
