//! Headless end-to-end tests: a real `Evaluator` + `InstRegistry` over a
//! real `Runtime`, rendered through the native `NativeAudioHost` /
//! `AudioSide` pair with no device (design 12.8.12; BE-FINAL; TASK-008
//! criteria 2-4 and the beep proxy for criterion 10).
//!
//! `E2e` wires the same production path as `examples/beep.rs` and
//! `src/host/wasm/main_half.rs::main_init`: a `Runtime` is built first (so
//! its `RuntimeSink` exists), then the `Evaluator` over the same instrument
//! registry (its constructor realizes the seven prelude templates and
//! releases their `Install` effects to the sink), then `rt.drain(&mut ev)`
//! pushes those installs into the engine. Every `AudioSide::render` call
//! runs under `dsp::alloc_probe::armed`, asserting zero allocations (17
//! invariant 3).

mod beep;
mod buses;
mod live_input;
mod regions;
mod sched_gaps;
mod templates;

use std::cell::RefCell;
use std::rc::Rc;

use crate::dsp::alloc_probe::armed;
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{Hosts, InstResolver, SampleLoader};
use crate::host::native::{AudioSide, FrameClock, NativeAudioHost};
use crate::host::noop::NoopHost;
use crate::ns::evaluator::Evaluator;
use crate::ns::insts::InstRegistry;
use crate::ns::namespace::Prelude;
use crate::reader::span::FileId;
use crate::sched::cells::Tier;
use crate::sched::runtime::{Runtime, RuntimeConfig};
use crate::vm::fail::Failure;

/// The headless engine's sample rate.
pub(super) const SR: u32 = 48_000;
/// The control cell pool (matches `NativeConfig::default`).
const CELLS: usize = 1024;
/// One audio callback's block size (well under `MAX_BLOCK`).
const BLOCK: usize = 256;
/// The evaluated source's file id.
const SOURCE: FileId = FileId::new(1);

/// The headless rig: a real `Runtime` and `Evaluator` over one instrument
/// registry, rendered through a device-less `NativeAudioHost`/`AudioSide`.
pub(super) struct E2e {
    pub rt: Runtime,
    pub ev: Evaluator,
    pub side: AudioSide,
    pub clock: FrameClock,
    pub reg: Rc<RefCell<InstRegistry>>,
    /// Events committed to a host over every `run_for` call so far.
    pub committed: usize,
    /// Query and commit faults over every `run_for` call so far.
    pub faults: Vec<Failure>,
}

impl E2e {
    /// A rig with no sample loader (`NoopHost::load` fails every request).
    pub(super) fn new() -> Self {
        Self::with_loader(Box::new(NoopHost))
    }

    /// A rig over `loader` (a test double that installs deterministic
    /// sample data, for the sample-backed templates).
    pub(super) fn with_loader(loader: Box<dyn SampleLoader>) -> Self {
        let caps = CapabilitySet::native();
        let (host, side) = NativeAudioHost::headless(SR, caps, CELLS);
        // The clock and cells are cloned before the host is boxed into
        // `Hosts` (its owner from here on).
        let clock = host.clock();
        let cells = host.cells();
        let hosts = Hosts {
            audio: Box::new(host),
            samples: loader,
            ..Hosts::noop()
        };
        let reg = InstRegistry::shared();
        let resolver: Rc<dyn InstResolver> = Rc::new(Rc::clone(&reg));
        let rcfg = RuntimeConfig {
            cell_pool: cells.capacity(),
            tier: Tier::Native(cells),
            ..RuntimeConfig::default()
        };
        // The runtime first, so its `RuntimeSink` exists for the evaluator
        // (which realizes the prelude templates and releases their
        // `Install` effects into it), then one `drain` pushes them through.
        let (mut rt, sink) = Runtime::new(hosts, resolver, caps, rcfg);
        let mut ev = Evaluator::with_insts(
            Prelude::core(),
            Box::new(NoopHost),
            Box::new(sink),
            Rc::clone(&reg),
        );
        assert!(
            reg.borrow().template_errors().is_empty(),
            "prelude template errors: {:?}",
            reg.borrow().template_errors()
        );
        let rep = rt.drain(&mut ev);
        assert!(
            rep.faults.is_empty(),
            "template install faults: {:?}",
            rep.faults
        );
        Self {
            rt,
            ev,
            side,
            clock,
            reg,
            committed: 0,
            faults: Vec::new(),
        }
    }

    /// Evaluates `src` (every form must be `Ok`), then drains the runtime.
    pub(super) fn eval(&mut self, src: &str) {
        let outcomes = self
            .ev
            .eval_str(src, SOURCE)
            .unwrap_or_else(|d| panic!("`{src}` does not read/expand: {d}"));
        for o in &outcomes {
            if let Err(f) = &o.value {
                panic!("`{src}` failed: {} : {}", f.code, f.message);
            }
        }
        let rep = self.rt.drain(&mut self.ev);
        assert!(
            rep.faults.is_empty(),
            "`{src}` drain faults: {:?}",
            rep.faults
        );
    }

    /// Ticks and renders one `BLOCK`-frame stereo block at a time until
    /// `seconds` of audio has rendered, returning the left channel. Every
    /// render runs armed, asserting zero allocations; every tick's
    /// `committed`/`faults` accumulate onto the rig.
    pub(super) fn run_for(&mut self, seconds: f64) -> Vec<f32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames_total = (seconds * f64::from(SR)).round() as usize;
        let mut left = Vec::with_capacity(frames_total);
        let mut rendered = 0usize;
        while rendered < frames_total {
            let now = self.clock.now();
            let rep = self.rt.tick(&mut self.ev, now);
            self.committed += rep.committed;
            self.faults.extend(rep.faults);
            let mut buf = vec![0.0f32; BLOCK * 2];
            let (_, allocs) = armed(|| self.side.render(&mut buf, 2));
            assert_eq!(allocs, 0, "the audio callback allocated");
            left.extend(buf.chunks_exact(2).map(|f| f[0]));
            rendered += BLOCK;
        }
        left
    }

    /// Renders both output channels, retaining the callback allocation probe.
    pub(super) fn run_stereo_for(&mut self, seconds: f64) -> (Vec<f32>, Vec<f32>) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames_total = (seconds * f64::from(SR)).round() as usize;
        let mut left = Vec::with_capacity(frames_total);
        let mut right = Vec::with_capacity(frames_total);
        let mut rendered = 0usize;
        while rendered < frames_total {
            let now = self.clock.now();
            let rep = self.rt.tick(&mut self.ev, now);
            self.committed += rep.committed;
            self.faults.extend(rep.faults);
            let mut buf = vec![0.0f32; BLOCK * 2];
            let (_, allocs) = armed(|| self.side.render(&mut buf, 2));
            assert_eq!(allocs, 0, "the audio callback allocated");
            left.extend(buf.chunks_exact(2).map(|f| f[0]));
            right.extend(buf.chunks_exact(2).map(|f| f[1]));
            rendered += BLOCK;
        }
        (left, right)
    }
}

/// The root-mean-square of `samples` (loudness proxy).
#[allow(clippy::cast_precision_loss)]
pub(super) fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// True when every sample is finite.
pub(super) fn all_finite(samples: &[f32]) -> bool {
    samples.iter().all(|s| s.is_finite())
}

/// The largest absolute sample value (0.0 for an empty slice).
pub(super) fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |m, s| m.max(s.abs()))
}
