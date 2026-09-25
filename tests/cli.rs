//! Integration tests for the `vactrol` binary (design 14.5.10,
//! `command.md`): every verb runs against `--host noop`, with every fixture
//! built in a temp directory at runtime (nothing on disk beyond this file)
//! and no network access. Every spawned child is waited on or killed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// A fresh, empty directory under `std::env::temp_dir()`, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "vactrol-cli-test-{tag}-{}-{nanos}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create a temp directory");
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A child process killed and waited on when it goes out of scope, even
/// on a panicking test.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn vactrol(cwd: &Path, home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vactrol"));
    cmd.current_dir(cwd);
    cmd.env("VACTROL_HOME", home);
    cmd
}

#[test]
fn version_exits_zero_and_prints_the_version() {
    let home = TempDir::new("version-home");
    let cwd = TempDir::new("version-cwd");
    let out = vactrol(cwd.path(), home.path())
        .arg("--version")
        .output()
        .expect("run `vactrol --version`");
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")), "{stdout}");
}

#[test]
fn run_with_noop_host_and_cycles_exits_zero() {
    let home = TempDir::new("run-ok-home");
    let cwd = TempDir::new("run-ok-cwd");
    std::fs::write(cwd.join("a.vact"), "s [:bd :sd] > d1\n").expect("write a.vact");
    let out = vactrol(cwd.path(), home.path())
        .args(["run", "a.vact", "--host", "noop", "--cycles", "2"])
        .output()
        .expect("run `vactrol run`");
    assert!(out.status.success(), "{out:?}");
}

#[test]
fn run_with_a_type_error_exits_three_and_prints_the_code() {
    let home = TempDir::new("run-err-home");
    let cwd = TempDir::new("run-err-cwd");
    std::fs::write(cwd.join("b.vact"), "+ 1 \"a\"\n").expect("write b.vact");
    let out = vactrol(cwd.path(), home.path())
        .args(["run", "b.vact", "--host", "noop", "--cycles", "1"])
        .output()
        .expect("run `vactrol run` (type error)");
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(combined.contains("error["), "{combined}");
}

