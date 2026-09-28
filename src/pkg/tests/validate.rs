//! Entry validation: paths, kinds, duplicates, caps and assets, each
//! rejected with `package-integrity` naming the entry.

use std::rc::Rc;

use crate::pkg::store::PkgError;
use crate::pkg::validate::{
    check_path, validate_assets, validate_entries, EntryKind, IntegrityError, Limits,
};
use crate::types::diag::DiagCode;

fn f(p: &str) -> (String, EntryKind, u64) {
    (p.to_string(), EntryKind::File, 1)
}

fn rejects(entries: &[(String, EntryKind, u64)], limits: &Limits, entry: &str) -> IntegrityError {
    let e = validate_entries(entries, limits).expect_err(entry);
    assert_eq!(e.entry, entry, "{e}");
    assert_eq!(PkgError::from(e.clone()).code(), DiagCode::PackageIntegrity);
    e
}

#[test]
fn paths() {
    for ok in [
        "a",
        "a/b.vact",
        "samples/warm/1.wav",
        "a b/c-d_e.f",
        "日本/x",
    ] {
        assert_eq!(check_path(ok), Ok(()), "{ok}");
    }
    for bad in [
        "",
        "/etc/passwd",
        "../x",
        "a/../../x",
        "a/./b",
        "a//b",
        "a/",
        "C:/x",
        "c:x",
        "a\\b",
        "a\nb",
        "a\u{7f}b",
        "a\tb",
        "\u{1}",
    ] {
        assert!(check_path(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn malicious_entries_are_rejected_naming_the_entry() {
    let l = Limits::default();
    let ok = [
        f("vactr.toml"),
        f("pads.vact"),
        (String::from("samples"), EntryKind::Dir, 0),
    ];
    assert_eq!(validate_entries(&ok, &l), Ok(()));
    rejects(&[f("pads.vact"), f("../evil.vact")], &l, "../evil.vact");
    rejects(&[f("/etc/passwd")], &l, "/etc/passwd");
    rejects(
        &[(String::from("samples"), EntryKind::Symlink, 0)],
        &l,
        "samples",
    );
    rejects(&[(String::from("dev"), EntryKind::Other, 0)], &l, "dev");
    rejects(&[f("a.vact"), f("b.vact"), f("a.vact")], &l, "a.vact");
    let e = rejects(&[f("warm.vact"), f("Warm.vact")], &l, "Warm.vact");
    assert!(e.reason.contains("case folding"), "{e}");
    rejects(&[f("ÄRGER.vact"), f("ärger.vact")], &l, "ärger.vact");
    rejects(&[f("a"), f("a/b")], &l, "a/b");
}

#[test]
fn caps_are_enforced() {
    let small = Limits {
        max_entries: 2,
        max_bytes: 10,
    };
    rejects(&[f("a"), f("b"), f("c")], &small, "c");
    let big = [f("a"), (String::from("b"), EntryKind::File, 10)];
    let e = rejects(&big, &small, "b");
    assert!(e.reason.contains("bytes"), "{e}");
    assert_eq!(Limits::default().max_entries, 10_000);
    assert_eq!(Limits::default().max_bytes, 256 * 1024 * 1024);
}

#[test]
fn assets_normalize_strictly_under_the_root() {
    let entries = [f("samples/warm/a.wav"), f("readme")];
    let got = validate_assets(&entries, &[Rc::from("./samples/"), Rc::from("wt")]).expect("ok");
    let got: Vec<&str> = got.iter().map(|a| &**a).collect();
    assert_eq!(got, ["samples", "wt"]);
    for bad in ["../outside", "/abs", ".", "./", "a/../..", "readme"] {
        let e = validate_assets(&entries, &[Rc::from(bad)]).expect_err(bad);
        assert_eq!(e.entry, bad);
    }
}
