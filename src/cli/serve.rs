//! `vactr serve` (design 14.5.10, 17): binds the loopback session
//! socket, optionally evaluating a file first, and serves until killed.
//! `Session` stays on this (the main) thread; [`crate::cli::ws`] only
//! hands it parsed text frames and connection lifecycle events over
//! `mpsc`.

use std::io::Write;
use std::net::{IpAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::cli::args::HostChoice;
use crate::cli::owner::{self, OwnerEvent};
use crate::cli::ws::{self, ConnLimit};
use crate::cli::{build_session, print_eval_outcome};

/// Exit 1 (cannot read the file, or cannot bind); otherwise runs until
/// killed.
pub fn main(file: Option<PathBuf>, host: HostChoice, port: u16, bind: IpAddr, cwd: &Path) -> i32 {
    let (mut session, clock) = match build_session(host, cwd) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("vactr: {error}");
            return 1;
        }
    };
    if let Some(path) = &file {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let name = path.to_string_lossy().into_owned();
                let (outcome, _batches) = session.eval(&text, &name, 1, 1, None);
                print_eval_outcome(&session, &outcome, &mut std::io::stderr());
            }
            Err(e) => {
                eprintln!("vactr: cannot read `{}`: {e}", path.display());
                return 1;
            }
        }
    }

    let listener = match TcpListener::bind((bind, port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("vactr: cannot bind {bind}:{port}: {e}");
            return 1;
        }
    };
    let local = match listener.local_addr() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("vactr: cannot read the bound address: {e}");
            return 1;
        }
    };
    let token = match ws::new_token() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("vactr: {e}");
            return 1;
        }
    };
    eprintln!("ws://{local}/session?token={token}");
    let _ = std::io::stderr().flush();

    let (events_tx, events_rx) = mpsc::channel::<OwnerEvent>();
    ws::spawn_accept_loop(
        listener,
        token,
        ConnLimit::new(ws::MAX_CONNECTIONS),
        events_tx,
    );

    owner::run_loop(session, clock, events_rx, |_, _| {});
    0
}