#[test]
fn repl_with_noop_host_evaluates_piped_stdin() {
    let home = TempDir::new("repl-home");
    let cwd = TempDir::new("repl-cwd");
    let mut child = vactrol(cwd.path(), home.path())
        .args(["repl", "--host", "noop"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `vactrol repl`");
    {
        let stdin = child.stdin.as_mut().expect("repl stdin");
        stdin
            .write_all(b"let a 1\n+ a 1\n/ 1 0\n_2\n")
            .expect("write to repl stdin");
    }
    let out = child.wait_with_output().expect("wait for repl to exit");
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let p1 = stdout
        .find("_1 = 1")
        .unwrap_or_else(|| panic!("missing `_1 = 1`: {stdout}"));
    let p2 = stdout
        .find("_2 = 2")
        .unwrap_or_else(|| panic!("missing `_2 = 2`: {stdout}"));
    let perr = stdout
        .find("error[division-by-zero]")
        .unwrap_or_else(|| panic!("missing `error[division-by-zero]`: {stdout}"));
    let p3 = stdout
        .find("_3 = 2")
        .unwrap_or_else(|| panic!("missing `_3 = 2`: {stdout}"));
    assert!(p1 < p2 && p2 < perr && perr < p3, "{stdout}");
    assert!(!stdout.contains("_4"), "{stdout}");
}

/// Writes `<store>/<path>@<version>/` with a minimal manifest and one
/// source file.
fn write_pkg_fixture(store: &Path, path: &str, version: &str, body: &str) {
    let dir = store.join(format!("{path}@{version}"));
    std::fs::create_dir_all(&dir).expect("mkdir the fixture package");
    std::fs::write(
        dir.join("vactrol.toml"),
        format!("[package]\npath = \"{path}\"\n\n[deps]\n"),
    )
    .expect("write the fixture vactrol.toml");
    std::fs::write(dir.join("pads.vact"), body).expect("write the fixture source");
}

const PADS: &str = "github.com/test/vactrol-pads";

fn lock_line_prefix(path: &str, version: &str) -> String {
    format!("{path} {version} sha256:")
}

fn lock_has_valid_entry(lock: &str, path: &str, version: &str) -> bool {
    let prefix = lock_line_prefix(path, version);
    lock.lines().any(|l| {
        l.strip_prefix(&prefix)
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
    })
}

#[test]
fn get_with_dir_store_writes_manifest_and_lock_then_raises_the_version() {
    let home = TempDir::new("get-home");
    let cwd = TempDir::new("get-cwd");
    let store = TempDir::new("get-store");
    write_pkg_fixture(store.path(), PADS, "v1.0.0", "let warm 3\n");
    write_pkg_fixture(store.path(), PADS, "v1.1.0", "let warm 3\n");
    let store_arg = format!("dir:{}", store.path().display());

    let out = vactrol(cwd.path(), home.path())
        .args(["get", &format!("{PADS}@v1.0.0"), "--store", &store_arg])
        .output()
        .expect("run `vactrol get` v1.0.0");
    assert!(out.status.success(), "{out:?}");

    let toml = std::fs::read_to_string(cwd.join("vactrol.toml")).expect("read vactrol.toml");
    assert!(toml.contains(PADS) && toml.contains("v1.0.0"), "{toml}");

    let lock = std::fs::read_to_string(cwd.join("vactrol.lock")).expect("read vactrol.lock");
    assert!(lock.starts_with("# vactrol.lock v1"), "{lock}");
    assert!(lock_has_valid_entry(&lock, PADS, "v1.0.0"), "{lock}");

    let out2 = vactrol(cwd.path(), home.path())
        .args(["get", &format!("{PADS}@v1.1.0"), "--store", &store_arg])
        .output()
        .expect("run `vactrol get` v1.1.0");
    assert!(out2.status.success(), "{out2:?}");

    let toml2 = std::fs::read_to_string(cwd.join("vactrol.toml")).expect("read vactrol.toml (2)");
    assert!(
        toml2.contains("v1.1.0") && !toml2.contains("v1.0.0"),
        "{toml2}"
    );
    let lock2 = std::fs::read_to_string(cwd.join("vactrol.lock")).expect("read vactrol.lock (2)");
    assert!(lock_has_valid_entry(&lock2, PADS, "v1.1.0"), "{lock2}");
}

#[test]
fn get_without_a_version_picks_the_latest_release() {
    let home = TempDir::new("get-latest-home");
    let cwd = TempDir::new("get-latest-cwd");
    let store = TempDir::new("get-latest-store");
    write_pkg_fixture(store.path(), PADS, "v1.0.0", "let warm 3\n");
    write_pkg_fixture(store.path(), PADS, "v1.1.0", "let warm 3\n");
    let store_arg = format!("dir:{}", store.path().display());

    let out = vactrol(cwd.path(), home.path())
        .args(["get", PADS, "--store", &store_arg])
        .output()
        .expect("run `vactrol get` with no version");
    assert!(out.status.success(), "{out:?}");
    let lock = std::fs::read_to_string(cwd.join("vactrol.lock")).expect("read vactrol.lock");
    assert!(lock_has_valid_entry(&lock, PADS, "v1.1.0"), "{lock}");
}

#[cfg(unix)]
#[test]
fn get_with_a_corrupted_fixture_exits_one_with_package_integrity() {
    let home = TempDir::new("get-bad-home");
    let cwd = TempDir::new("get-bad-cwd");
    let store = TempDir::new("get-bad-store");
    let bad_path = "github.com/test/vactrol-bad";
    let dir = store.join(&format!("{bad_path}@v1.0.0"));
    std::fs::create_dir_all(&dir).expect("mkdir the bad fixture");
    std::fs::write(
        dir.join("vactrol.toml"),
        format!("[package]\npath = \"{bad_path}\"\n\n[deps]\n"),
    )
    .expect("write the bad fixture vactrol.toml");
    std::os::unix::fs::symlink("../../..", dir.join("evil")).expect("create the traversal symlink");
    let store_arg = format!("dir:{}", store.path().display());

    let out = vactrol(cwd.path(), home.path())
        .args(["get", &format!("{bad_path}@v1.0.0"), "--store", &store_arg])
        .output()
        .expect("run `vactrol get` on a corrupted fixture");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("package-integrity"), "{stderr}");
}

#[cfg(not(feature = "lsp"))]
#[test]
fn lsp_without_the_feature_exits_one() {
    let home = TempDir::new("lsp-home");
    let cwd = TempDir::new("lsp-cwd");
    let out = vactrol(cwd.path(), home.path())
        .arg("lsp")
        .output()
        .expect("run `vactrol lsp`");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("vactrol was built without the lsp feature"),
        "{stderr}"
    );
}

