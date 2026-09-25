//! `vactrol lsp` stdio smoke test (design 14.5.11): the binary speaks LSP
//! over pipes with Content-Length framing, publishes diagnostics for a
//! lang-reference spec block, answers hover with the checker's type, and
//! exits 0 after `shutdown`/`exit`.
#![cfg(feature = "lsp")]

#[allow(dead_code)]
mod support;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const TIMEOUT: Duration = Duration::from_secs(10);

/// Kills the child on drop so a failing test never leaks the server.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Value>,
    /// Every message received so far, in order.
    seen: Vec<Value>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_frame(r: &mut impl BufRead) -> Option<Value> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = v.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; len?];
    r.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_vactrol"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn vactrol lsp");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut r = BufReader::new(stdout);
            while let Some(v) = read_frame(&mut r) {
                if tx.send(v).is_err() {
                    break;
                }
            }
        });
        Server {
            child,
            stdin,
            rx,
            seen: Vec::new(),
        }
    }

    fn send(&mut self, msg: &Value) {
        let body = msg.to_string();
        let stdin = self.stdin.as_mut().expect("stdin open");
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).expect("write");
        stdin.flush().expect("flush");
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let mut msg = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if !params.is_null() {
            msg["params"] = params;
        }
        self.send(&msg);
        self.wait_for(|m| m.get("id") == Some(&json!(id)) && m.get("method").is_none())
    }

    fn notify(&mut self, method: &str, params: Value) {
        let mut msg = json!({"jsonrpc": "2.0", "method": method});
        if !params.is_null() {
            msg["params"] = params;
        }
        self.send(&msg);
    }

    /// The first message (already seen or arriving within the timeout)
    /// matching `pred`.
    fn wait_for(&mut self, pred: impl Fn(&Value) -> bool) -> Value {
        if let Some(m) = self.seen.iter().find(|m| pred(m)) {
            return m.clone();
        }
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let m = self
                .rx
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("timed out; seen: {:#?}", self.seen));
            self.seen.push(m.clone());
            if pred(&m) {
                return m;
            }
        }
    }

    fn diagnostics_for(&mut self, uri: &str) -> Value {
        self.wait_for(|m| {
            m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == uri
        })
    }

    fn hover(&mut self, id: u64, uri: &str, line: u32, character: u32) -> String {
        let r = self.request(
            id,
            "textDocument/hover",
            json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
        );
        r["result"]["contents"]["value"]
            .as_str()
            .unwrap_or_else(|| panic!("hover contents: {r}"))
            .to_string()
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "vactrol", "version": 1, "text": text}}),
        );
    }
}

fn codes(publish: &Value) -> Vec<String> {
    publish["params"]["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter_map(|d| d["code"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn lsp_over_stdio_publishes_diagnostics_hovers_and_exits_cleanly() {
    let block = support::spec_blocks(&support::spec_doc("lang-reference.md"))
        .into_iter()
        .next()
        .expect("lang-reference.md block 1");
    let mut s = Server::start();
    let init = s.request(
        1,
        "initialize",
        json!({"processId": null, "rootUri": null, "capabilities": {}}),
    );
    let caps = &init["result"]["capabilities"];
    assert_eq!(caps["textDocumentSync"], json!(1), "full sync: {init}");
    assert_eq!(caps["hoverProvider"], json!(true));
    assert_eq!(caps["documentFormattingProvider"], json!(true));
    s.notify("initialized", json!({}));

    // Block 1 (`+ 43 32 > * 12 > print ...`) checks clean.
    let spec = "file:///spec/lang-reference-1.vact";
    s.open(spec, &block);
    let published = s.diagnostics_for(spec);
    assert!(codes(&published).is_empty(), "{published}");
    let lit = block.find("43").expect("43");
    let hover = s.hover(2, spec, 0, u32::try_from(lit).expect("col"));
    assert!(hover.contains("int"), "{hover}");

    // A bound name hovers with its type; an unbound one is diagnosed.
    let doc = "file:///spec/second.vact";
    s.open(doc, "let total 43\ntotal\nnope-not-bound\n");
    let published = s.diagnostics_for(doc);
    assert!(
        codes(&published).iter().any(|c| c == "undefined-name"),
        "{published}"
    );
    let hover = s.hover(3, doc, 1, 1);
    assert!(hover.contains("total: int"), "{hover}");

    // Exactly one publishDiagnostics for the spec block's URI.
    let n = s
        .seen
        .iter()
        .filter(|m| m["method"] == "textDocument/publishDiagnostics" && m["params"]["uri"] == spec)
        .count();
    assert_eq!(n, 1);

    let shut = s.request(4, "shutdown", Value::Null);
    assert!(shut.get("error").is_none(), "{shut}");
    s.notify("exit", Value::Null);
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(st) = s.child.try_wait().expect("try_wait") {
            break st;
        }
        assert!(
            Instant::now() < deadline,
            "vactrol lsp did not exit within {TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
}
