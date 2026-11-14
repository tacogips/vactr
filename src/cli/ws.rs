//! The native loopback session socket (design 14.5.10, 17): one accept
//! thread and one thread per connection, all owned by [`crate::cli::serve`].
//! `Session` never leaves the main (evaluator) thread (17.1); connection
//! threads only move parsed text frames and outbound strings across `mpsc`
//! channels.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::{Response as HttpResponse, StatusCode};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

use crate::cli::owner::OwnerEvent;
use crate::pkg::digest::hex;
use crate::session::codec::MAX_FRAME;

/// At most this many live connections (14.5.10); the next handshake gets
/// HTTP 503.
pub(crate) const MAX_CONNECTIONS: usize = 8;

/// Each connection's socket read timeout, so its thread can also flush its
/// outbound queue between reads.
const READ_TIMEOUT: Duration = Duration::from_millis(5);

/// How long a client may take to complete the handshake, so a silent
/// client cannot hold one of the [`MAX_CONNECTIONS`] slots forever.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// A fresh 32-byte token from the OS generator, lowercase hex (design 17).
///
/// # Errors
/// The generator's error; there is no weaker fallback, so `serve` refuses
/// to start instead.
pub(crate) fn new_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).map_err(|e| format!("cannot generate the session token: {e}"))?;
    Ok(hex(&buf))
}

/// Constant-time byte comparison: a length mismatch is `false` at once; an
/// equal-length compare never exits early on the first differing byte.
#[must_use]
pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// The handshake's path and token check (a pure function): the path must
/// be `/session` and the `token` query parameter must match `token` under
/// [`ct_eq`]. Anything else is HTTP 401.
///
/// # Errors
/// The HTTP status to answer with.
pub(crate) fn check_request(path_and_query: &str, token: &str) -> Result<(), u16> {
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    if path != "/session" {
        return Err(401);
    }
    let given = query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == "token").then_some(v)
    });
    match given {
        Some(v) if ct_eq(v.as_bytes(), token.as_bytes()) => Ok(()),
        _ => Err(401),
    }
}

fn error_response(code: u16) -> ErrorResponse {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    HttpResponse::builder()
        .status(status)
        .body(None::<String>)
        .unwrap_or_else(|_| {
            let mut resp = HttpResponse::new(None);
            *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            resp
        })
}

/// A shared connection-count gate.
#[derive(Clone)]
pub(crate) struct ConnLimit {
    count: Arc<AtomicUsize>,
    max: usize,
}

impl ConnLimit {
    pub(crate) fn new(max: usize) -> ConnLimit {
        ConnLimit {
            count: Arc::new(AtomicUsize::new(0)),
            max,
        }
    }

    /// Tries to take one slot; `false` when `max` are already held.
    #[must_use]
    pub(crate) fn acquire(&self) -> bool {
        self.count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < self.max).then_some(n + 1)
            })
            .is_ok()
    }

    /// Releases one slot.
    pub(crate) fn release(&self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Releases a [`ConnLimit`] slot when a connection thread ends, on every
/// path (a clean close, a handshake rejection, a read error or a panic).
struct ReleaseOnDrop(ConnLimit);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// What a connection thread reports to the main (evaluator) thread.
pub(crate) enum ConnEvent {
    /// The handshake succeeded; `outbound` queues text frames to send.
    Connected {
        id: u32,
        outbound: mpsc::Sender<String>,
    },
    /// One decoded text frame.
    Text { id: u32, text: String },
    /// The connection closed or failed.
    Closed { id: u32 },
}

/// Spawns the accept loop: one thread that accepts connections, gates them
/// on `limit` (a raw HTTP 503 before the handshake when full), and spawns
/// one thread per accepted connection.
pub(crate) fn spawn_accept_loop(
    listener: TcpListener,
    token: String,
    limit: ConnLimit,
    events: mpsc::Sender<OwnerEvent>,
) {
    thread::spawn(move || {
        let mut next_id: u32 = 1;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            if !limit.acquire() {
                let _ = stream.write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\n\
                      Content-Length: 0\r\nConnection: close\r\n\r\n",
                );
                continue;
            }
            let id = next_id;
            next_id = next_id.wrapping_add(1);
            if next_id == 0 {
                next_id = 1;
            }
            let token = token.clone();
            let limit = limit.clone();
            let events = events.clone();
            thread::spawn(move || {
                let _guard = ReleaseOnDrop(limit);
                handle_connection(stream, id, &token, &events);
            });
        }
    });
}

fn handle_connection(stream: TcpStream, id: u32, token: &str, events: &mpsc::Sender<OwnerEvent>) {
    let token = token.to_string();
    let callback = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        let pq = req
            .uri()
            .path_and_query()
            .map_or_else(|| req.uri().path().to_string(), |p| p.as_str().to_string());
        match check_request(&pq, &token) {
            Ok(()) => Ok(resp),
            Err(code) => Err(error_response(code)),
        }
    };
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME))
        .max_frame_size(Some(MAX_FRAME));
    // A timed-out handshake read fails the handshake and frees the slot.
    let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    match tungstenite::accept_hdr_with_config(stream, callback, Some(config)) {
        Ok(ws) => run_connection(ws, id, events),
        Err(_) => {
            let _ = events.send(OwnerEvent::Conn(ConnEvent::Closed { id }));
        }
    }
}

fn run_connection(mut ws: WebSocket<TcpStream>, id: u32, events: &mpsc::Sender<OwnerEvent>) {
    let _ = ws.get_ref().set_read_timeout(Some(READ_TIMEOUT));
    let (out_tx, out_rx) = mpsc::channel::<String>();
    if events
        .send(OwnerEvent::Conn(ConnEvent::Connected {
            id,
            outbound: out_tx,
        }))
        .is_err()
    {
        return;
    }
    loop {
        let mut sent_any = false;
        while let Ok(text) = out_rx.try_recv() {
            sent_any = true;
            if ws.send(Message::Text(text.into())).is_err() {
                let _ = events.send(OwnerEvent::Conn(ConnEvent::Closed { id }));
                return;
            }
        }
        if sent_any && ws.flush().is_err() {
            let _ = events.send(OwnerEvent::Conn(ConnEvent::Closed { id }));
            return;
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                if events
                    .send(OwnerEvent::Conn(ConnEvent::Text {
                        id,
                        text: text.to_string(),
                    }))
                    .is_err()
                {
                    return;
                }
            }
            Ok(Message::Close(_)) => {
                let _ = events.send(OwnerEvent::Conn(ConnEvent::Closed { id }));
                return;
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                let _ = events.send(OwnerEvent::Conn(ConnEvent::Closed { id }));
                return;
            }
        }
    }
}