#[cfg(feature = "host-native")]
mod serve_tests {
    use super::{KillOnDrop, TempDir};
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    type WsClient =
        tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

    fn set_timeout(ws: &WsClient) {
        if let tungstenite::stream::MaybeTlsStream::Plain(tcp) = ws.get_ref() {
            let _ = tcp.set_read_timeout(Some(Duration::from_millis(200)));
        }
    }

    /// Reads text frames until `pred` matches one, or `timeout` elapses.
    fn read_until(ws: &mut WsClient, timeout: Duration, pred: impl Fn(&str) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match ws.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    if pred(&text) {
                        return true;
                    }
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => {}
            }
        }
        false
    }

    fn expect_401(url: &str, why: &str) {
        match tungstenite::connect(url) {
            Ok(_) => panic!("{why}: the handshake unexpectedly succeeded"),
            Err(tungstenite::Error::Http(resp)) => {
                assert_eq!(resp.status(), 401, "{why}: got {:?}", resp.status());
            }
            Err(other) => panic!("{why}: expected an HTTP handshake error, got {other:?}"),
        }
    }

    #[test]
    fn serve_with_noop_host_round_trips_over_the_session_socket() {
        let home = TempDir::new("serve-home");
        let cwd = TempDir::new("serve-cwd");
        let child = super::vactrol(cwd.path(), home.path())
            .args(["serve", "--host", "noop", "--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn `vactrol serve`");
        let mut guard = KillOnDrop(child);

        let stderr = guard.0.stderr.take().expect("serve stderr");
        let mut lines = BufReader::new(stderr).lines();
        let url = loop {
            let line = lines
                .next()
                .expect("serve stderr closed before printing a url")
                .expect("read a line of serve stderr");
            if line.starts_with("ws://") {
                break line;
            }
        };

        let (mut ws, _resp) = tungstenite::connect(&url).expect("connect to the session socket");
        set_timeout(&ws);

        ws.send(tungstenite::Message::Text(
            r#"{"v":1,"seq":1,"kind":"subscribe","body":{"telemetry":false,"levels":false,"diagnostics":true}}"#
                .into(),
        ))
        .expect("send subscribe");
        ws.send(tungstenite::Message::Text(
            r#"{"v":1,"seq":2,"kind":"eval","body":{"file":"a.vact","code":"+ 1 2","doc_revision":1,"edit_epoch":1}}"#
                .into(),
        ))
        .expect("send eval");

        let got_result = read_until(&mut ws, Duration::from_secs(10), |text| {
            text.contains("\"kind\":\"eval-result\"") && text.contains("\"re\":2")
        });
        assert!(got_result, "never received an eval-result for seq 2");

        ws.send(tungstenite::Message::Text("not json".into()))
            .expect("send garbage");
        let got_error = read_until(&mut ws, Duration::from_secs(10), |text| {
            text.contains("\"kind\":\"protocol-error\"") && text.contains("bad-json")
        });
        assert!(
            got_error,
            "never received a protocol-error for garbage input"
        );

        let token_at = url.find("token=").expect("the url carries a token");
        let wrong_token_url = format!(
            "{}token=0000000000000000000000000000000000000000000000000000000000000000",
            &url[..token_at]
        );
        expect_401(&wrong_token_url, "a wrong token");

        let wrong_path_url = url.replacen("/session?", "/nope?", 1);
        expect_401(&wrong_path_url, "a wrong path");
    }
}
