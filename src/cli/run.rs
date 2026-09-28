//! `vactrol run <file.vact>` (design 14.5.10, `command.md`): evaluates the
//! whole document once, prints its diagnostics, console output and form
//! failures, then ticks until interrupted or for `--cycles N`.

use std::io::Write;
use std::path::Path;

use crate::cli::args::HostChoice;
use crate::cli::{build_session, print_eval_outcome, HostClock, TICK_PERIOD};
use crate::session::repl::tick;
use crate::session::Session;

/// Exit 0 (clean), 1 (cannot read the file) or 3 (the document reported
/// error-severity diagnostics or a failed form; the run still ticks the
/// requested cycles).
pub fn main(file: &Path, host: HostChoice, cycles: Option<u64>, cwd: &Path) -> i32 {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("vactrol: cannot read `{}`: {e}", file.display());
            return 1;
        }
    };
    let (mut session, clock) = match build_session(host, cwd) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("vactrol: {error}");
            return 1;
        }
    };
    let file_name = file.to_string_lossy().into_owned();
    let (outcome, _batches) = session.eval(&text, &file_name, 1, 1, None);
    let mut out = std::io::stdout();
    print_eval_outcome(&session, &outcome, &mut out);
    let has_errors = outcome.has_errors();
    match cycles {
        Some(n) => run_cycles(&mut session, &clock, n, &mut out),
        None => run_forever(&mut session, &clock, &mut out),
    }
    if has_errors {
        3
    } else {
        0
    }
}

/// Ticks until `n` cycles have elapsed on the runtime clock, with a safety
/// cap (`n * 600` virtual/real seconds) so a frozen transport cannot hang
/// a test.
fn run_cycles(session: &mut Session, clock: &HostClock, n: u64, out: &mut dyn Write) {
    let dt = TICK_PERIOD.as_secs_f64();
    let start = clock.now();
    let start_cycles = session.runtime().clock().to_cycles(start);
    #[allow(clippy::cast_precision_loss)]
    let n_f = n as f64;
    let cap = start + n_f * 600.0;
    loop {
        clock.advance(dt);
        let now = clock.now();
        let _ = tick(session, now, out);
        let elapsed = session.runtime().clock().to_cycles(now) - start_cycles;
        if elapsed >= n_f || now >= cap {
            break;
        }
    }
}

/// Ticks until the process is interrupted (`run` with no `--cycles`).
fn run_forever(session: &mut Session, clock: &HostClock, out: &mut dyn Write) -> ! {
    loop {
        std::thread::sleep(TICK_PERIOD);
        clock.advance(TICK_PERIOD.as_secs_f64());
        let _ = tick(session, clock.now(), out);
    }
}
