//! The main half's hosts (design 12.8.5, 12.8.10, 16, 16.1).
//!
//! `WasmAudioHost` turns every `AudioHost` call into wire records in the
//! outbox, which JS posts to the worklet one `ArrayBuffer` each. Samples go
//! through the 16.1 sender-paced window: admission against the arena model
//! first (`arena-exhausted` fails immediately), then ONE slice in flight,
//! the next posted only on that slice's `SliceOk`. Graphs are encoded with
//! the `dsp::arena` graph codec. The engine publishes cumulative counters;
//! `drain` hands the runtime deltas, so its late-event widening (12.8.4)
//! counts each late event once. `WasmCellPort` carries the browser cell
//! records; their acknowledgments reach the runtime through `drain`.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::arena::{encode_bus, encode_inst, SLICE_BYTES};
use crate::dsp::cells::CellId;
use crate::dsp::ring::{encode_graph_record, encode_sample_begin, encode_slice};
use crate::dsp::ring::{encode_song_graph_record, encode_song_sample_begin, encode_song_slice};
use crate::host::caps::{AudioHost, GraphHandle, HostSigs, SampleData, SampleLoader, SampleSrc};
use crate::host::wasm::abi::{push_record, TAG_CONSOLE};
use crate::host::wire::{encode_batch, AudioEvent, Ctl, CtlMsg, HostMsg, SlotControl};
use crate::reader::span::{FileId, Span};
use crate::sched::cells::CellPort;
use crate::sched::slots::CtlId;
use crate::song::routing::{SongHostAck, SongLeaseKey, SongResourceKind};
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::intern::name_of_kw;
use crate::vm::fail::{FailCode, Failure};

#[path = "messages/song.rs"]
mod song;

/// Floats per slice (one 64 KB slice, 16.1).
pub const SLICE_FLOATS: usize = SLICE_BYTES / 4;
/// Graph resource ids start here, apart from the sample ids the runtime's
/// sample table hands out from 1.
pub const GRAPH_BASE: u32 = 0x4000_0000;
/// Sent events remembered for the harness.
const SENT_LOG: usize = 64;

/// Writes one console line for the page.
pub fn console(text: &str) {
    let mut rec = Vec::with_capacity(1 + text.len());
    rec.push(TAG_CONSOLE);
    rec.extend_from_slice(text.as_bytes());
    push_record(&rec);
}

/// A queued sample install.
struct Pending {
    id: u32,
    gen: u32,
    data: Arc<SampleData>,
    /// The next float offset to send.
    next: usize,
    begun: bool,
}

struct SongPending {
    lease: SongLeaseKey,
    data: Arc<SampleData>,
    next: usize,
    begun: bool,
}

/// What the harness can observe of one sent event.
#[derive(Clone, Copy, Debug)]
pub struct SentEvent {
    pub time: f64,
    pub slot: u32,
    /// The control the harness probes: `Const` value, or the cell id.
    pub probe: Option<Ctl>,
}

/// The state behind `WasmAudioHost`, shared with the ABI exports.
pub struct HostState {
    pub now: f64,
    pub arena_bytes: usize,
    arena_used: usize,
    sizes: BTreeMap<u32, usize>,
    /// Records from the worklet, drained by the runtime at each tick.
    pub acks: VecDeque<HostMsg>,
    queue: VecDeque<Pending>,
    /// `(resource, offset, floats)` of the slice in flight.
    in_flight: Option<(u32, u32, usize)>,
    song_queue: VecDeque<SongPending>,
    song_flight: Option<(SongLeaseKey, u32, usize)>,
    song_bytes: usize,
    song_reservations: Vec<(SongLeaseKey, usize)>,
    song_clock_last: Option<crate::song::routing::SongClockRequest>,
    song_clock_pending: Option<crate::song::routing::SongClockRequest>,
    song_clock_report: Option<crate::song::routing::SongHostClock>,
    song_clock_error: Option<&'static str>,
    next_gen: u32,
    next_graph: u32,
    last: [u32; 4],
    pub sigs: HostSigs,
    pub probe_ctl: Option<CtlId>,
    pub sent: VecDeque<SentEvent>,
    pub sent_total: u64,
    pub slices_sent: u64,
    pub installed: u64,
    pub retired: Vec<u32>,
    pub late_total: u64,
    buf: Vec<u8>,
}

