//! The native audio host (design 12.2, 12.8.4, 12.8.5, 12.8.10, 16).
//!
//! `NativeAudioHost` is the evaluator half: it owns the producer ends of the
//! event ring and the priority control channel, the ack and garbage
//! consumers, the shared `AtomicCells`, and the frame clock. `AudioSide` is
//! the audio half: the `Engine` and the consumer ends. The cpal callback
//! owns the `AudioSide` and only calls `AudioSide::render`, which runs
//! `Engine::process` over preallocated buffers and then stores the frame
//! counter and the host signals in atomics. It allocates nothing and takes
//! no lock (17 invariant 3).
//!
//! Resource ids: samples keep the scheduler's `SampleTable` ids; graph
//! installs take fresh ids from `GRAPH_RESOURCE_BASE` up, so an
//! `Installed`/`Retired` ack of a graph never names a sample. Every install
//! is generation 1 of its resource id; the engine retires the replaced
//! template or bus by instrument (or bus) identity and hands the box back
//! through the garbage ring, which `drain` empties on this thread.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SupportedStreamConfig};

use crate::dsp::arena::StoreKind;
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::AtomicCells;
use crate::dsp::engine::{Engine, EngineConfig, EngineIo};
use crate::dsp::ring::{
    AckConsumer, AckProducer, Consumer, EventConsumer, EventProducer, EventRing, Garbage,
    NativeInstall, NativeRecord, Producer, SpscRing, CHANNEL_CAPACITY, EVENT_CAPACITY,
};
use crate::dsp::ugen::{BuildEnv, Template};
use crate::host::caps::{AudioHost, GraphHandle, HostSigs, SampleData};
use crate::host::native::{unavailable, NativeConfig};
use crate::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Diagnostic};

/// The engine block: longer callbacks render in pieces of this size.
pub const MAX_BLOCK: usize = 512;
/// Priority control channel records (installs travel here too).
pub const CONTROL_CAPACITY: usize = 4 * CHANNEL_CAPACITY;
/// Audio -> evaluator records. One `process()` sends at most a few acks,
/// one counter record and 16 analysis cells; at a 5 ms drain cadence this
/// holds several seconds of traffic, so install and retire acks are not
/// lost behind analysis cells.
pub const ACK_CAPACITY: usize = 8192;
/// Retired structures waiting to be dropped on the evaluator thread.
pub const GARBAGE_CAPACITY: usize = 1024;
/// The first resource id of a graph install.
pub const GRAPH_RESOURCE_BASE: u32 = 0x4000_0000;

/// The audio timebase: frames rendered / sample rate (12.8.4). Clones
/// share the counter; the callback advances it after each block.
#[derive(Clone, Debug)]
pub struct FrameClock {
    frames: Arc<AtomicU64>,
    rate: u32,
}

impl FrameClock {
    /// A clock at frame 0.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            frames: Arc::new(AtomicU64::new(0)),
            rate: sample_rate.max(1),
        }
    }

    /// Host seconds.
    #[must_use]
    pub fn now(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let frames = self.frames.load(Ordering::Acquire) as f64;
        frames / f64::from(self.rate)
    }

    /// Frames rendered so far.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// Counts `n` rendered frames (release store; the callback's only
    /// write besides the host signals).
    pub fn advance(&self, n: u64) {
        let now = self.frames.load(Ordering::Relaxed);
        self.frames.store(now.wrapping_add(n), Ordering::Release);
    }
}

/// `HostSigs` as atomic `f32` bit patterns: `amp`, then the 8 fft bands.
#[derive(Debug, Default)]
struct SharedSigs([AtomicU32; 9]);

impl SharedSigs {
    fn store(&self, s: &HostSigs) {
        self.0[0].store(s.amp.to_bits(), Ordering::Release);
        for (slot, v) in self.0[1..].iter().zip(s.fft.iter()) {
            slot.store(v.to_bits(), Ordering::Release);
        }
    }

