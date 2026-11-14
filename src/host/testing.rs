//! Test hosts and mock transports (design 12.8.5), test builds only.
//!
//! Recording hosts log every call in order with the mock-clock time it
//! arrived; clones share the log, so a test keeps a handle after boxing the
//! host into `Hosts`. The two transports model the tiers' cell and control
//! delivery: `NativeTransport` shares `AtomicCells` and delivers at once,
//! `BrowserTransport` moves encoded bytes through a FIFO with delay, loss
//! and stall into an isolated `Mirror` (no shared-memory shortcut).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use crate::dsp::cells::{AtomicCells, CellId, Mirror};
use crate::host::caps::{
    AudioHost, CaptureId, CapturePoll, GraphHandle, HostSigs, MidiEvent, MidiHost, OscEvent,
    OscHost, RenderHost, SampleData, TapReader, TapSrc,
};
use crate::host::wire::{
    batch_len, decode_batch, encode_batch, AudioEvent, CtlMsg, HostMsg, SlotControl, SlotControlAck,
};
use crate::tex::shader::ShaderDesc;
use crate::tex::texnode::OutId;
use crate::tex::uniforms::Uniforms;
use crate::vm::fail::{FailCode, Failure};

/// A call log shared by a recording host and its clones: `(arrival, call)`.
type Log<C> = Rc<RefCell<Vec<(f64, C)>>>;

/// A settable clock shared by the recording hosts (host seconds).
#[derive(Clone, Debug, Default)]
pub struct MockClock(Rc<Cell<f64>>);

impl MockClock {
    pub fn new(now: f64) -> Self {
        Self(Rc::new(Cell::new(now)))
    }

    pub fn now(&self) -> f64 {
        self.0.get()
    }

    pub fn set(&self, now: f64) {
        self.0.set(now);
    }

    pub fn advance(&self, dt: f64) {
        self.0.set(self.0.get() + dt);
    }
}

/// One `AudioHost` call.
#[derive(Clone, PartialEq, Debug)]
pub enum AudioCall {
    Send(AudioEvent),
    Control(SlotControl),
    Post(CtlMsg),
    SwapGraph(GraphHandle),
    InstallSample(u32, Arc<SampleData>),
    RetireSample(u32),
}

/// The sample rate of the recording host's synthetic taps and captures.
pub const SYNTH_RATE: u32 = 48_000;
/// The synthetic tap frequency of `:master` (bin 8 of a 128-point FFT at
/// `SYNTH_RATE`), Hz.
pub const SYNTH_MASTER_HZ: f64 = 3000.0;
/// The synthetic tap frequency of every named bus, Hz.
pub const SYNTH_BUS_HZ: f64 = 6000.0;
/// The synthetic tap amplitude.
pub const SYNTH_AMP: f64 = 0.5;

/// Sample `frame` of the synthetic signal of `src`.
#[must_use]
pub fn synth_sample(src: &TapSrc, frame: u64) -> f32 {
    let hz = match src {
        TapSrc::Master => SYNTH_MASTER_HZ,
        TapSrc::Bus(_) => SYNTH_BUS_HZ,
    };
    #[allow(clippy::cast_precision_loss)]
    let t = frame as f64 / f64::from(SYNTH_RATE);
    #[allow(clippy::cast_possible_truncation)]
    let x = (SYNTH_AMP * (std::f64::consts::TAU * hz * t).sin()) as f32;
    x
}

/// A deterministic tap reader: a sine at a fixed frequency per source,
/// ending at the current mock-clock frame.
#[derive(Clone, Debug, Default)]
pub struct SynthTaps {
    pub clock: MockClock,
}

impl TapReader for SynthTaps {
    fn snapshot(&mut self, src: &TapSrc, frames: usize, out: &mut Vec<f32>) -> Result<(), Failure> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end = (self.clock.now().max(0.0) * f64::from(SYNTH_RATE)) as u64;
        let first = end.saturating_sub(frames as u64);
        out.clear();
        out.extend((0..frames as u64).map(|k| synth_sample(src, first + k)));
        Ok(())
    }
}

/// An armed synthetic capture: `(src, start frame, frames, delivered)`.
type SynthCapture = (TapSrc, u64, u64, u64);

