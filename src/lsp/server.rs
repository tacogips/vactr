//! The tower-lsp `LanguageServer` of `vactr lsp` (design 14.5.11) and
//! the optional session-socket attach client.
//!
//! Handlers hold only `Send` values: the analysis thread's request sender
//! and the tower-lsp client. Each request is forwarded to the analysis
//! thread and its `oneshot` reply awaited, so no `Rc` crosses an `.await`.

use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::task::{Context, Poll};
use std::thread;

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::{mpsc as tmpsc, oneshot};
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams, Hover,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, InitializedParams,
    OneOf, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextEdit,
};
use tower_lsp::{Client, LanguageServer};

use crate::lsp::analysis::{AnalysisReq, Published};
use crate::session::codec::{decode_as, encode};
use crate::session::protocol::{ClientMsg, DiagBody, Envelope, ServerMsg, SubscribeBody};

/// The language server state shared by every handler.
pub struct Backend {
    client: Client,
    analysis: mpsc::Sender<AnalysisReq>,
    /// Set by `shutdown`; `run_stdio` exits 0 only when it is set.
    shutdown: Arc<AtomicBool>,
}

impl Backend {
    /// A backend forwarding to the analysis thread behind `analysis`.
    #[must_use]
    pub fn new(
        client: Client,
        analysis: mpsc::Sender<AnalysisReq>,
        shutdown: Arc<AtomicBool>,
    ) -> Backend {
        Backend {
            client,
            analysis,
            shutdown,
        }
    }

    /// Sends a request built around a fresh reply channel and awaits the
    /// reply; `None` when the analysis thread is gone.
    async fn ask<T>(&self, make: impl FnOnce(oneshot::Sender<T>) -> AnalysisReq) -> Option<T> {
        let (tx, rx) = oneshot::channel();
        self.analysis.send(make(tx)).ok()?;
        rx.await.ok()
    }

    async fn publish(&self, p: Published) {
        self.client
            .publish_diagnostics(p.uri, p.diagnostics, p.version)
            .await;
    }
}

fn gone() -> Error {
    Error::internal_error()
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        #[allow(deprecated)]
        let root: Option<PathBuf> = params
            .workspace_folders
            .as_ref()
            .and_then(|w| w.first())
            .map(|w| w.uri.clone())
            .or(params.root_uri)
            .and_then(|u| u.to_file_path().ok());
        self.analysis
            .send(AnalysisReq::Configure { root })
            .map_err(|_| gone())?;
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![":".into(), ".".into()]),
                    ..CompletionOptions::default()
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "vactr".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let d = params.text_document;
        let p = self
            .ask(|reply| AnalysisReq::Open {
                uri: d.uri,
                version: d.version,
                text: d.text,
                reply,
            })
            .await;
        if let Some(p) = p {
            self.publish(p).await;
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync: the last change holds the whole text.
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        let doc = params.text_document;
        let p = self
            .ask(|reply| AnalysisReq::Change {
                uri: doc.uri,
                version: doc.version,
                text: change.text,
                reply,
            })
            .await;
        if let Some(p) = p {
            self.publish(p).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(p) = self.ask(|reply| AnalysisReq::Close { uri, reply }).await {
            self.publish(p).await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let at = params.text_document_position_params;
        self.ask(|reply| AnalysisReq::Hover {
            uri: at.text_document.uri,
            pos: at.position,
            reply,
        })
        .await
        .ok_or_else(gone)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let at = params.text_document_position;
        let items = self
            .ask(|reply| AnalysisReq::Complete {
                uri: at.text_document.uri,
                pos: at.position,
                reply,
            })
            .await
            .ok_or_else(gone)?;
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        self.ask(|reply| AnalysisReq::Format { uri, reply })
            .await
            .ok_or_else(gone)
    }
}

/// Merges runtime `diag` bodies from the attach thread into the analysis
/// thread's documents and re-publishes the changed ones. Ends when the
/// attach thread or the analysis thread is gone.
pub async fn forward_runtime_diags(
    client: Client,
    analysis: mpsc::Sender<AnalysisReq>,
    mut bodies: tmpsc::UnboundedReceiver<DiagBody>,
) {
    while let Some(body) = bodies.recv().await {
        let (tx, rx) = oneshot::channel();
        if analysis
            .send(AnalysisReq::RuntimeDiags { body, reply: tx })
            .is_err()
        {
            return;
        }
        let Ok(changed) = rx.await else {
            return;
        };
        for p in changed {
            client
                .publish_diagnostics(p.uri, p.diagnostics, p.version)
                .await;
        }
    }
}

/// Connects to the session socket at `url` on a background thread,
/// subscribes to diagnostics and forwards every `diag` body to `out`. A
/// failure logs one line to stderr; the server continues standalone.
///
/// # Errors
/// The OS refused to start the thread.
pub fn attach(url: String, out: tmpsc::UnboundedSender<DiagBody>) -> std::io::Result<()> {
    thread::Builder::new()
        .name("vactr-lsp-attach".into())
        .spawn(move || {
            if let Err(e) = attach_loop(&url, &out) {
                eprintln!("vactr lsp: session attach to {url} ended: {e}; continuing standalone");
            }
        })
        .map(|_| ())
}

fn attach_loop(
    url: &str,
    out: &tmpsc::UnboundedSender<DiagBody>,
) -> std::result::Result<(), String> {
    use tungstenite::Message;
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| e.to_string())?;
    let sub = Envelope::new(
        1,
        None,
        ClientMsg::Subscribe(SubscribeBody {
            telemetry: false,
            levels: false,
            diagnostics: true,
        }),
    );
    ws.send(Message::text(encode(&sub)))
        .map_err(|e| e.to_string())?;
    loop {
        match ws.read().map_err(|e| e.to_string())? {
            Message::Text(t) => {
                if let Ok(Envelope {
                    body: ServerMsg::Diag(body),
                    ..
                }) = decode_as::<ServerMsg>(t.as_str())
                {
                    if out.send(body).is_err() {
                        return Ok(());
                    }
                }
            }
            Message::Close(_) => return Err("the session closed the socket".into()),
            _ => {}
        }
    }
}

