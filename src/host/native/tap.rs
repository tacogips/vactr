//! Live taps and captures of the native audio host (design 14.5.9).
//!
//! `TapShared` is allocated once with the host and shared by the audio side
//! and the evaluator side:
//!
//! - one history ring of `TAP_FRAMES` mono frames per source: `:master`,
//!   and bus ids `1..=MAX_TAP_BUSES` (ring `k` holds bus `k + 1`, the ids
//!   the instrument registry hands out in order). After every engine block
//!   `AudioSide::render` copies the master output and each live bus block
//!   into its ring (a bus with no live chain records silence). The copy is
//!   a run of relaxed atomic stores under a seqlock sequence counter: no
//!   allocation, no lock, no `Rc` (12.8.9);
//! - `MAX_CAPTURES` capture slots, each an SPSC ring of interleaved stereo
//!   frames. `arm_capture` fills a free slot's parameters and publishes it;
//!   the audio side streams the armed source into the ring from the start
//!   frame on; `poll_capture` drains it on the evaluator side.
//!
//! `NativeTapReader` reads a consistent snapshot of a history ring,
//! retrying while the sequence is odd or changed. Taps are copies: nothing
//! read here feeds back into the engine.

use std::rc::Rc;
use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crate::dsp::bus::BusGraph;
use crate::dsp::graph::BusId;
use crate::host::caps::{CaptureId, CapturePoll, InstResolver, TapReader, TapSrc};
use crate::vm::fail::{FailCode, Failure};

/// The history of one tap source, mono frames.
pub const TAP_FRAMES: usize = 8192;
/// Named buses with a tap (bus ids `1..=MAX_TAP_BUSES`).
pub const MAX_TAP_BUSES: usize = 16;
/// Captures that may run at once.
pub const MAX_CAPTURES: usize = 4;
/// One capture ring, floats (interleaved stereo; about 1.4 s at 48 kHz
/// between two evaluator polls).
pub const CAPTURE_RING: usize = 1 << 17;
/// Snapshot attempts before a reader gives up on a busy ring.
const READ_TRIES: u32 = 1024;

const IDLE: u32 = 0;
const ARMED: u32 = 1;
const DONE: u32 = 2;
const OVERRUN: u32 = 3;

fn host_unavailable(message: &str) -> Failure {
    Failure::new(FailCode::HostUnavailable, message)
}

/// One seqlocked history ring.
#[derive(Debug)]
struct History {
    seq: AtomicU64,
    /// Frames written so far (the next write position, unwrapped).
    head: AtomicU64,
    data: Box<[AtomicU32]>,
}

impl History {
    fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
            head: AtomicU64::new(0),
            data: (0..TAP_FRAMES).map(|_| AtomicU32::new(0)).collect(),
        }
    }

    /// Appends `n` mono frames produced by `frame(k)` (audio side).
    fn write(&self, n: usize, frame: impl Fn(usize) -> f32) {
        let s = self.seq.load(Ordering::Relaxed);
        self.seq.store(s.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        let head = self.head.load(Ordering::Relaxed);
        for k in 0..n {
            #[allow(clippy::cast_possible_truncation)]
            let at = (head.wrapping_add(k as u64) % TAP_FRAMES as u64) as usize;
            self.data[at].store(frame(k).to_bits(), Ordering::Relaxed);
        }
        self.head
            .store(head.wrapping_add(n as u64), Ordering::Relaxed);
        self.seq.store(s.wrapping_add(2), Ordering::Release);
    }

    /// The last `n` frames, oldest first (evaluator side).
    fn read(&self, n: usize, out: &mut Vec<f32>) -> Result<(), Failure> {
        let n = n.min(TAP_FRAMES);
        for _ in 0..READ_TRIES {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            out.clear();
            let head = self.head.load(Ordering::Relaxed);
            #[allow(clippy::cast_possible_truncation)]
            let have = head.min(TAP_FRAMES as u64) as usize;
            // Frames never written read as silence.
            out.extend(std::iter::repeat(0.0).take(n.saturating_sub(have)));
            let first = head.wrapping_sub(n.min(have) as u64);
            for k in 0..n.min(have) {
                #[allow(clippy::cast_possible_truncation)]
                let at = (first.wrapping_add(k as u64) % TAP_FRAMES as u64) as usize;
                out.push(f32::from_bits(self.data[at].load(Ordering::Relaxed)));
            }
            fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s1 {
                return Ok(());
            }
        }
        Err(host_unavailable("the audio tap is busy; try again"))
    }
}

