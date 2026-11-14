//! Sample values: captured or rendered audio buffers (design 14.5.9).
//!
//! `Sound::Buffer` holds an `Rc<SampleBuf>`. A buffer is created `Pending`
//! by `capture`/`render` and filled (or failed) by the runtime when the
//! staged effect completes. Reading a `Pending` buffer (analysis or
//! playback) is `Failure(capture-pending)`; a `Failed` buffer returns its
//! own failure.

use std::cell::{Cell, Ref, RefCell};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use crate::host::caps::SampleData;
use crate::vm::fail::{FailCode, Failure};

thread_local! {
    static NEXT_BUF: Cell<u64> = const { Cell::new(1) };
}

fn next_buf_id() -> u64 {
    NEXT_BUF.with(|n| {
        let id = n.get();
        n.set(id.wrapping_add(1));
        id
    })
}

/// Where a buffer's data stands.
#[derive(Clone, Debug)]
pub enum BufState {
    /// A capture or render has not finished yet.
    Pending,
    /// Interleaved stereo frames (`[l0, r0, l1, r1, ..]`).
    Ready(Rc<[f32]>),
    /// The capture or render failed.
    Failed { code: FailCode, message: Rc<str> },
}

/// An audio buffer value. `id` is unique per session thread and is the
/// identity a buffer is installed under as a sample resource. The sample
/// rate of a capture is the audio host's, known only when it is filled.
pub struct SampleBuf {
    pub id: u64,
    rate: Cell<u32>,
    state: RefCell<BufState>,
}

impl SampleBuf {
    fn make(rate: u32, state: BufState) -> Rc<SampleBuf> {
        Rc::new(SampleBuf {
            id: next_buf_id(),
            rate: Cell::new(rate),
            state: RefCell::new(state),
        })
    }

    /// The sample rate, Hz (0 while a capture's rate is not known yet).
    #[must_use]
    pub fn rate(&self) -> u32 {
        self.rate.get()
    }

    /// A buffer still being captured or rendered.
    #[must_use]
    pub fn pending(rate: u32) -> Rc<SampleBuf> {
        SampleBuf::make(rate, BufState::Pending)
    }

    /// A buffer with its interleaved stereo frames.
    #[must_use]
    pub fn ready(rate: u32, frames: impl Into<Rc<[f32]>>) -> Rc<SampleBuf> {
        SampleBuf::make(rate, BufState::Ready(frames.into()))
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> Ref<'_, BufState> {
        self.state.borrow()
    }

    /// Marks the buffer `Ready` with interleaved stereo frames.
    pub fn fill(&self, frames: impl Into<Rc<[f32]>>) {
        *self.state.borrow_mut() = BufState::Ready(frames.into());
    }

    /// Marks the buffer `Ready` at sample rate `rate`.
    pub fn fill_at(&self, rate: u32, frames: impl Into<Rc<[f32]>>) {
        self.rate.set(rate);
        self.fill(frames);
    }

    /// Marks the buffer `Failed`.
    pub fn fail(&self, code: FailCode, message: &str) {
        *self.state.borrow_mut() = BufState::Failed {
            code,
            message: Rc::from(message),
        };
    }

    /// The number of stereo frames when `Ready`.
    #[must_use]
    pub fn frames(&self) -> Option<usize> {
        match &*self.state.borrow() {
            BufState::Ready(data) => Some(data.len() / 2),
            _ => None,
        }
    }

    /// The interleaved stereo frames of a `Ready` buffer.
    ///
    /// # Errors
    /// `capture-pending` while `Pending`; a `Failed` buffer's own failure.
    pub fn ready_frames(&self) -> Result<Rc<[f32]>, Failure> {
        match &*self.state.borrow() {
            BufState::Ready(data) => Ok(Rc::clone(data)),
            BufState::Pending => Err(Failure::new(
                FailCode::CapturePending,
                "the sound buffer is still being captured or rendered",
            )),
            BufState::Failed { code, message } => Err(Failure::new(*code, message.to_string())),
        }
    }

    /// The mono frames `(l + r) / 2` of a `Ready` buffer.
    ///
    /// # Errors
    /// As `ready_frames`.
    pub fn mono_frames(&self) -> Result<Rc<[f32]>, Failure> {
        let data = self.ready_frames()?;
        Ok(data
            .chunks_exact(2)
            .map(|lr| 0.5 * (lr[0] + lr[1]))
            .collect())
    }

    /// The buffer as sample data, for installation on the audio side.
    ///
    /// # Errors
    /// As `ready_frames`.
    pub fn to_sample_data(&self) -> Result<Arc<SampleData>, Failure> {
        let data = self.ready_frames()?;
        Ok(Arc::new(SampleData {
            rate: self.rate().max(1),
            channels: 2,
            frames: data.iter().copied().collect(),
        }))
    }
}

/// Buffers compare by identity.
impl PartialEq for SampleBuf {
    fn eq(&self, other: &SampleBuf) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for SampleBuf {}

impl fmt::Debug for SampleBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match &*self.state.borrow() {
            BufState::Pending => "pending".to_owned(),
            BufState::Ready(data) => format!("{} frames", data.len() / 2),
            BufState::Failed { code, .. } => format!("failed {code}"),
        };
        write!(f, "SampleBuf(#{} {} Hz {state})", self.id, self.rate())
    }
}
