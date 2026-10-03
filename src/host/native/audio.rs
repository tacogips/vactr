//! The native audio host (design 12.2, 12.8.4, 12.8.5, 12.8.10, 16).
//!
//! `NativeAudioHost` is the evaluator half: it owns the producer ends of the
//! event ring and the priority control channel, the ack and garbage
//! consumers, the shared `AtomicCells`, and the frame clock. `AudioSide` is
//! the audio half: the `Engine` and the consumer ends. The cpal callback
//! owns the `AudioSide` and calls `AudioSide::render` or, with `audio_in`,
//! the bounded input-queue bridge and `render_with_input`. Both run over
//! preallocated buffers and then store the frame
//! counter and the host signals in atomics, and copies each block into the
//! live tap rings and armed captures (`tap.rs`, 14.5.9). It allocates
//! nothing and takes no lock (17 invariant 3).
//!
//! Resource ids: samples keep the scheduler's `SampleTable` ids; graph
//! installs take fresh ids from `GRAPH_RESOURCE_BASE` up, so an
//! `Installed`/`Retired` ack of a graph never names a sample. Every install
//! is generation 1 of its resource id; the engine retires the replaced
//! template or bus by instrument (or bus) identity and hands the box back
//! through the garbage ring, which `drain` empties on this thread.

mod song;
mod song_capacity;
use song_capacity::SongCapacityCache;

use std::rc::Rc;
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
use crate::host::caps::{
    AudioHost, CaptureId, CapturePoll, GraphHandle, HostSigs, InstResolver, SampleData, TapReader,
    TapSrc,
};
use crate::host::native::capture::{self, CaptureCounters, CaptureStats, InputConsumer};
use crate::host::native::tap::{self, NativeTapReader, TapShared};
use crate::host::native::{unavailable, NativeConfig};
use crate::host::wire::{AudioEvent, CtlMsg, HostMsg, SlotControl};
use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Diagnostic, Severity};
use crate::vm::fail::Failure;

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
    taps: Arc<TapShared>,
    /// Interleaved stereo for devices that are not stereo.
    scratch: Box<[f32]>,
    /// Fixed stereo staging for a mono external source.
    input_scratch: Box<[f32]>,
}

impl AudioSide {
    #[cfg(test)]
    pub(crate) fn has_template(&self, id: crate::dsp::graph::InstId) -> bool {
        self.engine.has_template(id)
    }