impl HostState {
    /// A host over a worklet arena of `arena_bytes`.
    #[must_use]
    pub fn new(arena_bytes: usize) -> Self {
        Self {
            now: 0.0,
            arena_bytes,
            arena_used: 0,
            sizes: BTreeMap::new(),
            acks: VecDeque::new(),
            queue: VecDeque::new(),
            in_flight: None,
            song_queue: VecDeque::new(),
            song_flight: None,
            song_bytes: 0,
            song_reservations: Vec::new(),
            song_clock_last: None,
            song_clock_pending: None,
            song_clock_report: None,
            song_clock_error: None,
            next_gen: 0,
            next_graph: GRAPH_BASE,
            last: [0; 4],
            sigs: HostSigs::default(),
            probe_ctl: None,
            sent: VecDeque::new(),
            sent_total: 0,
            slices_sent: 0,
            installed: 0,
            retired: Vec::new(),
            late_total: 0,
            buf: Vec::new(),
        }
    }

    /// Account for sample reservations until retirement acknowledgment.
    /// Additional graph/generation capacity must be supplied by host preparation.
    pub fn remaining_song_sample_limits(
        &self,
        mut limits: crate::song::assets::SongAssetLimits,
    ) -> Result<crate::song::assets::SongAssetLimits, Failure> {
        limits.max_resources = limits
            .max_resources
            .min(u32::try_from(crate::dsp::arena::MAX_RESOURCES).unwrap_or(u32::MAX));
        limits.max_pcm_bytes = limits.max_pcm_bytes.min(
            u64::try_from(self.arena_bytes)
                .map_err(|_| Failure::new(FailCode::Overflow, "arena byte count overflow"))?,
        );
        let resources = self
            .sizes
            .len()
            .checked_add(self.song_reservations.len())
            .ok_or_else(|| Failure::new(FailCode::Overflow, "reserved sample count overflow"))?;
        let bytes = self
            .arena_used
            .checked_add(self.song_bytes)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "reserved sample bytes overflow"))?;
        limits.after_reservations(
            u32::try_from(resources)
                .map_err(|_| Failure::new(FailCode::Overflow, "sample count overflow"))?,
            0,
            u64::try_from(bytes)
                .map_err(|_| Failure::new(FailCode::Overflow, "used arena bytes overflow"))?,
            0,
        )
    }

    /// Installs queued or in flight.
    #[must_use]
    pub fn installs_queued(&self) -> usize {
        self.queue.len()
    }

    /// True while a slice waits for its `SliceOk`.
    #[must_use]
    pub fn slice_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    fn gen(&mut self) -> u32 {
        self.next_gen += 1;
        self.next_gen
    }

    fn post(&mut self, msg: &CtlMsg) {
        let mut rec = [0u8; CtlMsg::MAX_LEN];
        let n = msg.encode(&mut rec);
        push_record(&rec[..n]);
    }

    /// Queues a sample install after the 16.1 admission check.
    fn install(&mut self, id: u32, data: Arc<SampleData>) {
        let Some(bytes) = data.frames.len().checked_mul(4) else {
            return;
        };
        let admitted = self
            .arena_used
            .checked_add(self.song_bytes)
            .and_then(|used| used.checked_add(bytes))
            .is_some_and(|total| total <= self.arena_bytes);
        if !admitted {
            let d = Diagnostic::error(
                DiagCode::ArenaExhausted,
                Span::new(FileId::new(0), 0, 0),
                format!(
                    "sample {id} of {bytes} bytes does not fit the free arena ({} bytes free)",
                    self.arena_bytes
                        .saturating_sub(self.arena_used)
                        .saturating_sub(self.song_bytes)
                ),
            );
            console(&format!("diag {d}"));
            return;
        }
        self.arena_used += bytes;
        self.sizes.insert(id, bytes);
        let gen = self.gen();
        self.queue.push_back(Pending {
            id,
            gen,
            data,
            next: 0,
            begun: false,
        });
        self.pump();
    }

    /// Sends the next slice when none is in flight (the one-slice window).
    fn pump(&mut self) {
        if self.in_flight.is_some() {
            return;
        }
        let Some(p) = self.queue.front_mut() else {
            return;
        };
        let channels = p.data.channels.max(1);
        let total = p.data.frames.len();
        if !p.begun {
            p.begun = true;
            let frames = u32::try_from(total / usize::from(channels)).unwrap_or(u32::MAX);
            let rec = encode_sample_begin(p.id, p.gen, frames, channels, p.data.rate);
            push_record(&rec);
        }
        let n = SLICE_FLOATS.min(total - p.next.min(total));
        let (id, off) = (p.id, u32::try_from(p.next).unwrap_or(u32::MAX));
        encode_slice(id, off, &p.data.frames[p.next..p.next + n], &mut self.buf);
        push_record(&self.buf);
        self.slices_sent += 1;
        self.in_flight = Some((id, off, n));
    }

    /// `SliceOk`: the window moves on.
    fn slice_ok(&mut self, resource: u32, offset: u32) {
        if self.in_flight.map(|f| (f.0, f.1)) != Some((resource, offset)) {
            return;
        }
        let (_, _, n) = self.in_flight.take().unwrap_or((0, 0, 0));
        if let Some(p) = self.queue.front_mut() {
            p.next += n;
            if p.next >= p.data.frames.len() {
                self.queue.pop_front();
            }
        }
        self.pump();
    }

    /// A retire: a queued install that never began is dropped (and reported
    /// retired at once); anything else goes to the worklet.
    fn retire(&mut self, id: u32) {
        let unsent = self.queue.iter().position(|p| p.id == id && !p.begun);
        if let Some(i) = unsent {
            self.queue.remove(i);
            self.release(id);
            self.acks.push_back(HostMsg::Retired { resource: id });
            return;
        }
        if let Some(i) = self.queue.iter().position(|p| p.id == id) {
            if self.in_flight.is_some_and(|f| f.0 == id) {
                self.in_flight = None;
            }
            self.queue.remove(i);
            self.pump();
        }
        self.post(&CtlMsg::SampleRetire { resource: id });
    }

    /// An install fault on the worklet: the install is abandoned so the
    /// window moves on to the next one.
    pub fn abort(&mut self, id: u32) {
        if self.in_flight.is_some_and(|f| f.0 == id) {
            self.in_flight = None;
        }
        if let Some(i) = self.queue.iter().position(|p| p.id == id) {
            self.queue.remove(i);
            self.release(id);
        }
        self.pump();
    }

    fn release(&mut self, id: u32) {
        if let Some(b) = self.sizes.remove(&id) {
            self.arena_used = self.arena_used.saturating_sub(b);
        }
    }

    /// One record from the worklet (turned into runtime messages).
    pub fn on_msg(&mut self, m: HostMsg) {
        let m = match m {
            HostMsg::Song(ack) => {
                self.song_ack(ack);
                m
            }
            HostMsg::Counters {
                late,
                dropped,
                stolen,
                skipped,
            } => {
                let cur = [late, dropped, stolen, skipped];
                let d: Vec<u32> = cur
                    .iter()
                    .zip(self.last.iter())
                    .map(|(c, l)| c.saturating_sub(*l))
                    .collect();
                self.last = cur;
                self.late_total += u64::from(d[0]);
                HostMsg::Counters {
                    late: d[0],
                    dropped: d[1],
                    stolen: d[2],
                    skipped: d[3],
                }
            }
            HostMsg::SliceOk { resource, offset } => {
                self.slice_ok(resource, offset);
                m
            }
            HostMsg::Installed { .. } => {
                self.installed += 1;
                m
            }
            HostMsg::Retired { resource } => {
                self.release(resource);
                self.retired.push(resource);
                m
            }
            other => other,
        };
        self.acks.push_back(m);
    }
}