    fn load(&self) -> HostSigs {
        let get = |i: usize| f32::from_bits(self.0[i].load(Ordering::Acquire));
        let mut fft = [0.0; 8];
        for (i, v) in fft.iter_mut().enumerate() {
            *v = get(i + 1);
        }
        HostSigs { amp: get(0), fft }
    }
}

/// The audio half: everything the callback touches, allocated up front.
pub struct AudioSide {
    engine: Engine,
    events: EventConsumer,
    controls: Consumer<NativeRecord>,
    acks: AckProducer,
    garbage: Producer<Garbage>,
    cells: AtomicCells,
    clock: FrameClock,
    sigs: Arc<SharedSigs>,
    /// Interleaved stereo for devices that are not stereo.
    scratch: Box<[f32]>,
}

impl AudioSide {
    /// The callback body: fills `out` (interleaved, `channels` per frame).
    /// Stereo renders in place; mono takes the mid signal; wider devices
    /// get left and right on their first two channels and silence on the
    /// rest.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        if channels == 2 {
            let frames = out.len() / 2;
            self.process(Target::Out(out), frames);
        } else if channels == 0 {
            out.fill(0.0);
        } else {
            let total = out.len() / channels;
            let mut done = 0;
            while done < total {
                let n = (total - done).min(MAX_BLOCK);
                self.process(Target::Scratch, n);
                let frames = &mut out[done * channels..(done + n) * channels];
                for (f, lr) in frames
                    .chunks_exact_mut(channels)
                    .zip(self.scratch.chunks_exact(2))
                {
                    if channels == 1 {
                        f[0] = 0.5 * (lr[0] + lr[1]);
                    } else {
                        f[0] = lr[0];
                        f[1] = lr[1];
                        f[2..].fill(0.0);
                    }
                }
                done += n;
            }
        }
        self.sigs.store(&self.engine.host_sigs());
    }

    fn process(&mut self, target: Target<'_>, frames: usize) {
        let Self {
            engine,
            events,
            controls,
            acks,
            garbage,
            cells,
            clock,
            scratch,
            ..
        } = self;
        let mut io = EngineIo {
            events,
            controls,
            acks,
            cells,
            garbage: Some(garbage),
        };
        let buf = match target {
            Target::Out(out) => out,
            Target::Scratch => &mut scratch[..],
        };
        engine.process(&mut io, buf, frames);
        clock.advance(frames as u64);
    }
}

enum Target<'a> {
    Out(&'a mut [f32]),
    Scratch,
}

/// The evaluator half of the native audio host.
pub struct NativeAudioHost {
    events: EventProducer,
    controls: Producer<NativeRecord>,
    acks: AckConsumer,
    garbage: Consumer<Garbage>,
    cells: AtomicCells,
    clock: FrameClock,
    sigs: Arc<SharedSigs>,
    env: BuildEnv,
    next_graph: u32,
    dropped: u32,
    reported: u32,
    diags: Vec<Diagnostic>,
    stream_errors: Arc<AtomicU32>,
    stream: Option<cpal::Stream>,
}