/// Records every `AudioHost` call; `drain` returns the queued `replies`.
/// Its taps are `SynthTaps`; a capture completes once `frames` worth of
/// mock-clock time has passed after its start.
#[derive(Clone, Debug, Default)]
pub struct RecordingAudioHost {
    pub clock: MockClock,
    calls: Log<AudioCall>,
    replies: Rc<RefCell<Vec<HostMsg>>>,
    sigs: Rc<Cell<HostSigs>>,
    captures: Rc<RefCell<Vec<Option<SynthCapture>>>>,
}

impl RecordingAudioHost {
    pub fn new(clock: MockClock) -> Self {
        Self {
            clock,
            ..Self::default()
        }
    }

    /// Every call so far, with its arrival time.
    pub fn calls(&self) -> Vec<(f64, AudioCall)> {
        self.calls.borrow().clone()
    }

    /// Queues a record for the next `drain`.
    pub fn reply(&self, msg: HostMsg) {
        self.replies.borrow_mut().push(msg);
    }

    /// Sets what `analysis` returns.
    pub fn set_sigs(&self, sigs: HostSigs) {
        self.sigs.set(sigs);
    }

    fn log(&self, call: AudioCall) {
        self.calls.borrow_mut().push((self.clock.now(), call));
    }
}

impl AudioHost for RecordingAudioHost {
    fn send(&mut self, ev: AudioEvent) {
        self.log(AudioCall::Send(ev));
    }
    fn control(&mut self, c: SlotControl) {
        self.log(AudioCall::Control(c));
    }
    fn post(&mut self, msg: CtlMsg) {
        self.log(AudioCall::Post(msg));
    }
    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        out.append(&mut self.replies.borrow_mut());
    }
    fn now(&self) -> f64 {
        self.clock.now()
    }
    fn swap_graph(&mut self, g: GraphHandle) {
        self.log(AudioCall::SwapGraph(g));
    }
    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.log(AudioCall::InstallSample(id, data));
    }
    fn retire_sample(&mut self, id: u32) {
        self.log(AudioCall::RetireSample(id));
    }
    fn analysis(&self) -> HostSigs {
        self.sigs.get()
    }
    fn tap_reader(&mut self) -> Option<Box<dyn TapReader>> {
        Some(Box::new(SynthTaps {
            clock: self.clock.clone(),
        }))
    }
    fn arm_capture(
        &mut self,
        src: &TapSrc,
        start: f64,
        frames: usize,
    ) -> Result<CaptureId, Failure> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = (start.max(0.0) * f64::from(SYNTH_RATE)).round() as u64;
        let mut caps = self.captures.borrow_mut();
        caps.push(Some((*src, start, frames as u64, 0)));
        Ok(CaptureId::new(u32::try_from(caps.len() - 1).unwrap_or(0)))
    }
    fn poll_capture(&mut self, id: CaptureId, out: &mut Vec<f32>) -> CapturePoll {
        let mut caps = self.captures.borrow_mut();
        let Some(Some((src, start, frames, delivered))) = caps.get_mut(id.get() as usize) else {
            return CapturePoll::Failed(Failure::new(FailCode::HostUnavailable, "no such capture"));
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let now = (self.clock.now().max(0.0) * f64::from(SYNTH_RATE)) as u64;
        let ready = now.saturating_sub(*start).min(*frames);
        for k in *delivered..ready {
            let x = synth_sample(src, *start + k);
            out.extend([x, x]);
        }
        *delivered = ready.max(*delivered);
        if *delivered == *frames {
            caps[id.get() as usize] = None;
            CapturePoll::Done
        } else {
            CapturePoll::Pending
        }
    }
}

/// One `MidiHost` or `OscHost` call.
#[derive(Clone, PartialEq, Debug)]
pub enum SinkCall<E> {
    Send(E),
    Control(SlotControl),
}

/// Records every call of an event sink.
#[derive(Clone, Debug)]
pub struct RecordingSink<E> {
    pub clock: MockClock,
    calls: Log<SinkCall<E>>,
}

impl<E: Clone> RecordingSink<E> {
    pub fn new(clock: MockClock) -> Self {
        Self {
            clock,
            calls: Rc::default(),
        }
    }

    /// Every call so far, with its arrival time.
    pub fn calls(&self) -> Vec<(f64, SinkCall<E>)> {
        self.calls.borrow().clone()
    }

    fn log(&self, call: SinkCall<E>) {
        self.calls.borrow_mut().push((self.clock.now(), call));
    }
}

/// Records every `MidiHost` call.
pub type RecordingMidiHost = RecordingSink<MidiEvent>;

/// Records every `OscHost` call.
pub type RecordingOscHost = RecordingSink<OscEvent>;

