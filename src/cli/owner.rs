//! Reusable native session owner loop for WebSocket and Tauri transports.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::cli::args::HostChoice;
use crate::cli::ws::ConnEvent;
use crate::cli::{build_session, HostClock, TICK_PERIOD};
use crate::session::codec::encode;
use crate::session::{Dest, Outgoing, Session};

/// Audio lifecycle notifications from the native shell.
pub enum AudioSessionEvent {
    Interrupted,
    Resumed,
    RouteChanged,
}

/// Events consumed by the session owner.
pub(crate) enum OwnerEvent {
    Conn(ConnEvent),
    Audio(AudioSessionEvent),
    Shutdown,
}

/// A native session and its transport channels, owned by one control thread.
pub struct SessionOwner {
    events: Sender<OwnerEvent>,
    next_id: AtomicU32,
    join: Option<JoinHandle<()>>,
}

impl SessionOwner {
    /// Build a session on its owner thread and return after initialization.
    pub fn spawn(host: HostChoice, cwd: PathBuf) -> Result<SessionOwner, String> {
        let (events, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = thread::spawn(move || match build_session(host, &cwd) {
            Ok((session, clock)) => {
                let _ = ready_tx.send(Ok(()));
                run_loop(session, clock, receiver, |_, _| {});
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        });
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(SessionOwner {
                events,
                next_id: AtomicU32::new(1),
                join: Some(join),
            }),
            Ok(Err(error)) => {
                let _ = join.join();
                Err(error)
            }
            Err(error) => {
                let _ = join.join();
                Err(format!("session owner did not initialize: {error}"))
            }
        }
    }

    /// Register an outbound channel and return its connection id.
    pub fn connect(&self, outbound: Sender<String>) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).max(1);
        let _ = self
            .events
            .send(OwnerEvent::Conn(ConnEvent::Connected { id, outbound }));
        id
    }

    /// Submit one client frame.
    pub fn send(&self, id: u32, text: String) {
        let _ = self
            .events
            .send(OwnerEvent::Conn(ConnEvent::Text { id, text }));
    }

    /// Disconnect one client.
    pub fn close(&self, id: u32) {
        let _ = self.events.send(OwnerEvent::Conn(ConnEvent::Closed { id }));
    }

    /// Notify the owner of an audio lifecycle event.
    pub fn audio_event(&self, event: AudioSessionEvent) {
        let _ = self.events.send(OwnerEvent::Audio(event));
    }

    /// Stop and join the control thread.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        let _ = self.events.send(OwnerEvent::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for SessionOwner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The common session loop used directly by `serve` and by `SessionOwner`.
pub(crate) fn run_loop(
    mut session: Session,
    clock: HostClock,
    rx: Receiver<OwnerEvent>,
    mut route: impl FnMut(&Session, Vec<Outgoing>),
) {
    let mut conns = BTreeMap::<u32, Sender<String>>::new();
    let mut last_tick = Instant::now();
    loop {
        let wait = TICK_PERIOD.saturating_sub(last_tick.elapsed());
        match rx.recv_timeout(wait) {
            Ok(OwnerEvent::Conn(ConnEvent::Connected { id, outbound })) => {
                conns.insert(id, outbound);
            }
            Ok(OwnerEvent::Conn(ConnEvent::Text { id, text })) => {
                session.observe_clock(clock_reading(&clock));
                let outgoing = session.apply_text(id, &text);
                route_outgoing(&session, &conns, outgoing.clone());
                route(&session, outgoing);
            }
            Ok(OwnerEvent::Conn(ConnEvent::Closed { id })) => {
                session.disconnect(id);
                conns.remove(&id);
            }
            Ok(OwnerEvent::Audio(event)) => {
                if let HostClock::Native(_, link) = &clock {
                    match event {
                        AudioSessionEvent::Interrupted => link.stream.suspend(),
                        AudioSessionEvent::Resumed => {
                            if let Err(diagnostic) = link.stream.resume() {
                                session.push_diagnostic(diagnostic);
                            }
                        }
                        AudioSessionEvent::RouteChanged => link.output.invalidate(),
                    }
                }
                session.clock_discontinuity();
            }
            Ok(OwnerEvent::Shutdown) => break,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if last_tick.elapsed() >= TICK_PERIOD {
            last_tick = Instant::now();
            clock.advance(TICK_PERIOD.as_secs_f64());
            session.observe_clock(clock_reading(&clock));
            let outgoing = session.tick_routed(clock.now());
            route_outgoing(&session, &conns, outgoing.clone());
            route(&session, outgoing);
        }
    }
}

fn clock_reading(clock: &HostClock) -> Option<crate::session::ClockReading> {
    match clock {
        HostClock::Native(_, link) => {
            let reading = link.output.read();
            let kind = match reading.latency_kind {
                crate::host::native::clock::LatencyKind::Measured => {
                    crate::session::LatencyKind::Measured
                }
                crate::host::native::clock::LatencyKind::Estimate => {
                    crate::session::LatencyKind::Estimate
                }
                crate::host::native::clock::LatencyKind::Unavailable => {
                    crate::session::LatencyKind::Unavailable
                }
            };
            Some(crate::session::ClockReading {
                processing_time: reading.processing_time,
                latency_seconds: reading.latency_seconds,
                latency_kind: kind,
                uncertainty_seconds: reading.uncertainty_seconds,
            })
        }
        HostClock::Virtual(_) => None,
    }
}

fn route_outgoing(
    session: &Session,
    conns: &BTreeMap<u32, Sender<String>>,
    outgoing: Vec<Outgoing>,
) {
    for message in outgoing {
        let text = encode(&message.env);
        match message.to {
            Dest::Conn(id) => {
                if let Some(sender) = conns.get(&id) {
                    let _ = sender.send(text);
                }
            }
            Dest::Topic(topic) => {
                for (&id, sender) in conns {
                    if session.wants(id, topic) {
                        let _ = sender.send(text.clone());
                    }
                }
            }
        }
    }
}