impl NativeAudioHost {
    /// Opens the default output device as an f32 stream and starts it.
    ///
    /// # Errors
    /// `beyond-capability` ("not available on this host") when there is no
    /// output device, no f32 configuration, or the stream cannot start.
    pub fn open(cfg: &NativeConfig) -> Result<Self, Diagnostic> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| {
            unavailable("audio output is not available on this host (no output device)")
        })?;
        let supported = f32_config(&device)?;
        let channels = usize::from(supported.channels());
        let rate = supported.sample_rate().0;
        let config = supported.config();
        #[allow(clippy::cast_precision_loss)]
        let engine = EngineConfig::new(
            &CapabilitySet::native(),
            rate as f32,
            MAX_BLOCK,
            StoreKind::NativeArc,
        );
        let (mut this, mut side) = Self::pair(engine, AtomicCells::new(cfg.cells));
        let errors = Arc::clone(&this.stream_errors);
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    side.render(data, channels);
                },
                move |_| {
                    errors.fetch_add(1, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|e| {
                unavailable(format!("audio output is not available on this host ({e})"))
            })?;
        stream.play().map_err(|e| {
            unavailable(format!("audio output is not available on this host ({e})"))
        })?;
        this.stream = Some(stream);
        Ok(this)
    }

    /// A host with no device: the caller drives `AudioSide::render` (tests,
    /// offline use). Same rings and capacities as `open`.
    #[must_use]
    pub fn headless(sample_rate: u32, caps: CapabilitySet, cells: usize) -> (Self, AudioSide) {
        #[allow(clippy::cast_precision_loss)]
        let engine = EngineConfig::new(&caps, sample_rate as f32, MAX_BLOCK, StoreKind::NativeArc);
        Self::pair(engine, AtomicCells::new(cells))
    }

    fn pair(cfg: EngineConfig, cells: AtomicCells) -> (Self, AudioSide) {
        let engine = Engine::with_config(cfg);
        let env = engine.build_env();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let clock = FrameClock::new(cfg.sample_rate as u32);
        let sigs = Arc::new(SharedSigs::default());
        let (events, events_rx) = EventRing::split(EVENT_CAPACITY);
        let (controls, controls_rx) = SpscRing::split(CONTROL_CAPACITY);
        let (acks_tx, acks) = SpscRing::split(ACK_CAPACITY);
        let (garbage_tx, garbage) = SpscRing::split(GARBAGE_CAPACITY);
        let side = AudioSide {
            engine,
            events: events_rx,
            controls: controls_rx,
            acks: acks_tx,
            garbage: garbage_tx,
            cells: cells.clone(),
            clock: clock.clone(),
            sigs: Arc::clone(&sigs),
            scratch: vec![0.0; 2 * MAX_BLOCK].into_boxed_slice(),
        };
        let host = NativeAudioHost {
            events,
            controls,
            acks,
            garbage,
            cells,
            clock,
            sigs,
            env,
            next_graph: GRAPH_RESOURCE_BASE,
            dropped: 0,
            reported: 0,
            diags: Vec::new(),
            stream_errors: Arc::new(AtomicU32::new(0)),
            stream: None,
        };
        (host, side)
    }

    /// The cell table the callback reads; give the same one to the
    /// runtime (`Tier::Native`).
    #[must_use]
    pub fn cells(&self) -> AtomicCells {
        self.cells.clone()
    }

    /// The audio timebase.
    #[must_use]
    pub fn clock(&self) -> FrameClock {
        self.clock.clone()
    }

    /// Records dropped because a ring or channel was full.
    #[must_use]
    pub fn dropped(&self) -> u32 {
        self.dropped
    }

    /// Errors the cpal stream reported.
    #[must_use]
    pub fn stream_errors(&self) -> u32 {
        self.stream_errors.load(Ordering::Relaxed)
    }

    /// Host diagnostics (`ring-overflow`, `graph-too-large`) since the last
    /// call.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diags)
    }

    fn overflow(&mut self, what: &str) {
        self.dropped = self.dropped.saturating_add(1);
        self.diags.push(Diagnostic::error(
            DiagCode::RingOverflow,
            Span::new(FileId::CONSOLE, 0, 0),
            format!("the audio {what} ring is full; the record was dropped"),
        ));
    }

    fn post_record(&mut self, rec: NativeRecord) {
        if self.controls.push(rec).is_err() {
            self.overflow("control");
        }
    }

    fn fresh_graph(&mut self) -> u32 {
        let id = self.next_graph;
        self.next_graph = self.next_graph.wrapping_add(1).max(GRAPH_RESOURCE_BASE);
        id
    }

    fn build_failed(&mut self, what: &str, e: &dyn std::fmt::Debug) {
        self.diags.push(Diagnostic::error(
            DiagCode::GraphTooLarge,
            Span::new(FileId::CONSOLE, 0, 0),
            format!("the {what} graph cannot be built for the audio engine ({e:?})"),
        ));
    }

    fn collect_garbage(&mut self) {
        while self.garbage.pop().is_some() {}
    }
}

