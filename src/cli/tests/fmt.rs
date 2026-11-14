//! Behavioral tests for the `fmt` CLI adapter.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cli::args::FmtInput;
use crate::cli::fmt;
use crate::reader::import::AliasEnv;
use crate::reader::span::FileId;
use crate::types::diag::Severity;

const DEEP_CONTINUATION: &str = "s :bd\n\t\t\t> d1\n";
const FORMATTED: &str = "s :bd\n\t> d1\n";

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("vactr-fmt-cli-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temporary test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).expect("write test file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_reader_clean(source: &str) {
    let read = crate::reader::read(source, FileId::new(1), &AliasEnv::new());
    assert!(
        read.diags
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "sample must be reader-clean: {:?}",
        read.diags
    );
}

fn invoke(check: bool, input: FmtInput, cwd: &Path, stdin: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut stdin = Cursor::new(stdin.to_vec());
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = fmt::run(check, &input, cwd, &mut stdin, &mut stdout, &mut stderr);
    (status, stdout, stderr)
}

#[test]
fn changed_file_is_rewritten_and_check_only_reports_the_path() {
    assert_reader_clean(DEEP_CONTINUATION);
    assert_reader_clean(FORMATTED);
    let dir = TempDir::new();
    let file = dir.write("sample.vact", DEEP_CONTINUATION.as_bytes());

    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("sample.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(
        std::fs::read(&file).expect("read formatted file"),
        FORMATTED.as_bytes()
    );

    std::fs::write(&file, DEEP_CONTINUATION).expect("restore unformatted input");
    let (status, stdout, stderr) = invoke(
        true,
        FmtInput::Paths(vec![PathBuf::from("sample.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"sample.vact\n");
    assert!(stderr.is_empty());
    assert_eq!(
        std::fs::read(&file).expect("read unchanged file"),
        DEEP_CONTINUATION.as_bytes()
    );
}

#[test]
fn unchanged_file_is_not_written_in_either_mode() {
    assert_reader_clean(FORMATTED);
    let dir = TempDir::new();
    let file = dir.write("stable.vact", FORMATTED.as_bytes());
    let before = std::fs::metadata(&file)
        .expect("metadata before fmt")
        .modified()
        .expect("mtime before fmt");

    for check in [false, true] {
        let (status, stdout, stderr) = invoke(
            check,
            FmtInput::Paths(vec![PathBuf::from("stable.vact")]),
            dir.path(),
            b"",
        );
        assert_eq!(status, 0);
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        assert_eq!(
            std::fs::read(&file).expect("read stable file"),
            FORMATTED.as_bytes()
        );
        assert_eq!(
            std::fs::metadata(&file)
                .expect("metadata after fmt")
                .modified()
                .expect("mtime after fmt"),
            before
        );
    }
}

#[test]
fn reader_error_refuses_file_and_reports_diagnostics() {
    let dir = TempDir::new();
    let invalid = b"s \"abc\n";
    let file = dir.write("invalid.vact", invalid);

    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("invalid.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 3);
    assert!(stdout.is_empty());
    let diagnostic = String::from_utf8(stderr).expect("diagnostic UTF-8");
    assert!(diagnostic.contains("vactr: invalid.vact:"), "{diagnostic}");
    assert!(diagnostic.contains(":1:"), "{diagnostic}");
    assert!(
        diagnostic.contains("error[unterminated-string]"),
        "{diagnostic}"
    );
    assert_eq!(std::fs::read(&file).expect("read refused file"), invalid);
}

#[test]
fn stdin_formats_checks_and_preserves_refused_input() {
    assert_reader_clean(DEEP_CONTINUATION);
    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Stdin,
        Path::new("."),
        DEEP_CONTINUATION.as_bytes(),
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, FORMATTED.as_bytes());
    assert!(stderr.is_empty());

    let (status, stdout, stderr) = invoke(
        true,
        FmtInput::Stdin,
        Path::new("."),
        DEEP_CONTINUATION.as_bytes(),
    );
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());

    let refused = b"s \"abc\n";
    let (status, stdout, stderr) = invoke(false, FmtInput::Stdin, Path::new("."), refused);
    assert_eq!(status, 3);
    assert_eq!(stdout, refused);
    assert!(String::from_utf8(stderr)
        .expect("diagnostic UTF-8")
        .contains("vactr: -:"));
}

#[test]
fn io_error_does_not_prevent_later_files_from_being_formatted() {
    assert_reader_clean(DEEP_CONTINUATION);
    let dir = TempDir::new();
    let good = dir.write("good.vact", DEEP_CONTINUATION.as_bytes());

    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![
            PathBuf::from("missing.vact"),
            PathBuf::from("good.vact"),
        ]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .expect("diagnostic UTF-8")
        .contains("vactr: missing.vact:"));
    assert_eq!(
        std::fs::read(good).expect("read formatted good file"),
        FORMATTED.as_bytes()
    );
}

#[test]
fn directory_and_invalid_utf8_are_io_errors() {
    let dir = TempDir::new();
    std::fs::create_dir(dir.path().join("nested")).expect("create nested directory");
    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("nested")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .expect("diagnostic UTF-8")
        .contains("vactr: nested:"));

    let invalid = [0xff, 0xfe];
    let file = dir.write("binary.vact", &invalid);
    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("binary.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 1);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .expect("diagnostic UTF-8")
        .contains("vactr: binary.vact: not UTF-8"));
    assert_eq!(std::fs::read(file).expect("read binary input"), invalid);
}

#[test]
fn four_space_file_is_rewritten_and_stable_on_the_second_run() {
    let input = include_bytes!("../../fmt/tests/fixtures/space4.in");
    let expected = include_bytes!("../../fmt/tests/fixtures/space4.out");
    let dir = TempDir::new();
    let file = dir.write("sample.vact", input);

    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("sample.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(std::fs::read(&file).expect("read repaired file"), expected);

    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("sample.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(std::fs::read(&file).expect("read stable file"), expected);
}

#[test]
fn check_mode_reports_space_file_without_rewriting_it() {
    let input = include_bytes!("../../fmt/tests/fixtures/space4.in");
    let dir = TempDir::new();
    let file = dir.write("sample.vact", input);

    let (status, stdout, stderr) = invoke(
        true,
        FmtInput::Paths(vec![PathBuf::from("sample.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"sample.vact\n");
    assert!(stderr.is_empty());
    assert_eq!(std::fs::read(&file).expect("read unchanged file"), input);
}

#[test]
fn ambiguous_space_file_is_refused_unchanged() {
    let input = include_bytes!("../../fmt/tests/fixtures/space-ambiguous.in");
    let dir = TempDir::new();
    let file = dir.write("ambiguous.vact", input);

    let (status, stdout, stderr) = invoke(
        false,
        FmtInput::Paths(vec![PathBuf::from("ambiguous.vact")]),
        dir.path(),
        b"",
    );
    assert_eq!(status, 3);
    assert!(stdout.is_empty());
    let diagnostic = String::from_utf8(stderr).expect("diagnostic UTF-8");
    assert!(
        diagnostic.contains("vactr: ambiguous.vact:"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("error[indent-space]"), "{diagnostic}");
    assert_eq!(
        std::fs::read(&file).expect("read refused file"),
        input,
        "refused input must remain byte-identical"
    );
}
