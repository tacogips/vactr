//! `vactr repl` (design 14.2, 14.5.10): a stdin reader thread feeding
//! `session::repl`'s `LineBuffer`/`submit`/`tick` on the main (evaluator)
//! thread, which never blocks longer than `TICK_PERIOD` so sound keeps
//! playing between lines.

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;

use crate::cli::args::HostChoice;
use crate::cli::{build_session, TICK_PERIOD};
use crate::session::repl::{submit, tick, LineBuffer};

const PROMPT: &str = "vactr> ";
const CONTINUATION: &str = "....> ";

/// Runs the REPL to EOF (Ctrl-D); always exits 0.
pub fn main(host: HostChoice, cwd: &Path) -> i32 {
    let (mut session, clock) = match build_session(host, cwd) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("vactr: {error}");
            return 1;
        }
    };
    let interactive = std::io::stdin().is_terminal();
    let (tx, rx) = mpsc::channel::<String>();
    let spawned = thread::Builder::new()
        .name("vactr-repl-stdin".into())
        .spawn(move || {
            // Bytes, not `lines()`: a line that is not UTF-8 is evaluated
            // (and diagnosed) lossily instead of ending the REPL.
            let mut stdin = std::io::stdin().lock();
            let mut raw = Vec::new();
            loop {
                raw.clear();
                match stdin.read_until(b'\n', &mut raw) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&raw).into_owned();
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    if spawned.is_err() {
        eprintln!("vactr: cannot start the stdin reader thread");
        return 1;
    }

    let mut out = std::io::stdout();
    let mut buf = LineBuffer::default();
    let mut prompted = false;
    loop {
        if interactive && !prompted {
            let p = if buf.is_open() { CONTINUATION } else { PROMPT };
            let _ = write!(out, "{p}");
            let _ = out.flush();
            prompted = true;
        }
        match rx.recv_timeout(TICK_PERIOD) {
            Ok(line) => {
                clock.advance(TICK_PERIOD.as_secs_f64());
                let _ = tick(&mut session, clock.now(), &mut out);
                for entry in buf.feed(&line) {
                    let _ = submit(&mut session, &entry, &mut out);
                }
                prompted = false;
            }
            Err(RecvTimeoutError::Timeout) => {
                clock.advance(TICK_PERIOD.as_secs_f64());
                let _ = tick(&mut session, clock.now(), &mut out);
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Some(entry) = buf.finish() {
        let _ = submit(&mut session, &entry, &mut out);
    }
    clock.advance(TICK_PERIOD.as_secs_f64());
    let _ = tick(&mut session, clock.now(), &mut out);
    let _ = out.flush();
    0
}
