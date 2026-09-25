//! The evaluator tick source (design 12.8.10): a timer thread that only
//! posts wakes.
//!
//! The thread sleeps `period` and sends `()` over a bounded channel of one.
//! A wake still pending when the next one is due is coalesced (the
//! evaluator reads `host.now()` on each wake, so a missed wake loses no
//! time). Dropping the `TickSource` stops the thread, wakes it and joins
//! it; the receiver then reports `Disconnected`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The default tick period (12.8.10).
pub const TICK_PERIOD: Duration = Duration::from_millis(5);

/// A running timer thread.
#[derive(Debug)]
pub struct TickSource {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TickSource {
    /// Starts the thread. If the OS refuses a thread, no wake ever
    /// arrives and the receiver is disconnected at once.
    #[must_use]
    pub fn start(period: Duration) -> (TickSource, Receiver<()>) {
        let (tx, rx) = sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("vactrol-tick".into())
            .spawn(move || {
                while !flag.load(Ordering::Acquire) {
                    thread::park_timeout(period);
                    if flag.load(Ordering::Acquire) {
                        break;
                    }
                    if let Err(TrySendError::Disconnected(())) = tx.try_send(()) {
                        break;
                    }
                }
            })
            .ok();
        (TickSource { stop, thread }, rx)
    }

    /// True while the thread runs.
    #[must_use]
    pub fn running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }
}

impl Drop for TickSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}