impl AudioHost for NativeAudioHost {
    fn send(&mut self, ev: AudioEvent) {
        if self.events.push(ev).is_err() {
            self.overflow("event");
        }
    }

    fn control(&mut self, c: SlotControl) {
        self.post_record(NativeRecord::Msg(CtlMsg::SlotControl(c)));
    }

    fn post(&mut self, msg: CtlMsg) {
        self.post_record(NativeRecord::Msg(msg));
    }

    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        while let Some(m) = self.acks.pop() {
            out.push(m);
        }
        self.collect_garbage();
        if self.dropped != self.reported {
            out.push(HostMsg::Counters {
                late: 0,
                dropped: self.dropped - self.reported,
                stolen: 0,
                skipped: 0,
            });
            self.reported = self.dropped;
        }
    }

    fn now(&self) -> f64 {
        self.clock.now()
    }

    fn swap_graph(&mut self, g: GraphHandle) {
        self.collect_garbage();
        let install = match g {
            GraphHandle::Inst { def, .. } => match Template::from_inst(&def, &self.env) {
                Ok(template) => NativeInstall::Inst {
                    resource: self.fresh_graph(),
                    gen: 1,
                    template,
                },
                Err(e) => return self.build_failed("instrument", &e),
            },
            GraphHandle::Bus { def, .. } => match BusTemplate::from_def(&def) {
                Ok(t) => NativeInstall::Bus {
                    resource: self.fresh_graph(),
                    gen: 1,
                    master: false,
                    template: Box::new(t),
                },
                Err(e) => return self.build_failed("bus", &e),
            },
            GraphHandle::Master(def) => match BusTemplate::from_def(&def) {
                Ok(t) => NativeInstall::Bus {
                    resource: self.fresh_graph(),
                    gen: 1,
                    master: true,
                    template: Box::new(t),
                },
                Err(e) => return self.build_failed("master", &e),
            },
        };
        self.post_record(NativeRecord::Install(install));
    }

    fn install_sample(&mut self, id: u32, data: Arc<SampleData>) {
        self.collect_garbage();
        self.post_record(NativeRecord::Install(NativeInstall::Sample {
            resource: id,
            gen: 1,
            data,
        }));
    }

    fn retire_sample(&mut self, id: u32) {
        self.post_record(NativeRecord::Msg(CtlMsg::SampleRetire { resource: id }));
    }

    fn analysis(&self) -> HostSigs {
        self.sigs.load()
    }
}

/// The device's default configuration when it is f32, else an f32 range
/// at the default rate (or the range's highest rate).
fn f32_config(device: &cpal::Device) -> Result<SupportedStreamConfig, Diagnostic> {
    let default = device
        .default_output_config()
        .map_err(|e| unavailable(format!("audio output is not available on this host ({e})")))?;
    if default.sample_format() == SampleFormat::F32 {
        return Ok(default);
    }
    let rate = default.sample_rate();
    let ranges = device
        .supported_output_configs()
        .map_err(|e| unavailable(format!("audio output is not available on this host ({e})")))?;
    let mut fallback = None;
    for r in ranges.filter(|r| r.sample_format() == SampleFormat::F32) {
        if r.min_sample_rate() <= rate && rate <= r.max_sample_rate() {
            return Ok(r.with_sample_rate(rate));
        }
        fallback.get_or_insert(r.with_max_sample_rate());
    }
    fallback.ok_or_else(|| {
        unavailable("audio output is not available on this host (no f32 output format)")
    })
}
