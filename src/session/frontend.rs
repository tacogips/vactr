//! Additive `Session` methods the browser editor needs (design 15.1.2 G5,
//! G6), kept out of `session.rs` (703 lines) so it barely grows.
//!
//! - [`Session::render_frame`]: forwards to `Runtime::render_frame` (G5),
//!   reporting a failing uniform as one `diag` message.
//! - [`Session::drive_packages`]: lends the session's own package cache
//!   (installing an in-memory one when the session has none) to the
//!   package driver, and installs the resulting lock on success (G6).
//! - [`Session::check`]: static analysis of arbitrary text — never the
//!   session's own document state — via `session::eval::analyze` plus the
//!   directive lint, with NO evaluation and no state change.

use crate::directives::build_table;
use crate::pkg::driver::{drive, DriverReply, DriverRequest, Prefetched};
use crate::pkg::lock::LockFile;
use crate::pkg::mem_cache::MemCache;
use crate::reader::span::{FileId, Span};
use crate::session::eval::{analyze, PackageView};
use crate::session::protocol::{DiagBody, ServerMsg, WireDiag};
use crate::session::publish::{diag_wire, failure_wire};
use crate::session::session::Session;

impl Session {
    /// Resolves every output's stored uniform plan at `host_now`
    /// (`Runtime::render_frame`, design 9.3, G5); returns one `diag`
    /// message naming the failures, or nothing when every plan resolved.
    pub fn render_frame(&mut self, host_now: f64) -> Vec<ServerMsg> {
        let faults = self.rt.render_frame(&mut self.ev, host_now);
        if faults.is_empty() {
            return Vec::new();
        }
        let files = &self.files;
        let nowhere = Span::new(FileId::CONSOLE, 0, 0);
        let add = faults
            .iter()
            .map(|f| failure_wire(files, f, nowhere))
            .collect();
        vec![ServerMsg::Diag(DiagBody {
            add,
            clear: Vec::new(),
        })]
    }

    /// Runs one package-driver step (`pkg::driver::drive`, design 15.1.2
    /// G6) over the session's own cache, installing a fresh in-memory one
    /// (`MemCache`) when the session has none. On `Done`, the rendered
    /// lock text replaces `self.lock`, so the next `eval` of an `import`
    /// loads exactly what the driver just published.
    pub fn drive_packages(&mut self, req: DriverRequest, supplied: &mut Prefetched) -> DriverReply {
        let cache = self.cache.get_or_insert_with(|| Box::new(MemCache::new()));
        let reply = drive(req, supplied, &mut **cache);
        if let DriverReply::Done { lock, .. } = &reply {
            if let Ok(parsed) = LockFile::parse(lock) {
                self.lock = Some(parsed);
            }
        }
        reply
    }

    /// Static analysis of `text` as `file` (design 14.5.7 "LSP analysis",
    /// G6): `session::eval::analyze` over the session's own lock and cache
    /// plus the directive lint, converted to wire diagnostics. This NEVER
    /// executes `text` and never changes session state — `file` need not
    /// even be an open document.
    #[must_use]
    pub fn check(&self, file: &str, text: &str) -> Vec<WireDiag> {
        let fid = self.lookup_file(file).unwrap_or(FileId::CONSOLE);
        let view = PackageView {
            lock: self.lock.as_ref(),
            cache: self.cache.as_deref(),
        };
        let analysis = analyze(text, fid, &view);
        let mut diags = analysis.diags;
        let (_, lint) = build_table(text, fid, &analysis.nodes, &analysis.trivia, &self.manifest);
        diags.extend(lint);
        diags
            .iter()
            .map(|d| {
                let mut w = diag_wire(&self.files, d);
                w.file = file.to_string();
                w
            })
            .collect()
    }
}
