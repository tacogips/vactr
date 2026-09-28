use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::{HoverContents, NumberOrString, Position, TextEdit, Url};

use crate::lsp::analysis::{format_edits, Analyzer, PkgConfig, LOCK_FILE};
use crate::lsp::convert::{offset, position};
use crate::pkg::native::fs_cache::STAGING_DIR;
use crate::session::protocol::{DiagBody, WireClear, WireDiag, WireSpan};

/// A fresh directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("vactr-lsp-{tag}-{}-{nanos:x}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn uri(name: &str) -> Url {
    Url::parse(&format!("file:///work/{name}")).expect("url")
}

fn open(text: &str) -> (Analyzer, Url) {
    let mut an = Analyzer::new(PkgConfig::default());
    let u = uri("a.vact");
    an.open(u.clone(), 1, text.to_string());
    (an, u)
}

fn hover_text(an: &Analyzer, u: &Url, line: u32, character: u32) -> String {
    let h = an
        .hover(u, Position::new(line, character))
        .expect("a hover");
    match h.contents {
        HoverContents::Markup(m) => m.value,
        other => panic!("markup hover expected, got {other:?}"),
    }
}

/// Applies non-overlapping edits (any order) to `text`.
fn apply(text: &str, edits: &[TextEdit]) -> String {
    let mut spans: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|e| {
            (
                offset(text, e.range.start),
                offset(text, e.range.end),
                e.new_text.as_str(),
            )
        })
        .collect();
    spans.sort_by_key(|s| s.0);
    let mut out = String::new();
    let mut at = 0;
    for (a, b, new) in spans {
        assert!(a >= at, "edits overlap");
        out.push_str(&text[at..a]);
        out.push_str(new);
        at = b;
    }
    out.push_str(&text[at..]);
    out
}

#[test]
fn utf16_positions_round_trip_around_multibyte_strings() {
    // `é` is 2 UTF-8 bytes / 1 UTF-16 unit; `😀` is 4 bytes / 2 units.
    let text = "let s \"é😀x\"\nab";
    let x = text.find('x').expect("x");
    assert_eq!(position(text, x), Position::new(0, 10));
    assert_eq!(offset(text, Position::new(0, 10)), x);
    let emoji = text.find('😀').expect("emoji");
    assert_eq!(position(text, emoji), Position::new(0, 8));
    // A column inside the surrogate pair rounds down to the emoji.
    assert_eq!(offset(text, Position::new(0, 9)), emoji);
    // Inside a UTF-8 sequence rounds down to its character.
    assert_eq!(position(text, emoji + 1), Position::new(0, 8));
    let b = text.rfind('b').expect("b");
    assert_eq!(position(text, b), Position::new(1, 1));
    assert_eq!(offset(text, Position::new(1, 1)), b);
    // Past the line end clamps to it; past the text clamps to the end.
    assert_eq!(
        offset(text, Position::new(0, 99)),
        text.find('\n').expect("nl")
    );
    assert_eq!(offset(text, Position::new(9, 0)), text.len());
    assert_eq!(position(text, 999), Position::new(1, 2));
}

#[test]
fn hover_over_a_let_bound_number_gives_its_type() {
    let (an, u) = open("let a 12\nlet f 1.5\na\nf\n");
    let a = hover_text(&an, &u, 2, 0);
    assert!(a.contains("int"), "{a}");
    assert!(a.contains('a'), "{a}");
    let f = hover_text(&an, &u, 3, 0);
    assert!(f.contains("float"), "{f}");
    // The literal itself.
    let lit = hover_text(&an, &u, 0, 6);
    assert!(lit.contains("int"), "{lit}");
    // Nothing typed at an empty line past the end.
    assert!(an.hover(&u, Position::new(9, 0)).is_none());
}

#[test]
fn completion_offers_prelude_names_keywords_and_document_names() {
    let (mut an, u) = open("let tempo-x 12\n\n");
    let labels: Vec<String> = an
        .complete(&u, Position::new(1, 0))
        .into_iter()
        .map(|c| c.label)
        .collect();
    for want in ["sine", ":bd", ":analog", "tempo-x"] {
        assert!(labels.iter().any(|l| l == want), "{want} missing");
    }
    // The word before the cursor filters: `:b` keeps keywords only.
    let (mut an, u) = open(":b");
    let items = an.complete(&u, Position::new(0, 2));
    assert!(items.iter().any(|c| c.label == ":bd"));
    assert!(items.iter().all(|c| c.label.starts_with(":b")));
}