/// The main half's `AudioHost`.
pub struct WasmAudioHost(pub Rc<RefCell<HostState>>);

impl AudioHost for WasmAudioHost {
    fn song_clock(&self) -> Result<crate::song::routing::SongHostClock, Failure> {
        self.song_clock_observation()
    }

    #[allow(clippy::result_large_err)] // Exact inline command refusal preserves caller ownership.
    fn try_song_command(
        &mut self,
        command: crate::song::routing::SongCommand,
    ) -> Result<(), crate::host::caps::SongCommandRefusal> {
        self.try_song_command_checked(command)
    }

    fn try_song_graph(
        &mut self,
        lease: SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<(), crate::host::caps::SongSubmitError> {
        self.try_song_graph_checked(lease, graph)
    }

    fn submit_song_graph(
        &mut self,
        lease: SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<(), Failure> {
        self.song_graph(lease, graph)
    }
    fn try_song_sample(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), crate::host::caps::SongSampleRefusal> {
        self.song_sample_checked(lease, data)
    }
    fn song_sample_sender_capacity(
        &self,
    ) -> Result<crate::host::caps::SongSampleSenderCapacity, Failure> {
        self.sample_sender_capacity()
    }
    fn submit_song_sample(
        &mut self,
        lease: SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.song_sample(lease, data)
    }

    fn send(&mut self, ev: AudioEvent) {
        let mut s = self.0.borrow_mut();
        let probe = s
            .probe_ctl
            .and_then(|id| ev.controls().iter().find(|(c, _)| *c == id).map(|e| e.1));
        if s.sent.len() == SENT_LOG {
            s.sent.pop_front();
        }
        s.sent.push_back(SentEvent {
            time: ev.time,
            slot: ev.slot.get(),
            probe,
        });
        s.sent_total += 1;
        let mut rec = [0u8; AudioEvent::ENCODED_LEN];
        let n = ev.encode(&mut rec);
        push_record(&rec[..n]);
    }

    fn control(&mut self, c: SlotControl) {
        self.0.borrow_mut().post(&CtlMsg::SlotControl(c));
    }

    fn post(&mut self, msg: CtlMsg) {
        self.0.borrow_mut().post(&msg);
    }

    fn poll_msg(&mut self) -> Result<Option<HostMsg>, Failure> {
        let mut s = self.0.borrow_mut();
        s.pump_song();
        Ok(s.acks.pop_front())
    }

    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        let mut s = self.0.borrow_mut();
        s.pump_song();
        out.extend(s.acks.drain(..));
    }

    fn now(&self) -> f64 {
        self.0.borrow().now
    }

    fn swap_graph(&mut self, g: GraphHandle) {
        let mut s = self.0.borrow_mut();
        let mut bytes = Vec::new();
        let encoded = match &g {
            GraphHandle::Inst { def, .. } => encode_inst(def, &mut bytes),
            GraphHandle::Bus { def, .. } => encode_bus(def, false, &mut bytes),
            GraphHandle::Master(def) => encode_bus(def, true, &mut bytes),
        };
        if encoded.is_err() || bytes.len() > SLICE_BYTES {
            let d = Diagnostic::error(
                DiagCode::GraphTooLarge,
                Span::new(FileId::new(0), 0, 0),
                format!("graph encoding of {} bytes exceeds one slice", bytes.len()),
            );
            console(&format!("diag {d}"));
            return;
        }
        s.next_graph += 1;
        let (id, gen) = (s.next_graph, s.gen());
        let mut rec = Vec::new();
        encode_graph_record(id, gen, &bytes, &mut rec);
        push_record(&rec);
    }

    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.0.borrow_mut().install(id, data);
    }

