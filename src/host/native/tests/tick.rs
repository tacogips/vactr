//! `TickSource`: wakes arrive, and dropping it stops and joins the thread.
//! Waits are bounded; no assertion depends on exact periods.

use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use crate::host::native::TickSource;

const BOUND: Duration = Duration::from_secs(5);

#[test]
fn wakes_arrive() {
    let (ticks, rx) = TickSource::start(Duration::from_millis(1));
    for _ in 0..3 {
        assert_eq!(rx.recv_timeout(BOUND), Ok(()));
    }
    assert!(ticks.running());
}

#[test]
fn drop_stops_and_joins_the_thread() {
    let (ticks, rx) = TickSource::start(Duration::from_millis(1));
    assert_eq!(rx.recv_timeout(BOUND), Ok(()));
    drop(ticks);
    // The thread has exited: at most one coalesced wake is left, then the
    // channel reports the sender gone.
    let _ = rx.try_recv();
    assert_eq!(rx.recv_timeout(BOUND), Err(RecvTimeoutError::Disconnected));
}

#[test]
fn a_long_period_still_joins_promptly() {
    let (ticks, _rx) = TickSource::start(Duration::from_secs(3600));
    let t = std::time::Instant::now();
    drop(ticks);
    assert!(t.elapsed() < BOUND, "drop unparks the sleeping thread");
}

#[test]
fn a_slow_reader_gets_coalesced_wakes() {
    let (ticks, rx) = TickSource::start(Duration::from_millis(1));
    assert_eq!(rx.recv_timeout(BOUND), Ok(()));
    std::thread::sleep(Duration::from_millis(30));
    drop(ticks);
    // Thirty periods passed unread: the channel holds one wake, never a
    // backlog.
    let pending = rx.try_iter().count();
    assert!(pending <= 1, "{pending} wakes queued");
}