/// One capture: its parameters and an SPSC ring of stereo floats.
#[derive(Debug)]
struct Capture {
    state: AtomicU32,
    /// 0 = `:master`, `k + 1` = bus ring `k`.
    src: AtomicU32,
    start: AtomicU64,
    frames: AtomicU64,
    /// Frames streamed so far (audio side).
    taken: AtomicU64,
    /// Float positions, unwrapped: the audio side writes, the evaluator
    /// reads.
    write: AtomicU64,
    read: AtomicU64,
    ring: Box<[AtomicU32]>,
}

impl Capture {
    fn new() -> Self {
        Self {
            state: AtomicU32::new(IDLE),
            src: AtomicU32::new(0),
            start: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            taken: AtomicU64::new(0),
            write: AtomicU64::new(0),
            read: AtomicU64::new(0),
            ring: (0..CAPTURE_RING).map(|_| AtomicU32::new(0)).collect(),
        }
    }

    /// Streams the part of block `[at, at + n)` inside the capture window
    /// (audio side).
    fn record(&self, at: u64, n: usize, lr: impl Fn(usize) -> (f32, f32)) {
        if self.state.load(Ordering::Acquire) != ARMED {
            return;
        }
        let (start, total) = (
            self.start.load(Ordering::Relaxed),
            self.frames.load(Ordering::Relaxed),
        );
        let taken = self.taken.load(Ordering::Relaxed);
        let end = at.wrapping_add(n as u64);
        let from = if taken == 0 { start.max(at) } else { at };
        if from >= end || taken >= total {
            return;
        }
        let count = (end - from).min(total - taken);
        let w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        if w.wrapping_sub(r) + 2 * count > CAPTURE_RING as u64 {
            self.state.store(OVERRUN, Ordering::Release);
            return;
        }
        #[allow(clippy::cast_possible_truncation)]
        let offset = (from - at) as usize;
        #[allow(clippy::cast_possible_truncation)]
        for k in 0..count as usize {
            let (l, rr) = lr(offset + k);
            let base = w.wrapping_add(2 * k as u64);
            self.ring[(base % CAPTURE_RING as u64) as usize].store(l.to_bits(), Ordering::Relaxed);
            self.ring[((base + 1) % CAPTURE_RING as u64) as usize]
                .store(rr.to_bits(), Ordering::Relaxed);
        }
        self.write
            .store(w.wrapping_add(2 * count), Ordering::Release);
        self.taken.store(taken + count, Ordering::Relaxed);
        if taken + count >= total {
            self.state.store(DONE, Ordering::Release);
        }
    }

    /// Moves the streamed floats into `out` (evaluator side).
    fn drain(&self, out: &mut Vec<f32>) {
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        let mut p = r;
        while p != w {
            #[allow(clippy::cast_possible_truncation)]
            let at = (p % CAPTURE_RING as u64) as usize;
            out.push(f32::from_bits(self.ring[at].load(Ordering::Relaxed)));
            p = p.wrapping_add(1);
        }
        self.read.store(w, Ordering::Release);
    }
}

/// The taps and captures shared by the audio and evaluator sides.
#[derive(Debug)]
pub struct TapShared {
    master: History,
    buses: Box<[History]>,
    captures: Box<[Capture]>,
}

impl Default for TapShared {
    fn default() -> Self {
        Self::new()
    }
}

impl TapShared {
    /// Preallocates every ring (evaluator side, at host construction).
    #[must_use]
    pub fn new() -> Self {
        Self {
            master: History::new(),
            buses: (0..MAX_TAP_BUSES).map(|_| History::new()).collect(),
            captures: (0..MAX_CAPTURES).map(|_| Capture::new()).collect(),
        }
    }