/// The client-to-server byte stream, reporting end of input right after the
/// `exit` notification. tower-lsp's `Server::serve` returns only at end of
/// input, so without this a client that keeps the pipe open after `exit`
/// would keep the process alive.
pub struct ExitAwareStdin<R> {
    inner: R,
    /// Bytes of the frame being scanned.
    buf: Vec<u8>,
    /// Set once a complete `exit` notification has been passed through.
    exited: bool,
}

impl<R> ExitAwareStdin<R> {
    /// Wraps `inner`.
    pub fn new(inner: R) -> ExitAwareStdin<R> {
        ExitAwareStdin {
            inner,
            buf: Vec::new(),
            exited: false,
        }
    }

    /// Scans newly read bytes for complete Content-Length frames.
    fn scan(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        loop {
            let Some(head_end) = self.buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                return;
            };
            let body_start = head_end + 4;
            let len = String::from_utf8_lossy(&self.buf[..head_end])
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())
                        .flatten()
                });
            let Some(len) = len else {
                // A malformed header: skip it; tower-lsp reports it.
                self.buf.drain(..body_start);
                continue;
            };
            if self.buf.len() < body_start + len {
                return;
            }
            let body = &self.buf[body_start..body_start + len];
            if is_exit(body) {
                self.exited = true;
            }
            self.buf.drain(..body_start + len);
        }
    }
}

/// True when `body` is the `exit` notification.
fn is_exit(body: &[u8]) -> bool {
    body.windows(6).any(|w| w == b"\"exit\"")
        && serde_json::from_slice::<serde_json::Value>(body).is_ok_and(|v| {
            v.get("method").and_then(serde_json::Value::as_str) == Some("exit")
                && v.get("id").is_none()
        })
}

impl<R: AsyncRead + Unpin> AsyncRead for ExitAwareStdin<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.exited {
            // End of input.
            return Poll::Ready(Ok(()));
        }
        let before = out.filled().len();
        match Pin::new(&mut this.inner).poll_read(cx, out) {
            Poll::Ready(Ok(())) => {
                let fresh = out.filled()[before..].to_vec();
                this.scan(&fresh);
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