impl MidiHost for RecordingSink<MidiEvent> {
    fn send(&mut self, ev: MidiEvent) {
        self.log(SinkCall::Send(ev));
    }
    fn control(&mut self, c: SlotControl) {
        self.log(SinkCall::Control(c));
    }
}

impl OscHost for RecordingSink<OscEvent> {
    fn send(&mut self, ev: OscEvent) {
        self.log(SinkCall::Send(ev));
    }
    fn control(&mut self, c: SlotControl) {
        self.log(SinkCall::Control(c));
    }
}

/// One `RenderHost` call.
#[derive(Clone, PartialEq, Debug)]
pub enum RenderCall {
    SetProgram(OutId, ShaderDesc),
    SetUniforms(OutId, Uniforms),
}

/// Records every `RenderHost` call.
#[derive(Clone, Debug, Default)]
pub struct RecordingRenderHost {
    pub clock: MockClock,
    calls: Log<RenderCall>,
}

impl RecordingRenderHost {
    pub fn new(clock: MockClock) -> Self {
        Self {
            clock,
            calls: Rc::default(),
        }
    }

    /// Every call so far, with its arrival time.
    pub fn calls(&self) -> Vec<(f64, RenderCall)> {
        self.calls.borrow().clone()
    }
}

impl RenderHost for RecordingRenderHost {
    fn set_program(&mut self, o: OutId, sh: ShaderDesc) {
        let now = self.clock.now();
        self.calls
            .borrow_mut()
            .push((now, RenderCall::SetProgram(o, sh)));
    }
    fn set_uniforms(&mut self, o: OutId, u: &Uniforms) {
        let now = self.clock.now();
        self.calls
            .borrow_mut()
            .push((now, RenderCall::SetUniforms(o, u.clone())));
    }
}

/// The native tier: cells are shared `AtomicCells`, controls are applied and
/// acknowledged at once. No cell message is ever sent.
#[derive(Debug)]
pub struct NativeTransport {
    cells: AtomicCells,
    delivered: Vec<CtlMsg>,
    acks: Vec<HostMsg>,
}

impl NativeTransport {
    pub fn new(capacity: usize) -> Self {
        Self {
            cells: AtomicCells::new(capacity),
            delivered: Vec::new(),
            acks: Vec::new(),
        }
    }

    /// Writes a cell; the audio side sees it on its next read.
    pub fn write_cell(&mut self, cell: CellId, value: f32) -> bool {
        self.cells.set(cell, value)
    }

    /// Delivers a priority record at once; a `SlotControl` is acknowledged.
    pub fn post(&mut self, msg: CtlMsg) {
        if let CtlMsg::SlotControl(c) = msg {
            self.acks.push(HostMsg::SlotControlAck(SlotControlAck {
                slot: c.slot,
                gen: c.new_gen,
            }));
        }
        self.delivered.push(msg);
    }

    /// Moves the acknowledgments into `out`.
    pub fn drain(&mut self, out: &mut Vec<HostMsg>) {
        out.append(&mut self.acks);
    }

    /// The audio side's view of the cells.
    pub fn audio_cells(&self) -> &AtomicCells {
        &self.cells
    }

    /// The records the audio side received, in order.
    pub fn delivered(&self) -> &[CtlMsg] {
        &self.delivered
    }
}

/// One direction of the browser FIFO: encoded records with their due tick.
#[derive(Debug, Default)]
struct Fifo {
    queue: VecDeque<(u64, Vec<u8>)>,
    drop_next: u32,
    dropped: u32,
}

impl Fifo {
    fn push(&mut self, due: u64, bytes: Vec<u8>) {
        if self.drop_next > 0 {
            self.drop_next -= 1;
            self.dropped += 1;
        } else {
            self.queue.push_back((due, bytes));
        }
    }

    fn pop_due(&mut self, now: u64) -> Option<Vec<u8>> {
        match self.queue.front() {
            Some((due, _)) if *due <= now => self.queue.pop_front().map(|(_, b)| b),
            _ => None,
        }
    }
}

/// The browser tier: an isolated `Mirror` behind a FIFO port. Records are
/// encoded to bytes on post and decoded on delivery, `delay` ticks later in
/// each direction; loss drops the next records; a stall holds everything
/// until resume. The mirror applies at most one batch per tick (the 16.1
/// credit), and returns acks through the same discipline.
#[derive(Debug)]
pub struct BrowserTransport {
    delay: u64,
    tick: u64,
    stalled: bool,
    to_audio: Fifo,
    to_eval: Fifo,
    mirror: Mirror,
    delivered: Vec<CtlMsg>,
    ready: Vec<HostMsg>,
}

