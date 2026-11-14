//! Offline render (design 14.5.9): `render cycles` renders the current
//! bindings into a sample buffer with no audio device.
//!
//! `Runtime::drain` handles the staged `Render` effect, where the evaluator
//! is free. The render builds a headless `NativeAudioHost` and a second
//! `Runtime` over the live resolver and caps, installs the live instrument
//! and bus graphs and the ACTIVE slot bindings, and ticks a virtual clock
//! from cycle 0 at the live tempo through the 11.3 steps (1)-(5) only: no
//! `at`/`once` thunk runs, so nothing leaks into the live session. The live
//! runtime, its clock and its hosts are untouched (its sample loader is
//! lent to the render and handed back). The frame count is bounded by
//! `max_capture_seconds`; the work is synchronous on the evaluator thread,
//! outside any audio callback.
//!
//! Native only (`host-native`, not wasm32): elsewhere the effect fails the
//! buffer with `beyond-capability`, which the `render` native already
//! reports at the call when the caps say so.

use std::rc::Rc;

use crate::dsp::offline::OFFLINE_RATE;
use crate::ns::evaluator::Evaluator;
use crate::reader::span::Span;
use crate::sched::runtime::Runtime;
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::vm::fail::Failure;

impl Runtime {
    /// The `Render` effect: renders into `buf`, or fails it and reports why.
    pub(crate) fn render_effect(
        &mut self,
        ev: &mut Evaluator,
        buf: &Rc<SampleBuf>,
        cycles: Ratio64,
        origin: Span,
        faults: &mut Vec<Failure>,
    ) {
        match render(ev, self, cycles) {
            Ok(frames) => buf.fill_at(OFFLINE_RATE, frames),
            Err(mut f) => {
                f.origin.span = f.origin.span.or(Some(origin));
                buf.fail(f.code, &f.message);
                faults.push(f);
            }
        }
    }
}

/// Renders `cycles` cycles of `live`'s active bindings; interleaved stereo
/// at `OFFLINE_RATE`.
///
/// # Errors
/// `beyond-capability` when the length exceeds `max_capture_seconds` (or
/// on a build without the native host); a tempo failure.
#[cfg(not(all(feature = "host-native", not(target_arch = "wasm32"))))]
pub fn render(_: &mut Evaluator, _: &mut Runtime, _: Ratio64) -> Result<Vec<f32>, Failure> {
    Err(Failure::new(
        crate::vm::fail::FailCode::BeyondCapability,
        "offline render is not available on this host",
    ))
}

/// Renders `cycles` cycles of `live`'s active bindings; interleaved stereo
/// at `OFFLINE_RATE`.
///
/// # Errors
/// `beyond-capability` when the length exceeds `max_capture_seconds`; a
/// tempo failure.
#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
pub fn render(
    ev: &mut Evaluator,
    live: &mut Runtime,
    cycles: Ratio64,
) -> Result<Vec<f32>, Failure> {
    native::render(ev, live, cycles)
}

#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
mod native {
    use std::rc::Rc;

    use crate::clock::clock::{Clock, ClockSource};
    use crate::dsp::caps::Cap;
    use crate::dsp::cells::CellRead;
    use crate::dsp::offline::OFFLINE_RATE;
    use crate::host::caps::{GraphHandle, Hosts};
    use crate::host::native::audio::{AudioSide, MAX_BLOCK};
    use crate::host::native::NativeAudioHost;
    use crate::host::noop::NoopHost;
    use crate::host::wire::Ctl;
    use crate::ns::evaluator::Evaluator;
    use crate::ns::stage::StagedEffect;
    use crate::sched::cells::Tier;
    use crate::sched::runtime::{DrainReport, Runtime, RuntimeConfig};
    use crate::sched::slots::Binding;
    use crate::value::ratio::Ratio64;
    use crate::vm::fail::{FailCode, Failure};

    pub(super) fn render(
        ev: &mut Evaluator,
        live: &mut Runtime,
        cycles: Ratio64,
    ) -> Result<Vec<f32>, Failure> {
        let tempo = live.clock.tempo();
        let secs = cycles.to_f64() * tempo.cycle_seconds()?;
        #[allow(clippy::cast_possible_truncation)]
        let want = Cap::CaptureSeconds(secs as f32);
        if let Err(d) = live.caps.require(want, None) {
            return Err(Failure::new(FailCode::BeyondCapability, d.message));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (secs * f64::from(OFFLINE_RATE)).round() as usize;
        let clock = Clock::new(ClockSource::Internal, tempo, 0.0)?;
        let (host, mut side) =
            NativeAudioHost::headless(OFFLINE_RATE, live.caps, live.cfg.cell_pool);
        let cfg = RuntimeConfig {
            lookahead: live.cfg.lookahead,
            commit_lead: live.cfg.commit_lead,
            cell_pool: live.cfg.cell_pool,
            tier: Tier::Native(host.cells()),
            seed: live.cfg.seed,
            sample_rate: OFFLINE_RATE,
            ..RuntimeConfig::default()
        };
        let hosts = Hosts {
            audio: Box::new(host),
            midi: Box::new(NoopHost),
            osc: Box::new(NoopHost),
            render: Box::new(NoopHost),
            midi_in: Box::new(NoopHost),
            // Lent for the render, handed back below.
            samples: std::mem::replace(&mut live.hosts.samples, Box::new(NoopHost)),
        };
        let (mut off, _sink) = Runtime::new(hosts, Rc::clone(&live.resolver), live.caps, cfg);
        off.run_thunks = false;
        off.clock = clock;
        install(ev, live, &mut off);
        let out = run(ev, &mut off, &mut side, frames);
        live.hosts.samples = std::mem::replace(&mut off.hosts.samples, Box::new(NoopHost));
        Ok(out)
    }

    /// The live graphs, cell values, samples and active bindings.
    fn install(ev: &mut Evaluator, live: &Runtime, off: &mut Runtime) {
        let graphs = ev.insts().map(|r| r.borrow().graphs()).unwrap_or_default();
        for g in graphs {
            let cells: Vec<_> = match &g {
                GraphHandle::Inst { def, .. } => def
                    .params
                    .iter()
                    .filter_map(|(_, c)| match c {
                        Ctl::Cell(cell) => Some(*cell),
                        Ctl::Const(_) => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let mut dr = DrainReport::default();
            off.apply(ev, StagedEffect::Install(g), &mut dr);
            // A default that is a tweak site plays its current value.
            if let Some(atomic) = live.cells.native() {
                for cell in cells {
                    off.cells.write_external(cell, atomic.get(cell));
                }
            }
        }
        live.samples.preload(&mut off.samples, &mut off.hosts);
        for slot in live.slots.iter().filter(|s| !s.ephemeral) {
            if let Some(b @ Binding::Pattern(_)) = &slot.bound {
                if off.rebind(slot.key, b.clone()).is_ok() {
                    if let Some(s) = off.slots.get_mut(slot.key) {
                        s.muted = slot.muted;
                    }
                }
            }
        }
    }

    /// Ticks and renders block by block from cycle 0.
    fn run(ev: &mut Evaluator, off: &mut Runtime, side: &mut AudioSide, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; 2 * frames];
        // Installs (graphs, samples) reach the engine before the first tick.
        side.render(&mut [], 2);
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(MAX_BLOCK);
            #[allow(clippy::cast_precision_loss)]
            let now = done as f64 / f64::from(OFFLINE_RATE);
            off.tick(ev, now);
            side.render(&mut out[2 * done..2 * (done + n)], 2);
            done += n;
        }
        out
    }
}