    /// Records one rendered block (audio side): `out` holds the `n`
    /// interleaved master frames that start at frame `at`; `buses` holds
    /// every bus's block. Allocation-free and lock-free.
    pub fn record(&self, out: &[f32], n: usize, at: u64, buses: &BusGraph) {
        let n = n.min(out.len() / 2);
        let master = |k: usize| (out[2 * k], out[2 * k + 1]);
        self.master.write(n, |k| {
            let (l, r) = master(k);
            0.5 * (l + r)
        });
        for (k, ring) in self.buses.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let bus = BusId::new(k as u32 + 1);
            match buses.find(bus) {
                Some(i) => {
                    let (l, r) = buses.frames(i, n);
                    let m = l.len().min(r.len());
                    ring.write(n, |j| if j < m { 0.5 * (l[j] + r[j]) } else { 0.0 });
                }
                None => ring.write(n, |_| 0.0),
            }
        }
        for c in self.captures.iter() {
            if c.state.load(Ordering::Relaxed) != ARMED {
                continue;
            }
            match c.src.load(Ordering::Relaxed) {
                0 => c.record(at, n, master),
                s => {
                    let (l, r) = buses
                        .find(BusId::new(s))
                        .map_or((&[][..], &[][..]), |i| buses.frames(i, n));
                    c.record(at, n, |j| {
                        (
                            l.get(j).copied().unwrap_or(0.0),
                            r.get(j).copied().unwrap_or(0.0),
                        )
                    });
                }
            }
        }
    }

    /// Arms a capture of `frames` frames of source `src` (0 = master, else
    /// a bus id) from frame `start` (evaluator side).
    fn arm(&self, src: u32, start: u64, frames: u64) -> Result<CaptureId, Failure> {
        let Some(k) = self
            .captures
            .iter()
            .position(|c| c.state.load(Ordering::Acquire) == IDLE)
        else {
            return Err(Failure::new(
                FailCode::BeyondCapability,
                format!("more than {MAX_CAPTURES} captures at once are not available on this host"),
            ));
        };
        let c = &self.captures[k];
        c.src.store(src, Ordering::Relaxed);
        c.start.store(start, Ordering::Relaxed);
        c.frames.store(frames, Ordering::Relaxed);
        c.taken.store(0, Ordering::Relaxed);
        c.write.store(0, Ordering::Relaxed);
        c.read.store(0, Ordering::Relaxed);
        c.state.store(ARMED, Ordering::Release);
        Ok(CaptureId::new(u32::try_from(k).unwrap_or(0)))
    }

    /// Drains capture `id` into `out` and reports where it stands
    /// (evaluator side); a finished or failed capture frees its slot.
    fn poll(&self, id: CaptureId, out: &mut Vec<f32>) -> CapturePoll {
        let Some(c) = usize::try_from(id.get())
            .ok()
            .and_then(|k| self.captures.get(k))
        else {
            return CapturePoll::Failed(host_unavailable("no such capture"));
        };
        let state = c.state.load(Ordering::Acquire);
        c.drain(out);
        match state {
            DONE => {
                c.state.store(IDLE, Ordering::Release);
                CapturePoll::Done
            }
            OVERRUN => {
                c.state.store(IDLE, Ordering::Release);
                CapturePoll::Failed(host_unavailable(
                    "the capture overran its ring (the evaluator fell behind)",
                ))
            }
            ARMED => CapturePoll::Pending,
            _ => CapturePoll::Failed(host_unavailable("no such capture")),
        }
    }

    fn history(&self, src: u32) -> Option<&History> {
        match src {
            0 => Some(&self.master),
            s => self.buses.get(usize::try_from(s).ok()?.checked_sub(1)?),
        }
    }
}

/// The ring index of a source: 0 for `:master`, the bus id for a named bus
/// that has a tap.
fn source_index(src: &TapSrc, names: Option<&Rc<dyn InstResolver>>) -> Result<u32, Failure> {
    match src {
        TapSrc::Master => Ok(0),
        TapSrc::Bus(kw) => {
            let names =
                names.ok_or_else(|| host_unavailable("bus taps need the host's bus names"))?;
            let id = names.bus(*kw).ok_or_else(|| {
                Failure::new(FailCode::Type, "the tap source is not a defined bus")
            })?;
            if (1..=MAX_TAP_BUSES).contains(&(id.get() as usize)) {
                Ok(id.get())
            } else {
                Err(Failure::new(
                    FailCode::BeyondCapability,
                    format!(
                        "taps of more than {MAX_TAP_BUSES} buses are not available on this host"
                    ),
                ))
            }
        }
    }
}

/// Reads snapshots of the native tap rings.
pub struct NativeTapReader {
    shared: Arc<TapShared>,
    names: Option<Rc<dyn InstResolver>>,
}

impl NativeTapReader {
    /// A reader over `shared`; `names` resolves bus keywords to bus ids.
    #[must_use]
    pub fn new(shared: Arc<TapShared>, names: Option<Rc<dyn InstResolver>>) -> Self {
        Self { shared, names }
    }
}

impl TapReader for NativeTapReader {
    fn snapshot(&mut self, src: &TapSrc, frames: usize, out: &mut Vec<f32>) -> Result<(), Failure> {
        let k = source_index(src, self.names.as_ref())?;
        let ring = self
            .shared
            .history(k)
            .ok_or_else(|| host_unavailable("no such tap"))?;
        ring.read(frames, out)
    }
}

/// The evaluator side of the taps: arming and polling captures for
/// `NativeAudioHost`.
pub(crate) fn arm_capture(
    shared: &TapShared,
    names: Option<&Rc<dyn InstResolver>>,
    src: &TapSrc,
    start_frame: u64,
    frames: usize,
) -> Result<CaptureId, Failure> {
    let k = source_index(src, names)?;
    shared.arm(k, start_frame, frames as u64)
}

pub(crate) fn poll_capture(shared: &TapShared, id: CaptureId, out: &mut Vec<f32>) -> CapturePoll {
    shared.poll(id, out)
}
