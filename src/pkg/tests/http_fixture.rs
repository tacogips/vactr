//! A local HTTP fixture: a `TcpListener` on `127.0.0.1:0` serving an
//! in-memory path -> bytes map on a thread (HTTP/1.1 GET, 404 otherwise,
//! stopped on drop), and a `ProxyTransport` client that accepts only
//! `http://127.0.0.1:<port>/` URLs. No public network is ever reached.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::pkg::proxy::ProxyTransport;
use crate::pkg::store::PkgError;

/// The serving thread and its address.
pub(super) struct HttpFixture {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Every requested path, in order.
    pub requests: Arc<Mutex<Vec<String>>>,
}

fn serve(mut stream: TcpStream, files: &BTreeMap<String, Vec<u8>>, log: &Mutex<Vec<String>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut req = Vec::new();
    let mut buf = [0u8; 1024];
    while !req.windows(4).any(|w| w == b"\r\n\r\n") && req.len() < 16 * 1024 {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => req.extend_from_slice(&buf[..n]),
        }
    }
    let text = String::from_utf8_lossy(&req);
    let mut words = text.lines().next().unwrap_or("").split(' ');
    let (method, path) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    if let Ok(mut l) = log.lock() {
        l.push(path.to_string());
    }
    let (status, body): (&str, &[u8]) = match files.get(path) {
        Some(b) if method == "GET" => ("200 OK", b),
        _ => ("404 Not Found", b"not found"),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

impl HttpFixture {
    /// Serves `files` (keyed by URL path, e.g. `/github.com/o/n/@v/list`).
    pub(super) fn start(files: BTreeMap<String, Vec<u8>>) -> HttpFixture {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("local addr");
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (flag, log) = (Arc::clone(&stop), Arc::clone(&requests));
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(s) = stream {
                    serve(s, &files, &log);
                }
            }
        });
        HttpFixture {
            addr,
            stop,
            thread: Some(thread),
            requests,
        }
    }

    /// `http://127.0.0.1:<port>`.
    pub(super) fn base(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A plain HTTP/1.1 client for loopback fixture URLs only.
pub(super) struct LocalClient;

impl ProxyTransport for LocalClient {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, PkgError> {
        let rest = url
            .strip_prefix("http://127.0.0.1:")
            .ok_or_else(|| PkgError::Network(format!("`{url}`: only loopback URLs")))?;
        let (port, path) = rest
            .split_once('/')
            .ok_or_else(|| PkgError::Network(format!("`{url}`: no path")))?;
        let port: u16 = port
            .parse()
            .map_err(|_| PkgError::Network(format!("`{url}`: bad port")))?;
        let net = |e: std::io::Error| PkgError::Network(format!("`{url}`: {e}"));
        let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(net)?;
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(net)?;
        write!(
            s,
            "GET /{path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
        )
        .map_err(net)?;
        let mut resp = Vec::new();
        s.read_to_end(&mut resp).map_err(net)?;
        let split = resp
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| PkgError::Network(format!("`{url}`: malformed response")))?;
        let head = String::from_utf8_lossy(&resp[..split]).into_owned();
        let body = resp[split + 4..].to_vec();
        match head.lines().next().and_then(|l| l.split(' ').nth(1)) {
            Some("200") => Ok(body),
            Some("404") => Err(PkgError::Unresolvable(format!("`{url}`: not found"))),
            other => Err(PkgError::Network(format!("`{url}`: status {other:?}"))),
        }
    }
}
