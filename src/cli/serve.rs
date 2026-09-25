//! `vactrol serve` (design 14.5.10, 17): binds the loopback session
//! socket, optionally evaluating a file first, and serves until killed.
//! `Session` stays on this (the main) thread; [`crate::cli::ws`] only
//! hands it parsed text frames and connection lifecycle events over
//! `mpsc`.

use std::collections::BTreeMap;
use std::io::Write;
use std::net::{IpAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Instant;

use crate::cli::args::HostChoice;
use crate::cli::ws::{self, ConnEvent, ConnLimit};
use crate::cli::{build_session, print_eval_outcome, TICK_PERIOD};
use crate::session::codec::encode;
use crate::session::{Dest, Outgoing, Session};

/// Exit 1 (cannot read the file, or cannot bind); otherwise runs until
/// killed.
pub fn main(file: Option<PathBuf>, host: HostChoice, port: u16, bind: IpAddr, cwd: &Path) -> i32 {
    let (mut session, clock) = build_session(host, cwd);
    if let Some(path) = &file {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let name = path.to_string_lossy().into_owned();
                let (outcome, _batches) = session.eval(&text, &name, 1, 1, None);
                print_eval_outcome(&session, &outcome, &mut std::io::stderr());
            }
            Err(e) => {
                eprintln!("vactrol: cannot read `{}`: {e}", path.display());
                return 1;
            }
        }
    }

    let listener = match TcpListener::bind((bind, port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vactrol: cannot bind {bind}:{port}: {e}");
            return 1;
        }
    };
    let local = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("vactrol: cannot read the bound address: {e}");
            return 1;
        }
    };
    let token = match ws::new_token() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("vactrol: {e}");
            return 1;
        }
    };
    eprintln!("ws://{local}/session?token={token}");
    let _ = std::io::stderr().flush();

    let (events_tx, events_rx) = mpsc::channel::<ConnEvent>();
    ws::spawn_accept_loop(
        listener,
        token,
        ConnLimit::new(ws::MAX_CONNECTIONS),
        events_tx,
    );

    let mut conns: BTreeMap<u32, Sender<String>> = BTreeMap::new();
    let mut last_tick = Instant::now();
    loop {
        let wait = TICK_PERIOD.saturating_sub(last_tick.elapsed());
        match events_rx.recv_timeout(wait) {
            Ok(ConnEvent::Connected { id, outbound }) => {
                conns.insert(id, outbound);
            }
            Ok(ConnEvent::Text { id, text }) => {
                let out = session.apply_text(id, &text);
                route(&session, &conns, out);
            }
            Ok(ConnEvent::Closed { id }) => {
                session.disconnect(id);
                conns.remove(&id);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // Tick on the period even under steady client traffic.
        if last_tick.elapsed() >= TICK_PERIOD {
            last_tick = Instant::now();
            clock.advance(TICK_PERIOD.as_secs_f64());
            let out = session.tick_routed(clock.now());
            route(&session, &conns, out);
        }
    }
    0
}

/// Sends each routed message to its requester, or to every connection
/// subscribed to its topic.
fn route(session: &Session, conns: &BTreeMap<u32, Sender<String>>, out: Vec<Outgoing>) {
    for o in out {
        let text = encode(&o.env);
        match o.to {
            Dest::Conn(c) => {
                if let Some(tx) = conns.get(&c) {
                    let _ = tx.send(text);
                }
            }
            Dest::Topic(t) => {
                for (&c, tx) in conns {
                    if session.wants(c, t) {
                        let _ = tx.send(text.clone());
                    }
                }
            }
        }
    }
}