#[test]
fn formatting_changes_only_whitespace_and_keeps_directive_lines() {
    let text = "let a 12   \n#@ name kick   \ns :bd > d1  #@ gain 0.5\t\n\t\nlet b 3\t \n\n\n";
    let edits = format_edits(text);
    assert!(!edits.is_empty());
    for e in &edits {
        let a = offset(text, e.range.start);
        let b = offset(text, e.range.end);
        assert!(text[a..b].chars().all(char::is_whitespace), "{e:?}");
        assert!(e.new_text.chars().all(char::is_whitespace), "{e:?}");
    }
    let out = apply(text, &edits);
    assert_eq!(
        out,
        "let a 12\n#@ name kick   \ns :bd > d1  #@ gain 0.5\t\n\nlet b 3\n"
    );
    // Every `#@` line is byte-identical.
    let before: Vec<&str> = text.lines().filter(|l| l.contains("#@")).collect();
    let after: Vec<&str> = out.lines().filter(|l| l.contains("#@")).collect();
    assert_eq!(before, after);
    // A missing final newline is added; clean text needs no edit.
    assert_eq!(
        apply("s :bd > d1", &format_edits("s :bd > d1")),
        "s :bd > d1\n"
    );
    assert!(format_edits("let a 1\n").is_empty());
    assert!(format_edits("").is_empty());
    // CRLF terminators are kept.
    assert_eq!(
        apply("a  \r\nb\r\n\r\n", &format_edits("a  \r\nb\r\n\r\n")),
        "a\r\nb\r\n"
    );
}

#[test]
fn an_unfetched_import_is_a_warning_and_nothing_is_fetched_or_written() {
    let tmp = TempDir::new("pkg");
    let root = tmp.path().join("ws");
    let cache = tmp.path().join("cache");
    std::fs::create_dir_all(&root).expect("root");
    std::fs::create_dir_all(cache.join(STAGING_DIR)).expect("cache");
    let digest = "0".repeat(64);
    std::fs::write(
        root.join(LOCK_FILE),
        format!("# vactr.lock v1\ngithub.com/acme/pads v1.0.0 sha256:{digest}\n"),
    )
    .expect("lock");
    let pkgs = PkgConfig::load(Some(&root), &cache);
    assert!(pkgs.lock.is_some(), "the workspace lock is read");
    assert!(pkgs.cache.is_some(), "an existing cache is opened");
    let mut an = Analyzer::new(pkgs);
    let u = uri("p.vact");
    let p = an.open(
        u.clone(),
        1,
        "import github.com/acme/pads\npads.warm > d1\nprint \"ran\"\n".to_string(),
    );
    let d = p
        .diagnostics
        .iter()
        .find(|d| d.code == Some(NumberOrString::String("package-not-fetched".into())))
        .expect("package-not-fetched");
    assert_eq!(
        d.severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING)
    );
    assert_eq!(d.source.as_deref(), Some("vactr"));
    assert!(p
        .diagnostics
        .iter()
        .all(|d| d.code != Some(NumberOrString::String("undefined-name".into()))));
    // Analysis wrote nothing: the cache holds only its empty staging dir,
    // and the workspace holds only the lock.
    let entries = |p: &Path| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(p)
            .expect("dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    };
    assert_eq!(entries(&cache), vec![STAGING_DIR.to_string()]);
    assert!(entries(&cache.join(STAGING_DIR)).is_empty());
    assert_eq!(entries(&root), vec![LOCK_FILE.to_string()]);
    // A missing cache is never created.
    let absent = tmp.path().join("absent-cache");
    assert!(PkgConfig::load(Some(&root), &absent).cache.is_none());
    assert!(!absent.exists());
}

#[test]
fn undefined_names_and_directive_lint_are_published() {
    let u = uri("u.vact");
    let mut an = Analyzer::new(PkgConfig::default());
    let p = an.open(u.clone(), 7, "nope-not-bound\n".to_string());
    assert_eq!(p.version, Some(7));
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.code == Some(NumberOrString::String("undefined-name".into()))));
    let p = an.open(
        u.clone(),
        8,
        "s :bd > d1\n#@ name kick\ns :sd > d2\n#@ name kick\n".to_string(),
    );
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == Some(NumberOrString::String("duplicate-label".into()))),
        "{:?}",
        p.diagnostics
    );
    let closed = an.close(u);
    assert!(closed.diagnostics.is_empty());
}

fn wire(file: &str, slot: &str, start: u32) -> WireDiag {
    WireDiag {
        code: "type-mismatch".into(),
        severity: "error".into(),
        message: "runtime failure".into(),
        span: WireSpan {
            start,
            end: start + 2,
        },
        file: file.into(),
        slot: Some(slot.into()),
        beat: None,
    }
}