impl BrowserTransport {
    pub fn new(capacity: usize, delay: u64) -> Self {
        Self {
            delay,
            tick: 0,
            stalled: false,
            to_audio: Fifo::default(),
            to_eval: Fifo::default(),
            mirror: Mirror::new(capacity),
            delivered: Vec::new(),
            ready: Vec::new(),
        }
    }

    pub fn set_delay(&mut self, delay: u64) {
        self.delay = delay;
    }

    /// Drops the next `n` records sent to the audio side.
    pub fn drop_next(&mut self, n: u32) {
        self.to_audio.drop_next += n;
    }

    /// Drops the next `n` acknowledgments sent back.
    pub fn drop_next_acks(&mut self, n: u32) {
        self.to_eval.drop_next += n;
    }

    /// The number of records lost so far `(to audio, to evaluator)`.
    pub fn dropped(&self) -> (u32, u32) {
        (self.to_audio.dropped, self.to_eval.dropped)
    }

    pub fn stall(&mut self) {
        self.stalled = true;
    }

    pub fn resume(&mut self) {
        self.stalled = false;
    }

    /// Records in flight toward the audio side.
    pub fn in_flight(&self) -> usize {
        self.to_audio.queue.len()
    }

    /// Posts one priority record.
    pub fn post(&mut self, msg: CtlMsg) {
        let mut buf = vec![0; CtlMsg::MAX_LEN];
        let n = msg.encode(&mut buf);
        buf.truncate(n);
        self.to_audio.push(self.tick + self.delay, buf);
    }

    /// Posts a `CellBatch` with its entries.
    pub fn post_batch(&mut self, seq: u32, entries: &[(CellId, u32, f32)]) {
        let mut buf = vec![0; batch_len(entries.len())];
        let n = encode_batch(seq, entries, &mut buf);
        buf.truncate(n);
        self.to_audio.push(self.tick + self.delay, buf);
    }

    /// Advances one tick: unless stalled, the audio side applies every due
    /// record in order, stopping after one batch, and due acks become
    /// ready for `drain`.
    pub fn tick(&mut self) {
        self.tick += 1;
        if self.stalled {
            return;
        }
        while let Some(bytes) = self.to_audio.pop_due(self.tick) {
            let batch = self.deliver(&bytes);
            if batch {
                break;
            }
        }
        while let Some(bytes) = self.to_eval.pop_due(self.tick) {
            if let Ok((msg, _)) = HostMsg::decode(&bytes) {
                self.ready.push(msg);
            }
        }
    }

    /// Applies one record on the audio side; returns whether it was a batch.
    fn deliver(&mut self, bytes: &[u8]) -> bool {
        let Ok((msg, _)) = CtlMsg::decode(bytes) else {
            return false;
        };
        let (ack, batch) = match msg {
            CtlMsg::CellBatch { .. } => match decode_batch(bytes) {
                Ok((view, _)) => (self.mirror.apply_batch(view.seq, view.iter()), true),
                Err(_) => (None, true),
            },
            CtlMsg::CellInit { cell, epoch, value } => {
                (self.mirror.apply_init(cell, epoch, value), false)
            }
            CtlMsg::CellRetire { cell, epoch } => {
                // No audio-side user in this model: retirement completes at once.
                let ack = if self.mirror.retire(cell, epoch) {
                    self.mirror.retired(cell)
                } else {
                    None
                };
                (ack, false)
            }
            CtlMsg::SlotControl(c) => (
                Some(HostMsg::SlotControlAck(SlotControlAck {
                    slot: c.slot,
                    gen: c.new_gen,
                })),
                false,
            ),
            _ => (None, false),
        };
        self.delivered.push(msg);
        if let Some(ack) = ack {
            let mut buf = vec![0; HostMsg::MAX_LEN];
            let n = ack.encode(&mut buf);
            buf.truncate(n);
            self.to_eval.push(self.tick + self.delay, buf);
        }
        batch
    }

    /// Moves the acknowledgments that have arrived into `out`.
    pub fn drain(&mut self, out: &mut Vec<HostMsg>) {
        out.append(&mut self.ready);
    }

    /// The audio side's cells.
    pub fn mirror(&self) -> &Mirror {
        &self.mirror
    }

    /// The records the audio side received, in order.
    pub fn delivered(&self) -> &[CtlMsg] {
        &self.delivered
    }
}
