//! Lock-free correlation between rendered frames and the host output clock.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cpal::OutputCallbackInfo;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LatencyKind {
    Measured,
    Estimate,
    Unavailable,
}

impl LatencyKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Estimate => "estimate",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockReading {
    pub processing_time: f64,
    pub latency_seconds: Option<f64>,
    pub latency_kind: LatencyKind,
    pub uncertainty_seconds: Option<f64>,
}

struct Shared {
    sequence: AtomicU64,
    frames: AtomicU64,
    callback_nanos: AtomicU64,
    playback_nanos: AtomicU64,
    buffer_frames: AtomicU64,
    running: AtomicBool,
}

/// Audio output clock whose callback writer only performs atomic stores.
#[derive(Clone)]
pub struct OutputClock {
    shared: Arc<Shared>,
    origin: Instant,
    sample_rate: u32,
    cached: Arc<Mutex<Option<ClockReading>>>,
}

impl OutputClock {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            shared: Arc::new(Shared {
                sequence: AtomicU64::new(0),
                frames: AtomicU64::new(0),
                callback_nanos: AtomicU64::new(0),
                playback_nanos: AtomicU64::new(u64::MAX),
                buffer_frames: AtomicU64::new(0),
                running: AtomicBool::new(false),
            }),
            origin: Instant::now(),
            sample_rate: sample_rate.max(1),
            cached: Arc::new(Mutex::new(None)),
        }
    }

    #[cfg(test)]
    pub(crate) fn backdated(sample_rate: u32, by: std::time::Duration) -> Self {
        let mut clock = Self::new(sample_rate);
        clock.origin = Instant::now().checked_sub(by).unwrap_or_else(Instant::now);
        clock
    }

    /// Publish one output callback's timestamp without locking or allocating.
    pub fn record(&self, info: &OutputCallbackInfo, frames_before: u64, buffer_frames: u32) {
        let timestamp = info.timestamp();
        let callback_nanos = Instant::now()
            .checked_duration_since(self.origin)
            .map_or(0, |d| d.as_nanos());
        let callback_nanos = u64::try_from(callback_nanos).unwrap_or(u64::MAX);
        let playback_nanos = timestamp
            .playback
            .duration_since(&timestamp.callback)
            .and_then(|d| u64::try_from(d.as_nanos()).ok())
            .unwrap_or(u64::MAX);
        self.record_values(
            frames_before,
            callback_nanos,
            playback_nanos,
            u64::from(buffer_frames),
        );
        self.set_running(true);
    }

    fn record_values(&self, frames: u64, callback: u64, playback: u64, buffer: u64) {
        let sequence = self.shared.sequence.load(Ordering::Relaxed);
        self.shared
            .sequence
            .store(sequence.wrapping_add(1), Ordering::Release);
        self.shared.frames.store(frames, Ordering::Release);
        self.shared
            .callback_nanos
            .store(callback, Ordering::Release);
        self.shared
            .playback_nanos
            .store(playback, Ordering::Release);
        self.shared.buffer_frames.store(buffer, Ordering::Release);
        self.shared
            .sequence
            .store(sequence.wrapping_add(2), Ordering::Release);
    }

    /// Read a stable host-clock sample, retrying at most four times.
    #[must_use]
    pub fn read(&self) -> ClockReading {
        let now = self.origin.elapsed().as_nanos();
        let now = u64::try_from(now).unwrap_or(u64::MAX);
        let mut sample = None;
        for _ in 0..4 {
            let before = self.shared.sequence.load(Ordering::Acquire);
            if before & 1 != 0 {
                continue;
            }
            let frames = self.shared.frames.load(Ordering::Acquire);
            let callback = self.shared.callback_nanos.load(Ordering::Acquire);
            let playback = self.shared.playback_nanos.load(Ordering::Acquire);
            let buffer = self.shared.buffer_frames.load(Ordering::Acquire);
            let after = self.shared.sequence.load(Ordering::Acquire);
            if before == after {
                sample = Some((frames, callback, playback, buffer));
                break;
            }
        }
        let Some((frames, callback, playback, buffer)) = sample else {
            return self.cached_reading().unwrap_or(ClockReading {
                processing_time: 0.0,
                latency_seconds: None,
                latency_kind: LatencyKind::Unavailable,
                uncertainty_seconds: None,
            });
        };
        let elapsed = now.saturating_sub(callback);
        let buffer_seconds = buffer as f64 / f64::from(self.sample_rate);
        let age_seconds = elapsed as f64 / 1_000_000_000.0;
        let mut reading =
            if self.shared.running.load(Ordering::Acquire) && callback != 0 && age_seconds <= 1.0 {
                let frame_time = frames as f64 / f64::from(self.sample_rate);
                let latency = (playback != u64::MAX).then_some(playback as f64 / 1_000_000_000.0);
                let kind = if latency.is_some() {
                    LatencyKind::Measured
                } else {
                    LatencyKind::Estimate
                };
                ClockReading {
                    processing_time: frame_time + age_seconds.min(buffer_seconds),
                    latency_seconds: latency.or(Some(buffer_seconds)),
                    latency_kind: kind,
                    uncertainty_seconds: Some(buffer_seconds),
                }
            } else {
                ClockReading {
                    processing_time: frames as f64 / f64::from(self.sample_rate),
                    latency_seconds: None,
                    latency_kind: LatencyKind::Unavailable,
                    uncertainty_seconds: None,
                }
            };
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(previous) = *cached {
            if reading.processing_time < previous.processing_time {
                reading.processing_time = previous.processing_time;
            }
        }
        *cached = Some(reading);
        reading
    }

    fn cached_reading(&self) -> Option<ClockReading> {
        *self
            .cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_running(&self, running: bool) {
        self.shared.running.store(running, Ordering::Release);
    }

    /// Mark the current sample invalid after an output-route change.
    pub fn invalidate(&self) {
        self.set_running(false);
    }

    #[cfg(test)]
    pub(crate) fn record_raw(&self, frames: u64, instant_nanos: u64, playback_nanos: Option<u64>) {
        self.record_values(
            frames,
            instant_nanos,
            playback_nanos.unwrap_or(u64::MAX),
            512,
        );
    }

    #[cfg(test)]
    pub(crate) fn now_nanos(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}
