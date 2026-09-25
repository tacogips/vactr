//! The REPL line loop (design 14.2, 14.5.10), over `BufRead`/`Write` so it
//! is testable. The CLI supplies the threads, the prompt and the real
//! clock; it feeds lines through the same `LineBuffer` rules.
//!
//! Continuation: a line ending in the block colon opens an entry; indented
//! lines, `>` lines and further colon lines continue it; a blank line (or
//! a new unindented statement) submits it. EOF submits what is pending and
//! ends the loop. Between lines the session ticks on `clock`, so sound
//! keeps playing and runtime diagnostics print as they arrive.

use std::io::{BufRead, Write};

use crate::session::protocol::ServerMsg;
use crate::session::session::Session;

/// True when `line` (without a trailing comment) ends in a block colon.
fn opens_block(line: &str) -> bool {
    let code = line.split('#').next().unwrap_or("");
    code.trim_end().ends_with(':')
}

/// Accumulates console lines into entries.
#[derive(Clone, Debug, Default)]
pub struct LineBuffer {
    buf: String,
    open: bool,
}

impl LineBuffer {
    /// True while a block entry is open (the CLI shows `....> `).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    fn take(&mut self) -> String {
        self.open = false;
        std::mem::take(&mut self.buf)
    }

    /// Feeds one line; returns the entries it completes, in order.
    pub fn feed(&mut self, line: &str) -> Vec<String> {
        let line = line.trim_end_matches(['\r', '\n']);
        let mut ready = Vec::new();
        if self.open {
            if line.trim().is_empty() {
                ready.push(self.take());
                return ready;
            }
            let continues = line.starts_with([' ', '\t'])
                || line.trim_start().starts_with('>')
                || opens_block(line);
            if continues {
                self.buf.push('\n');
                self.buf.push_str(line);
                return ready;
            }
            ready.push(self.take());
        }
        if line.trim().is_empty() {
            return ready;
        }
        if opens_block(line) {
            self.buf = line.to_string();
            self.open = true;
        } else {
            ready.push(line.to_string());
        }
        ready
    }

    /// The pending entry at EOF, if any.
    pub fn finish(&mut self) -> Option<String> {
        let s = self.take();
        (!s.trim().is_empty()).then_some(s)
    }
}

/// Evaluates one entry and writes its transcript.
///
/// # Errors
/// Only a write error on `out`.
pub fn submit(session: &mut Session, src: &str, out: &mut dyn Write) -> std::io::Result<()> {
    let res = session.eval_console(src);
    for line in &res.lines {
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// Ticks the session at `now` and writes the runtime diagnostics and
/// console lines that arrived.
///
/// # Errors
/// Only a write error on `out`.
pub fn tick(session: &mut Session, now: f64, out: &mut dyn Write) -> std::io::Result<()> {
    for msg in session.tick(now) {
        if let ServerMsg::Diag(d) = msg {
            for a in &d.add {
                let slot = a
                    .slot
                    .as_deref()
                    .map(|s| format!(" (slot {s})"))
                    .unwrap_or_default();
                writeln!(out, "{}[{}]{slot}: {}", a.severity, a.code, a.message)?;
            }
        }
    }
    for line in session.take_console() {
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// The line loop: reads `input` to EOF, ticking via `clock` before every
/// line and once at the end.
///
/// # Errors
/// A read or write error.
pub fn run_repl(
    session: &mut Session,
    mut input: impl BufRead,
    mut out: impl Write,
    clock: &mut dyn FnMut() -> f64,
) -> std::io::Result<()> {
    let mut lines = LineBuffer::default();
    let mut raw = Vec::new();
    loop {
        raw.clear();
        let n = input.read_until(b'\n', &mut raw)?;
        if n == 0 {
            break;
        }
        tick(session, clock(), &mut out)?;
        let line = String::from_utf8_lossy(&raw);
        for entry in lines.feed(&line) {
            submit(session, &entry, &mut out)?;
        }
    }
    if let Some(entry) = lines.finish() {
        submit(session, &entry, &mut out)?;
    }
    tick(session, clock(), &mut out)?;
    out.flush()
}
