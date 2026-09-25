//! The console (design 14.2, 14.5.10): console-file evaluation, the `_n`
//! registers and the transcript.
//!
//! A console entry runs through the same pipeline as a document, as the
//! file `FileId::CONSOLE` (where the reader accepts `_1`, `_2`, ...). Each
//! completed, non-failing form binds the NEXT register in the namespace's
//! console slots and prints `_n = <value>`; a failing one prints its
//! diagnostic and binds nothing, so registers count completed forms only.

use crate::reader::span::FileId;
use crate::session::publish::file_name;
use crate::session::session::{Dest, Outgoing, Session};
use crate::types::diag::Diagnostic;
use crate::vm::fail::Failure;

/// What one console entry did.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ConsoleOutcome {
    /// The transcript lines, in order: static diagnostics, console output,
    /// then each form's `_n = value` or failure.
    pub lines: Vec<String>,
    /// The registers this entry bound.
    pub registers: Vec<u32>,
    /// True when a form failed or an error was reported.
    pub failed: bool,
}

/// `severity[code] file:start..end: message`.
#[must_use]
pub fn format_diag(files_name: &str, d: &Diagnostic) -> String {
    format!(
        "{}[{}] {}:{}..{}: {}",
        d.severity.as_str(),
        d.code,
        files_name,
        d.span.start,
        d.span.end,
        d.message
    )
}

/// `error[code]: message` for a runtime failure.
#[must_use]
pub fn format_failure(f: &Failure) -> String {
    let mut s = format!("error[{}]: {}", f.code, f.message);
    if let Some(slot) = f.origin.slot {
        s.push_str(&format!(
            " (slot {})",
            crate::value::intern::name_of_kw(slot)
        ));
    }
    s
}

impl Session {
    /// Evaluates one console entry.
    pub fn eval_console(&mut self, src: &str) -> ConsoleOutcome {
        let (rev, epoch) = self
            .docs
            .get(&FileId::CONSOLE)
            .map_or((1, 0), |d| (d.text_rev + 1, d.epoch_reconciled));
        let (out, batches) = self.eval_doc(src, FileId::CONSOLE, rev, epoch, None);
        for b in batches {
            let routed: Vec<Outgoing> = self.route(0, None, vec![b]);
            self.outbox.extend(
                routed
                    .into_iter()
                    .filter(|o| matches!(o.to, Dest::Topic(_))),
            );
        }
        let mut res = ConsoleOutcome {
            failed: out.has_errors(),
            ..ConsoleOutcome::default()
        };
        for d in &out.diagnostics {
            let name = file_name(&self.files, d.span.file);
            res.lines.push(format_diag(&name, d));
        }
        res.lines.extend(out.faults.iter().map(format_failure));
        res.lines.extend(out.console.iter().cloned());
        for f in &out.forms {
            match (&f.value, &f.failure) {
                (_, Some(e)) => res.lines.push(format_failure(e)),
                (Some(v), None) => {
                    self.registers += 1;
                    let n = self.registers;
                    self.ev.ns().set_console_register(n, v.clone());
                    res.registers.push(n);
                    res.lines.push(format!("_{n} = {v}"));
                }
                (None, None) => {}
            }
        }
        res
    }

    /// The number of console registers bound so far.
    #[must_use]
    pub fn registers(&self) -> u32 {
        self.registers
    }
}
