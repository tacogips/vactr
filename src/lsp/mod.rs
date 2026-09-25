//! `vactrol lsp`: the language server over stdio (design 14.3, 14.5.11),
//! built only with the `lsp` feature on non-wasm32 targets.
//!
//! - `analysis`: the ONE analysis thread owning every `!Send` value.
//! - `server`: the tower-lsp handlers and the session attach client.
//! - `convert`: byte offsets, UTF-16 positions and diagnostics.

pub mod analysis;
pub mod convert;
pub mod server;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc as tmpsc;
use tower_lsp::{LspService, Server};

use crate::lsp::server::{attach, forward_runtime_diags, Backend, ExitAwareStdin};

/// Runs `vactrol lsp` over stdio, attached to the session socket at
/// `session` when given; returns the process exit code: 0 after a
/// `shutdown` request and `exit`, else 1.
#[must_use]
pub fn run_stdio(session: Option<&str>) -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("vactrol lsp: cannot start the runtime: {e}");
            return 1;
        }
    };
    let (analysis, worker) = match analysis::spawn() {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("vactrol lsp: cannot start the analysis thread: {e}");
            return 1;
        }
    };
    let shutdown = Arc::new(AtomicBool::new(false));
    let (diag_tx, diag_rx) = tmpsc::unbounded_channel();
    if let Some(url) = session {
        if let Err(e) = attach(url.to_string(), diag_tx) {
            eprintln!("vactrol lsp: cannot start the attach thread: {e}; continuing standalone");
        }
    }
    let flag = Arc::clone(&shutdown);
    rt.block_on(async move {
        let (service, socket) = LspService::new(move |client| {
            tokio::spawn(forward_runtime_diags(
                client.clone(),
                analysis.clone(),
                diag_rx,
            ));
            Backend::new(client, analysis, flag)
        });
        Server::new(
            ExitAwareStdin::new(tokio::io::stdin()),
            tokio::io::stdout(),
            socket,
        )
        .serve(service)
        .await;
    });
    // The stdin reader may still block in the runtime's blocking pool, so
    // shut down without waiting for it. The analysis thread ends once the
    // dropped tasks release its senders; the process does not wait for it.
    rt.shutdown_background();
    drop(worker);
    i32::from(!shutdown.load(Ordering::SeqCst))
}
