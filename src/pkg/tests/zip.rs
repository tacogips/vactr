//! The zip reader: stored and deflate entries, and correctly formed but
//! malicious archives, each rejected naming the entry. Nothing panics.

use crate::pkg::tests::support::{write_zip, zip_of, ZipSpec};
use crate::pkg::validate::{IntegrityError, Limits};
use crate::pkg::zip::read_zip;

fn rejects(zip: &[u8], limits: &Limits, entry: &str) -> IntegrityError {
    let e = read_zip(zip, limits).expect_err(entry);
    assert_eq!(e.entry, entry, "{e}");
    e
}

#[test]
fn reads_stored_and_deflate_entries() {
    let big = vec![b'x'; 5000];
    let zip = write_zip(
        &[
            ZipSpec::dir("samples"),
            ZipSpec::file("pads.vact", b"let warm 3\n"),
            ZipSpec::file("samples/big.txt", &big).deflated(),
            ZipSpec::file("empty", b""),
        ],
        0,
    );
    let got = read_zip(&zip, &Limits::default()).expect("reads");
    assert_eq!(
        got,
        vec![
            ("pads.vact".to_string(), b"let warm 3\n".to_vec()),
            ("samples/big.txt".to_string(), big),
            ("empty".to_string(), Vec::new()),
        ]
    );
}

#[test]
fn malicious_archives_are_rejected_naming_the_entry() {
    let l = Limits::default();
    let ok = ZipSpec::file("pads.vact", b"let warm 3\n");
    let with = |bad: ZipSpec| write_zip(&[ok.clone(), bad], 0);
    rejects(
        &with(ZipSpec::file("../evil.vact", b"x")),
        &l,
        "../evil.vact",
    );
    rejects(&with(ZipSpec::file("/etc/passwd", b"x")), &l, "/etc/passwd");
    rejects(
        &with(ZipSpec::symlink("samples", "../../outside")),
        &l,
        "samples",
    );
    rejects(&with(ZipSpec::file("pads.vact", b"again")), &l, "pads.vact");
    rejects(&with(ZipSpec::file("PADS.vact", b"x")), &l, "PADS.vact");
    let mut dev = ZipSpec::file("fifo", b"");
    dev.mode = Some(0o010_644);
    rejects(&with(dev), &l, "fifo");
    let small = Limits {
        max_entries: 2,
        max_bytes: 1024,
    };
    let three = write_zip(
        &[
            ok.clone(),
            ZipSpec::file("a", b"1"),
            ZipSpec::file("b", b"2"),
        ],
        0,
    );
    rejects(&three, &small, "<archive>");
    rejects(
        &with(ZipSpec::file("big", &[0u8; 2048]).deflated()),
        &small,
        "big",
    );
}

#[test]
fn bombs_sizes_methods_and_formats_are_refused() {
    let l = Limits::default();
    // A deflate bomb declared small: inflation stops at the declared size.
    let mut bomb = ZipSpec::file("bomb", &vec![0u8; 1 << 20]).deflated();
    bomb.declared = Some(1000);
    let e = rejects(&write_zip(&[bomb], 0), &l, "bomb");
    assert!(e.reason.contains("larger than declared"), "{e}");
    // Declared larger than the real data.
    let mut short = ZipSpec::file("short", b"abc").deflated();
    short.declared = Some(10);
    rejects(&write_zip(&[short], 0), &l, "short");
    let mut stored = ZipSpec::file("stored", b"abc");
    stored.declared = Some(10);
    rejects(&write_zip(&[stored], 0), &l, "stored");
    // Encryption and methods other than stored/deflate.
    let mut enc = ZipSpec::file("secret", b"abc");
    enc.flags = 1;
    rejects(&write_zip(&[enc], 0), &l, "secret");
    let mut bz = ZipSpec::file("bz", b"abc");
    bz.method = Some(12);
    rejects(&write_zip(&[bz], 0), &l, "bz");
    // Zip64 sizes and multi-disk archives.
    let mut z64 = ZipSpec::file("z64", b"abc");
    z64.declared = Some(u32::MAX);
    rejects(&write_zip(&[z64], 0), &l, "z64");
    rejects(&write_zip(&[ZipSpec::file("a", b"1")], 1), &l, "<archive>");
    // A name that is not UTF-8.
    let mut raw = ZipSpec::file("x", b"1");
    raw.name = vec![b'a', 0xff];
    rejects(&write_zip(&[raw], 0), &l, "a\u{fffd}");
}

#[test]
fn garbage_and_truncation_are_errors_not_panics() {
    let l = Limits::default();
    let good = zip_of(&[("a.vact", b"let a 1\n"), ("b.vact", b"let b 2\n")]);
    for cut in 0..good.len() {
        let _ = read_zip(&good[..cut], &l);
    }
    let mut flipped = good.clone();
    for i in 0..flipped.len() {
        flipped[i] ^= 0xff;
        let _ = read_zip(&flipped, &l);
        flipped[i] ^= 0xff;
    }
    assert!(read_zip(b"PK\x05\x06", &l).is_err());
    assert!(read_zip(&[0u8; 100], &l).is_err());
    assert_eq!(read_zip(&good, &l).expect("intact").len(), 2);
}
