//! Bounded, lock-free bridge between independently clocked CPAL input and output callbacks.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::dsp::ring::{Consumer, Producer, SpscRing};

use super::audio::MAX_BLOCK;

/// About 85 ms at 96 kHz; output trims excess queued latency to four blocks.
const CAPACITY_FRAMES: usize = 8192;
const MAX_BACKLOG_FRAMES: usize = 4 * MAX_BLOCK;

#[derive(Default)]
pub struct CaptureCounters {
    pub underrun: AtomicU64,
    pub overrun: AtomicU64,
    pub nonfinite: AtomicU64,
    pub short: AtomicU64,
    pub stale: AtomicU64,
}

/// A snapshot safe to inspect off the callback threads.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CaptureStats {
    pub underrun: u64,
    pub overrun: u64,
    pub nonfinite: u64,
    pub short: u64,
    pub stale: u64,
}

impl CaptureCounters {
    pub fn snapshot(&self) -> CaptureStats {
        CaptureStats {
            underrun: self.underrun.load(Ordering::Relaxed),
            overrun: self.overrun.load(Ordering::Relaxed),
            nonfinite: self.nonfinite.load(Ordering::Relaxed),
            short: self.short.load(Ordering::Relaxed),
            stale: self.stale.load(Ordering::Relaxed),
        }
    }
}

pub struct InputProducer {
    frames: Producer<[f32; 2]>,
    counters: Arc<CaptureCounters>,
}

pub struct InputConsumer {
    frames: Consumer<[f32; 2]>,
    counters: Arc<CaptureCounters>,
}

pub fn pair() -> (InputProducer, InputConsumer, Arc<CaptureCounters>) {
    let (producer, consumer) = SpscRing::split(CAPACITY_FRAMES);
    let counters = Arc::new(CaptureCounters::default());
    (
        InputProducer {
            frames: producer,
            counters: Arc::clone(&counters),
        },
        InputConsumer {
            frames: consumer,
            counters: Arc::clone(&counters),
        },
        counters,
    )
}

impl InputProducer {
    /// CPAL callback body. Incomplete trailing frames are discarded.
    pub fn push_interleaved(&mut self, input: &[f32], channels: usize) {
        if channels == 0 {
            self.counters
                .short
                .fetch_add(input.len() as u64, Ordering::Relaxed);
            return;
        }
        let (frames, remainder) = (input.len() / channels, input.len() % channels);
        self.counters
            .short
            .fetch_add(remainder as u64, Ordering::Relaxed);
        for frame in input[..frames * channels].chunks_exact(channels) {
            let left = if frame[0].is_finite() {
                frame[0]
            } else {
                self.counters.nonfinite.fetch_add(1, Ordering::Relaxed);
                0.0
            };
            let right = if channels == 1 {
                left
            } else if frame[1].is_finite() {
                frame[1]
            } else {
                self.counters.nonfinite.fetch_add(1, Ordering::Relaxed);
                0.0
            };
            let lr = [left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0)];
            if self.frames.push(lr).is_err() {
                self.counters.overrun.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl InputConsumer {
    /// Fills exactly one output block, silencing missing input and trimming
    /// excess producer-clock backlog so old captured audio is not replayed.
    pub fn fill(&mut self, output: &mut [f32]) {
        debug_assert_eq!(output.len() % 2, 0);
        let excess = self.frames.len().saturating_sub(MAX_BACKLOG_FRAMES);
        for _ in 0..excess {
            if self.frames.pop().is_some() {
                self.counters.stale.fetch_add(1, Ordering::Relaxed);
            }
        }
        for pair in output.chunks_exact_mut(2) {
            if let Some(lr) = self.frames.pop() {
                pair.copy_from_slice(&lr);
            } else {
                pair.fill(0.0);
                self.counters.underrun.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