#[test]
fn runtime_diags_merge_into_the_matching_document_and_clear_by_slot() {
    let (mut an, u) = open("s :bd > d1\n");
    let base = an
        .open(u.clone(), 2, "s :bd > d1\n".to_string())
        .diagnostics
        .len();
    let changed = an.runtime_diags(&DiagBody {
        add: vec![wire("a.vact", "d1", 0), wire("elsewhere.vact", "d2", 0)],
        clear: Vec::new(),
    });
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].uri, u);
    assert_eq!(changed[0].diagnostics.len(), base + 1);
    // A repeat of the same diagnostic changes nothing.
    let again = an.runtime_diags(&DiagBody {
        add: vec![wire("a.vact", "d1", 0)],
        clear: Vec::new(),
    });
    assert!(again.is_empty());
    // An absolute path naming the document matches too.
    let abs = an.runtime_diags(&DiagBody {
        add: vec![wire("/work/a.vact", "d3", 4)],
        clear: Vec::new(),
    });
    assert_eq!(abs.len(), 1);
    assert_eq!(abs[0].diagnostics.len(), base + 2);
    // An edit keeps the runtime diagnostics until their slot clears.
    let edited = an.open(u.clone(), 3, "s :sd > d1\n".to_string());
    assert_eq!(edited.diagnostics.len(), base + 2);
    let cleared = an.runtime_diags(&DiagBody {
        add: Vec::new(),
        clear: vec![
            WireClear { slot: "d1".into() },
            WireClear { slot: "d3".into() },
        ],
    });
    assert_eq!(cleared.len(), 1);
    assert_eq!(cleared[0].diagnostics.len(), base);
}

fn frame(body: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

/// Yields one queued chunk per read.
struct Chunks(std::collections::VecDeque<Vec<u8>>);

impl tokio::io::AsyncRead for Chunks {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        out: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if let Some(c) = self.0.pop_front() {
            out.put_slice(&c);
        }
        std::task::Poll::Ready(Ok(()))
    }
}

/// Reads to end of input through `ExitAwareStdin`.
fn read_all(parts: &[&[u8]]) -> Vec<u8> {
    use tokio::io::{AsyncRead, ReadBuf};
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let chunks = Chunks(parts.iter().map(|p| p.to_vec()).collect());
    let mut r = crate::lsp::server::ExitAwareStdin::new(chunks);
    rt.block_on(std::future::poll_fn(|cx| {
        let mut out = Vec::new();
        loop {
            let mut buf = [0u8; 256];
            let mut rb = ReadBuf::new(&mut buf);
            match std::pin::Pin::new(&mut r).poll_read(cx, &mut rb) {
                std::task::Poll::Ready(Ok(())) if rb.filled().is_empty() => {
                    return std::task::Poll::Ready(out);
                }
                std::task::Poll::Ready(Ok(())) => out.extend_from_slice(rb.filled()),
                other => panic!("{other:?}"),
            }
        }
    }))
}

#[test]
fn input_ends_right_after_the_exit_notification() {
    let shutdown = frame(r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#);
    let exit = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    let after = frame(r#"{"jsonrpc":"2.0","id":5,"method":"shutdown"}"#);
    let (e1, e2) = exit.split_at(10);
    let mut a = shutdown.clone();
    a.extend_from_slice(e1);
    let out = read_all(&[&a, e2, &after]);
    let mut want = a.clone();
    want.extend_from_slice(e2);
    assert_eq!(out, want, "nothing after `exit` is read");
    // A request whose params mention "exit" is not the notification.
    let not_exit = frame(r#"{"jsonrpc":"2.0","id":6,"method":"x","params":{"s":"exit"}}"#);
    assert_eq!(
        read_all(&[&not_exit, &after]).len(),
        not_exit.len() + after.len()
    );
}

#[test]
fn attach_subscribes_to_diagnostics_and_forwards_diag_bodies() {
    use crate::session::codec::{decode_as, encode};
    use crate::session::protocol::{ClientMsg, Envelope, ServerMsg};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let body = DiagBody {
        add: vec![wire("a.vact", "d1", 0)],
        clear: vec![WireClear { slot: "d2".into() }],
    };
    let sent = body.clone();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let mut ws = tungstenite::accept(stream).expect("handshake");
        let first = ws.read().expect("subscribe");
        let env = decode_as::<ClientMsg>(first.to_text().expect("text")).expect("decode");
        let ClientMsg::Subscribe(sub) = env.body else {
            panic!("subscribe expected");
        };
        assert!(sub.diagnostics && !sub.telemetry && !sub.levels);
        let msg = encode(&Envelope::new(1, None, ServerMsg::Diag(sent)));
        ws.send(tungstenite::Message::text(msg)).expect("send");
        let _ = ws.close(None);
        let _ = ws.flush();
    });
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    crate::lsp::server::attach(format!("ws://127.0.0.1:{port}/session?token=t"), tx)
        .expect("attach thread");
    assert_eq!(rx.blocking_recv(), Some(body));
    server.join().expect("server");
    // A failed connection ends the attach thread (one stderr line) and
    // closes the channel; the server keeps running standalone.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DiagBody>();
    let dead = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let dead_port = dead.local_addr().expect("addr").port();
    drop(dead);
    crate::lsp::server::attach(format!("ws://127.0.0.1:{dead_port}/"), tx).expect("thread");
    assert_eq!(rx.blocking_recv(), None);
}
