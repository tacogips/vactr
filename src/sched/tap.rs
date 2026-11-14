//! Captures and live taps on the runtime side (design 14.5.9).
//!
//! A `capture :src cycles` effect is armed on the audio host in `drain`:
//! the capture starts at the next cycle boundary (S1) and lasts `cycles`
//! cycles at the current tempo, bounded by `max_capture_seconds`. Every
//! `tick` polls the armed captures and moves the streamed frames into the
//! buffer, which becomes `Ready` (or `Failed`, with the call's origin)
//! when the host reports the capture done.

use std::rc::Rc;

use crate::dsp::caps::Cap;
use crate::host::caps::{CaptureId, CapturePoll, TapReader, TapSrc};
use crate::reader::span::Span;
use crate::sched::runtime::Runtime;
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::vm::fail::{FailCode, Failure};

/// An armed capture waiting for its frames.
#[derive(Debug)]
pub(crate) struct PendingCapture {
    id: CaptureId,
    buf: Rc<SampleBuf>,
    rate: u32,
    frames: Vec<f32>,
    origin: Span,
}

impl Runtime {
    /// The audio host's live tap reader (`None` when it has no taps), for
    /// the session's `AnalysisCx`.
    pub fn tap_reader(&mut self) -> Option<Box<dyn TapReader>> {
        self.hosts.audio.tap_reader()
    }

    /// Captures armed and not yet complete.
    #[must_use]
    pub fn pending_captures(&self) -> usize {
        self.captures.len()
    }

    /// The `Capture` effect: arms it, or fails the buffer and reports why.
    pub(crate) fn capture(
        &mut self,
        buf: Rc<SampleBuf>,
        src: TapSrc,
        cycles: Ratio64,
        origin: Span,
        faults: &mut Vec<Failure>,
    ) {
        if let Err(mut f) = self.arm(&buf, src, cycles, origin) {
            f.origin.span = f.origin.span.or(Some(origin));
            buf.fail(f.code, &f.message);
            faults.push(f);
        }
    }

    fn arm(
        &mut self,
        buf: &Rc<SampleBuf>,
        src: TapSrc,
        cycles: Ratio64,
        origin: Span,
    ) -> Result<(), Failure> {
        let secs = cycles.to_f64() * self.clock.tempo().cycle_seconds()?;
        #[allow(clippy::cast_possible_truncation)]
        let want = Cap::CaptureSeconds(secs as f32);
        if let Err(d) = self.caps.require(want, Some(origin)) {
            return Err(Failure::new(FailCode::BeyondCapability, d.message));
        }
        let (_, pos) = self.now_pos();
        let boundary = Ratio64::from_int(pos.floor().saturating_add(1));
        let start = self.clock.to_host(boundary);
        let rate = self.cfg.sample_rate.max(1);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (secs * f64::from(rate)).round().max(1.0) as usize;
        let id = self.hosts.audio.arm_capture(&src, start, frames)?;
        self.captures.push(PendingCapture {
            id,
            buf: Rc::clone(buf),
            rate,
            frames: Vec::with_capacity(2 * frames),
            origin,
        });
        Ok(())
    }

    /// Polls every armed capture: a finished one fills its buffer, a failed
    /// one fails it and reports the failure with the call's origin.
    pub(crate) fn poll_captures(&mut self, faults: &mut Vec<Failure>) {
        let mut i = 0;
        while i < self.captures.len() {
            let c = &mut self.captures[i];
            match self.hosts.audio.poll_capture(c.id, &mut c.frames) {
                CapturePoll::Pending => i += 1,
                CapturePoll::Done => {
                    let c = self.captures.remove(i);
                    c.buf.fill_at(c.rate, c.frames);
                }
                CapturePoll::Failed(mut f) => {
                    let c = self.captures.remove(i);
                    f.origin.span = f.origin.span.or(Some(c.origin));
                    c.buf.fail(f.code, &f.message);
                    faults.push(f);
                }
            }
        }
    }
}