    /// The callback body: fills `out` (interleaved, `channels` per frame).
    /// Stereo renders in place; mono takes the mid signal; wider devices
    /// get left and right on their first two channels and silence on the
    /// rest.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        self.render_impl(None, out, channels);
    }

    /// Renders with live external audio. Exactly one input sample per frame
    /// is duplicated to L/R; exactly two are interpreted as interleaved L/R.
    /// Any other input length or nonfinite sample is silenced and faulted.
    pub fn render_with_input(&mut self, input: &[f32], out: &mut [f32], channels: usize) {
        self.render_impl(Some(input), out, channels);
    }

    fn render_impl(&mut self, input: Option<&[f32]>, out: &mut [f32], channels: usize) {
        let total = if channels == 0 {
            0
        } else {
            out.len() / channels
        };
        let input_block = |done: usize, n: usize| match input {
            None => InputBlock::None,
            Some(samples) if samples.len() == total * 2 => {
                InputBlock::Stereo(&samples[2 * done..2 * (done + n)])
            }
            Some(samples) if samples.len() == total => InputBlock::Mono(&samples[done..done + n]),
            Some(_) => InputBlock::Bad,
        };
        if channels == 4 && self.engine.config().output_channels == 4 {
            let mut done = 0;
            while done < total {
                let n = (total - done).min(MAX_BLOCK);
                self.process_quad(&mut out[4 * done..4 * (done + n)], n, input_block(done, n));
                done += n;
            }
        } else if channels == 2 {
            // Engine-block pieces, so every bus block reaches the taps.
            let mut done = 0;
            loop {
                let n = (total - done).min(MAX_BLOCK);
                self.process(
                    Target::Out(&mut out[2 * done..2 * (done + n)]),
                    n,
                    input_block(done, n),
                );
                done += n;
                if done >= total {
                    break;
                }
            }
        } else if channels == 0 {
            out.fill(0.0);
        } else {
            let mut done = 0;
            while done < total {
                let n = (total - done).min(MAX_BLOCK);
                self.process(Target::Scratch, n, input_block(done, n));
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

    fn process(&mut self, target: Target<'_>, frames: usize, input: InputBlock<'_>) {
        let Self {
            engine,
            events,
            controls,
            acks,
            garbage,
            cells,
            clock,
            scratch,
            input_scratch,
            taps,
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
        let at = clock.frames();
        match input {
            InputBlock::None => engine.process(&mut io, buf, frames),
            InputBlock::Stereo(samples) => engine.process_with_input(&mut io, samples, buf, frames),
            InputBlock::Mono(samples) => {
                for (dst, &sample) in input_scratch[..2 * frames].chunks_exact_mut(2).zip(samples) {
                    dst.fill(sample);
                }
                engine.process_with_input(&mut io, &input_scratch[..2 * frames], buf, frames);
            }
            InputBlock::Bad => engine.process_with_input(&mut io, &[], buf, frames),
        }
        taps.record(buf, frames, at, engine.buses());
        clock.advance(frames as u64);
    }

    fn process_quad(&mut self, out: &mut [f32], frames: usize, input: InputBlock<'_>) {
        let Self {
            engine,
            events,
            controls,
            acks,
            garbage,
            cells,
            clock,
            scratch,
            input_scratch,
            taps,
            ..
        } = self;
        let mut io = EngineIo {
            events,
            controls,
            acks,
            cells,
            garbage: Some(garbage),
        };
        let at = clock.frames();
        match input {
            InputBlock::None => engine.process_four(&mut io, out, frames),
            InputBlock::Stereo(samples) => {
                engine.process_four_with_input(&mut io, samples, out, frames);
            }
            InputBlock::Mono(samples) => {
                for (dst, &sample) in input_scratch[..2 * frames].chunks_exact_mut(2).zip(samples) {
                    dst.fill(sample);
                }
                engine.process_four_with_input(&mut io, &input_scratch[..2 * frames], out, frames);
            }
            InputBlock::Bad => engine.process_four_with_input(&mut io, &[], out, frames),
        }
        for (pair, quad) in scratch[..2 * frames]
            .chunks_exact_mut(2)
            .zip(out.chunks_exact(4))
        {
            pair.copy_from_slice(&quad[..2]);
        }
        taps.record(&scratch[..2 * frames], frames, at, engine.buses());
        clock.advance(frames as u64);
    }
}

enum Target<'a> {
    Out(&'a mut [f32]),
    Scratch,
}

enum InputBlock<'a> {
    None,
    Stereo(&'a [f32]),
    Mono(&'a [f32]),
    Bad,
}

/// Shared by the hardware callback and deterministic capture-queue tests.
pub(crate) fn render_captured(
    side: &mut AudioSide,
    input: &mut InputConsumer,
    scratch: &mut [f32; 2 * MAX_BLOCK],
    output: &mut [f32],
    channels: usize,
) {
    if channels == 0 {
        output.fill(0.0);
        return;
    }
    let complete = output.len() / channels * channels;
    for chunk in output[..complete].chunks_mut(channels * MAX_BLOCK) {
        let frames = chunk.len() / channels;
        input.fill(&mut scratch[..2 * frames]);
        side.render_with_input(&scratch[..2 * frames], chunk, channels);
    }
    output[complete..].fill(0.0);
}

/// The evaluator half of the native audio host.
pub struct NativeAudioHost {
    song_capacity: SongCapacityCache,
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
    input_stream: Option<cpal::Stream>,
    capture_counters: Option<Arc<CaptureCounters>>,
    capture_reported: CaptureStats,
    taps: Arc<TapShared>,
    /// Resolves bus keywords for bus taps and captures (`set_bus_names`).
    bus_names: Option<Rc<dyn InstResolver>>,
}

impl NativeAudioHost {
    /// Opens the default output device as an f32 stream and starts it.
    ///
    /// # Errors
    /// `beyond-capability` ("not available on this host") when there is no
    /// output device, no f32 configuration, or the stream cannot start.
    pub fn open(cfg: &NativeConfig) -> Result<Self, Diagnostic> {
        Self::open_with_outputs(cfg, 2)
    }

    /// Opens a four-channel output device for the opt-in direct-stem contract.
    ///
    /// # Errors
    /// The default f32 output device cannot provide four channels.
    pub fn open_quad(cfg: &NativeConfig) -> Result<Self, Diagnostic> {
        Self::open_with_outputs(cfg, 4)
    }

    fn open_with_outputs(cfg: &NativeConfig, output_channels: u8) -> Result<Self, Diagnostic> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| {
            unavailable("audio output is not available on this host (no output device)")
        })?;
        let supported = f32_config(&device)?;
        let channels = usize::from(supported.channels());
        if output_channels == 4 && channels != 4 {
            return Err(unavailable(
                "quad output needs a four-channel default device configuration",
            ));
        }
        let rate = supported.sample_rate().0;
        let config = supported.config();
        #[allow(clippy::cast_precision_loss)]
        let engine = crate::host::song_profile::song_engine_config(
            rate as f32,
            MAX_BLOCK,
            CapabilitySet::native(),
            StoreKind::NativeArc,
            output_channels,
        )
        .map_err(|_| unavailable("unsupported song audio device configuration"))?;
        let (mut this, mut side) = Self::pair(engine, AtomicCells::new(cfg.cells));
        let capture = if cfg.audio_in {
            let input_device = host.default_input_device().ok_or_else(|| {
                unavailable("audio input was requested but no default input device is available")
            })?;
            let input_config = input_f32_config(&input_device, rate)?;
            let input_channels = usize::from(input_config.channels());
            let (mut producer, consumer, counters) = capture::pair();
            let input_errors = Arc::clone(&this.stream_errors);
            let input_stream = input_device
                .build_input_stream(
                    &input_config.config(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        producer.push_interleaved(data, input_channels);
                    },
                    move |_| {
                        input_errors.fetch_add(1, Ordering::Relaxed);
                    },
                    None,
                )
                .map_err(|e| {
                    unavailable(format!(
                        "audio input was requested but its stream cannot open ({e})"
                    ))
                })?;
            Some((input_stream, consumer, counters))
        } else {
            None
        };
        let errors = Arc::clone(&this.stream_errors);
        let (input_stream, mut input_consumer, counters) = match capture {
            Some((stream, consumer, counters)) => (Some(stream), Some(consumer), Some(counters)),
            None => (None, None, None),
        };
        let mut input_scratch = [0.0; 2 * MAX_BLOCK];
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if let Some(consumer) = &mut input_consumer {
                        render_captured(&mut side, consumer, &mut input_scratch, data, channels);
                    } else {
                        side.render(data, channels);
                    }
                },
                move |_| {
                    errors.fetch_add(1, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|e| {
                unavailable(format!("audio output is not available on this host ({e})"))
            })?;
        if let Some(input) = &input_stream {
            input.play().map_err(|e| {
                unavailable(format!(
                    "audio input was requested but its stream cannot start ({e})"
                ))
            })?;
        }
        stream.play().map_err(|e| {
            unavailable(format!("audio output is not available on this host ({e})"))
        })?;
        this.stream = Some(stream);
        this.input_stream = input_stream;
        this.capture_counters = counters;
        Ok(this)
    }

    /// A host with no device: the caller drives `AudioSide::render` (tests,
    /// offline use). Same rings as `open`; generic six-bus capacity is retained.
    #[must_use]
    pub fn headless(sample_rate: u32, caps: CapabilitySet, cells: usize) -> (Self, AudioSide) {
        #[allow(clippy::cast_precision_loss)]
        let engine = EngineConfig::new(&caps, sample_rate as f32, MAX_BLOCK, StoreKind::NativeArc);
        Self::pair(engine, AtomicCells::new(cells))
    }

    /// A headless native host with explicitly allocated Engine capacity.
    /// # Errors
    /// Invalid native storage, rate, or capacity/render configuration.
    pub fn headless_with_config(
        config: EngineConfig,
        cells: usize,
    ) -> Result<(Self, AudioSide), Failure> {
        let invalid = |message: &str| Failure::new(crate::vm::fail::FailCode::Type, message);
        config
            .validate()
            .map_err(|_| invalid("invalid native Engine configuration"))?;
        if !matches!(config.store, StoreKind::NativeArc) {
            return Err(invalid("native headless host requires NativeArc storage"));
        }
        if config.sample_rate.fract() != 0.0 {
            return Err(invalid(
                "native frame clock requires an integral sample rate",
            ));
        }
        if config.max_block < MAX_BLOCK {
            return Err(invalid(
                "native Engine quantum is smaller than render chunks",
            ));
        }
        Ok(Self::pair(config, AtomicCells::new(cells)))
    }

    /// A headless four-channel host for offline stems and tests.
    #[must_use]
    pub fn headless_quad(sample_rate: u32, caps: CapabilitySet, cells: usize) -> (Self, AudioSide) {
        #[allow(clippy::cast_precision_loss)]
        let mut engine =
            EngineConfig::new(&caps, sample_rate as f32, MAX_BLOCK, StoreKind::NativeArc);
        engine.output_channels = 4;
        Self::pair(engine, AtomicCells::new(cells))
    }

    fn pair(cfg: EngineConfig, cells: AtomicCells) -> (Self, AudioSide) {
        let mut engine = Engine::with_config(cfg);
        let env = engine.build_env();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let clock = FrameClock::new(cfg.sample_rate as u32);
        let sigs = Arc::new(SharedSigs::default());
        let taps = Arc::new(TapShared::new());
        let (events, events_rx) = EventRing::split(EVENT_CAPACITY);
        let (controls, controls_rx) = SpscRing::split(CONTROL_CAPACITY);
        let (acks_tx, acks) = SpscRing::split(ACK_CAPACITY);
        let ack_capacity = acks_tx.capacity();
        let (garbage_tx, garbage) = SpscRing::split(GARBAGE_CAPACITY);
        let staging = crate::dsp::engine::SongStagingConfig {
            preparations: 64,
            leases: 256,
            branches: 256,
            control_slots: u32::try_from(cells.capacity()).unwrap_or(u32::MAX),
            analysis_slots: u32::try_from(cfg.analysis_cells).unwrap_or(u32::MAX),
            native_pcm_bytes: crate::dsp::arena::DEFAULT_ARENA_BYTES as u64,
            critical_receipts: 2,
        };
        let configured = engine
            .configure_song_staging_with_transport(
                staging,
                u32::try_from(ack_capacity).unwrap_or(0),
            )
            .is_ok();
        let side = AudioSide {
            engine,
            events: events_rx,
            controls: controls_rx,
            acks: acks_tx,
            garbage: garbage_tx,
            cells: cells.clone(),
            clock: clock.clone(),
            sigs: Arc::clone(&sigs),
            taps: Arc::clone(&taps),
            scratch: vec![0.0; 2 * MAX_BLOCK].into_boxed_slice(),
            input_scratch: vec![0.0; 2 * MAX_BLOCK].into_boxed_slice(),
        };
        let mut host = NativeAudioHost {
            song_capacity: SongCapacityCache::new(controls.capacity(), ack_capacity, 2, configured),
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
            input_stream: None,
            capture_counters: None,
            capture_reported: CaptureStats::default(),
            taps,
            bus_names: None,
        };
        host.request_song_capacity();
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

    /// The bus names bus taps and captures resolve through (the session's
    /// instrument registry); without them only `:master` is tapped.
    pub fn set_bus_names(&mut self, names: Rc<dyn InstResolver>) {
        self.bus_names = Some(names);
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

    /// Input callback counters. `None` when capture is disabled.
    #[must_use]
    pub fn capture_stats(&self) -> Option<CaptureStats> {
        self.capture_counters.as_ref().map(|c| c.snapshot())
    }

    #[cfg(test)]
    pub(crate) fn attach_capture_counters(&mut self, counters: Arc<CaptureCounters>) {
        self.capture_counters = Some(counters);
    }

    /// Host diagnostics (`ring-overflow`, `graph-too-large`) since the last
    /// call.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        if let Some(now) = self.capture_stats() {
            let before = self.capture_reported;
            for (label, count) in [
                ("underrun", now.underrun.saturating_sub(before.underrun)),
                ("overrun", now.overrun.saturating_sub(before.overrun)),
                ("nonfinite", now.nonfinite.saturating_sub(before.nonfinite)),
                ("short frame", now.short.saturating_sub(before.short)),
                ("stale frame", now.stale.saturating_sub(before.stale)),
            ] {
                if count != 0 {
                    self.diags.push(Diagnostic {
                        span: Span::new(FileId::CONSOLE, 0, 0),
                        severity: Severity::Warning,
                        code: DiagCode::RingOverflow,
                        message: format!(
                            "audio input {label}: {count} frame samples since last report"
                        ),
                        origin: None,
                    });
                }
            }
            self.capture_reported = now;
        }
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
        if self.song_capacity.post(&mut self.controls, rec).is_err() {
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
    fn song_clock(&self) -> Result<crate::song::routing::SongHostClock, crate::vm::fail::Failure> {
        if !self.env.sr.is_finite()
            || self.env.sr.fract() != 0.0
            || !(8000.0..=192000.0).contains(&self.env.sr)
            || f64::from(self.env.sr) != f64::from(self.clock.sample_rate())
        {
            return Err(crate::vm::fail::Failure::new(
                crate::vm::fail::FailCode::HostUnavailable,
                "song clock requires an exact supported integral sample rate",
            ));
        }
        Ok(crate::song::routing::SongHostClock {
            frame: self.clock.frames(),
            sample_rate: self.clock.sample_rate(),
        })
    }

    #[allow(clippy::result_large_err)] // Exact inline command refusal preserves caller ownership.
    fn try_song_command(
        &mut self,
        command: crate::song::routing::SongCommand,
    ) -> Result<(), crate::host::caps::SongCommandRefusal> {
        if matches!(command, crate::song::routing::SongCommand::RequestClock(_)) {
            use crate::host::caps::{SongCommandRefusal, SongSubmitError};
            let mut bytes = [0; CtlMsg::MAX_LEN];
            if CtlMsg::Song(command).encode(&mut bytes) == 0 {
                return Err(SongCommandRefusal {
                    command,
                    error: SongSubmitError::Invalid(crate::vm::fail::Failure::new(
                        crate::vm::fail::FailCode::Type,
                        "invalid song clock command",
                    )),
                });
            }
            return self
                .controls
                .push(NativeRecord::Msg(CtlMsg::Song(command)))
                .map_err(|_| SongCommandRefusal {
                    command,
                    error: SongSubmitError::Backpressure,
                });
        }
        self.try_song_command_checked(command)
    }

    fn submit_song_native(
        &mut self,
        install: crate::dsp::ring::NativeSongInstall,
    ) -> Result<(), crate::dsp::ring::NativeSongInstall> {
        self.song_native(install)
    }
    fn materialize_song_native(
        &self,
        lease: crate::song::routing::SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<Option<crate::dsp::ring::NativeSongInstall>, crate::vm::fail::Failure> {
        self.materialize_song_graph(lease, graph).map(Some)
    }
    fn submit_song_graph(
        &mut self,
        lease: crate::song::routing::SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<(), crate::vm::fail::Failure> {
        self.song_graph(lease, graph)
    }
    fn try_song_sample(
        &mut self,
        lease: crate::song::routing::SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), crate::host::caps::SongSampleRefusal> {
        self.song_sample_checked(lease, data)
    }
    fn song_sample_sender_capacity(
        &self,
    ) -> Result<crate::host::caps::SongSampleSenderCapacity, crate::vm::fail::Failure> {
        Ok(crate::host::caps::SongSampleSenderCapacity::Unbounded)
    }
    fn submit_song_sample(
        &mut self,
        lease: crate::song::routing::SongLeaseKey,
        data: Arc<SampleData>,
    ) -> Result<(), Arc<SampleData>> {
        self.song_sample(lease, data)
    }

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

    fn poll_msg(&mut self) -> Result<Option<HostMsg>, Failure> {
        self.collect_garbage();
        self.song_capacity.retry(&mut self.controls);
        if let Some(message) = self.acks.pop() {
            let message = self.song_capacity.observe(message);
            self.song_capacity.retry(&mut self.controls);
            return Ok(message);
        }
        if self.dropped != self.reported {
            let message = HostMsg::Counters {
                late: 0,
                dropped: self.dropped - self.reported,
                stolen: 0,
                skipped: 0,
            };
            self.reported = self.dropped;
            return Ok(Some(message));
        }
        Ok(None)
    }

    fn drain(&mut self, out: &mut Vec<HostMsg>) {
        self.song_capacity.retry(&mut self.controls);
        while let Some(m) = self.acks.pop() {
            if let Some(message) = self.song_capacity.observe(m) {
                out.push(message);
            }
        }
        self.song_capacity.retry(&mut self.controls);
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

    fn tap_reader(&mut self) -> Option<Box<dyn TapReader>> {
        Some(Box::new(NativeTapReader::new(
            Arc::clone(&self.taps),
            self.bus_names.clone(),
        )))
    }

    fn arm_capture(
        &mut self,
        src: &TapSrc,
        start: f64,
        frames: usize,
    ) -> Result<CaptureId, Failure> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start = (start.max(0.0) * f64::from(self.clock.sample_rate())).round() as u64;
        tap::arm_capture(&self.taps, self.bus_names.as_ref(), src, start, frames)
    }

    fn poll_capture(&mut self, id: CaptureId, out: &mut Vec<f32>) -> CapturePoll {
        tap::poll_capture(&self.taps, id, out)
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

/// A f32 input configuration at the already chosen output rate. Prefer
/// stereo, then mono; wider devices use their first two channels.
fn input_f32_config(device: &cpal::Device, rate: u32) -> Result<SupportedStreamConfig, Diagnostic> {
    let ranges = device.supported_input_configs().map_err(|e| {
        unavailable(format!(
            "audio input was requested but its formats are unavailable ({e})"
        ))
    })?;
    ranges
        .filter(|r| {
            r.sample_format() == SampleFormat::F32
                && capture_channel_rank(r.channels()).is_some()
                && r.min_sample_rate().0 <= rate
                && rate <= r.max_sample_rate().0
        })
        .min_by_key(|r| capture_channel_rank(r.channels()))
        .map(|r| r.with_sample_rate(cpal::SampleRate(rate)))
        .ok_or_else(|| {
            unavailable(format!(
                "audio input was requested but no f32 input supports the output rate {rate} Hz"
            ))
        })
}

pub(crate) const fn capture_channel_rank(channels: u16) -> Option<u8> {
    match channels {
        0 => None,
        2 => Some(0),
        1 => Some(1),
        _ => Some(2),
    }
}