    fn retire_sample(&mut self, id: u32) {
        self.0.borrow_mut().retire(id);
    }

    fn analysis(&self) -> HostSigs {
        self.0.borrow().sigs
    }
}

/// The browser cell channel: records into the outbox; acknowledgments come
/// back through `WasmAudioHost::drain`.
pub struct WasmCellPort(pub Rc<RefCell<HostState>>);

impl CellPort for WasmCellPort {
    fn post(&mut self, msg: CtlMsg) {
        self.0.borrow_mut().post(&msg);
    }

    fn post_batch(&mut self, seq: u32, entries: &[(CellId, u32, f32)]) {
        let mut rec = vec![0u8; crate::host::wire::batch_len(entries.len())];
        let n = encode_batch(seq, entries, &mut rec);
        push_record(&rec[..n]);
    }

    fn drain(&mut self, _out: &mut Vec<HostMsg>) {}
}

/// Samples JS decoded (`decodeAudioData`) and handed over with `sample_put`,
/// keyed `bank:index` or by path text.
#[derive(Clone, Default)]
pub struct WasmSamples(pub Rc<RefCell<BTreeMap<String, Arc<SampleData>>>>);

/// The loader key of a sample source.
#[must_use]
pub fn sample_key(src: &SampleSrc) -> String {
    match src {
        SampleSrc::Bank { kw, index } => format!("{}:{index}", name_of_kw(*kw)),
        SampleSrc::Path(p) => p.text.to_string(),
        // A captured or rendered buffer is never loaded by the page; SS-ANALYSIS
        // installs its frames at commit (design 14.5.9).
        SampleSrc::Buffer { id } => format!("buffer:{id}"),
    }
}

impl SampleLoader for WasmSamples {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let key = sample_key(src);
        self.0.borrow().get(&key).cloned().ok_or_else(|| {
            Failure::new(
                FailCode::HostUnavailable,
                format!("the page has not provided the sample `{key}`"),
            )
        })
    }
}

/// Snapshot the decoded page map without retaining its mutable alias.
impl WasmSamples {
    #[must_use]
    pub fn song_factory(
        &self,
        complete_banks: BTreeMap<crate::value::intern::KwId, Vec<String>>,
        sources: BTreeMap<String, (FileId, Rc<str>)>,
    ) -> Rc<dyn crate::song::assets::SongAssetFactory> {
        Rc::new(crate::song::assets::DecodedSongAssetFactory::new(
            self.0.borrow().clone(),
            complete_banks,
            sources,
        ))
    }
}
