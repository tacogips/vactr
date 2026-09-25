//! `vactrol.lock`: header, ordering, round trips and rejections.

use crate::pkg::digest::hex;
use crate::pkg::lock::{LockEntry, LockFile, LOCK_HEADER};
use crate::pkg::tests::support::{id, v, DRUMS, PADS};

fn entry(path: &str, ver: &str, b: u8) -> LockEntry {
    LockEntry {
        id: id(path),
        version: v(ver),
        sha256: [b; 32],
    }
}

#[test]
fn renders_sorted_with_the_header_and_round_trips() {
    let lock = LockFile::new(vec![
        entry(PADS, "v1.1.0", 0xab),
        entry(DRUMS, "v0.2.0-rc.1", 1),
    ]);
    let text = lock.render();
    let expect = format!(
        "{LOCK_HEADER}\n{DRUMS} v0.2.0-rc.1 sha256:{}\n{PADS} v1.1.0 sha256:{}\n",
        hex(&[1; 32]),
        hex(&[0xab; 32])
    );
    assert_eq!(text, expect);
    let back = LockFile::parse(&text).expect("parses");
    assert_eq!(back, lock);
    assert_eq!(
        back.get(&id(PADS)).map(|e| e.version.clone()),
        Some(v("v1.1.0"))
    );
    assert_eq!(back.get(&id("github.com/x/y")), None);
    assert_eq!(
        LockFile::parse(LOCK_HEADER).expect("empty lock"),
        LockFile::default()
    );
}

#[test]
fn rejects_unknown_headers_and_malformed_lines() {
    let good = format!("{PADS} v1.1.0 sha256:{}", hex(&[7; 32]));
    let cases: Vec<(String, u32)> = vec![
        (format!("# vactrol.lock v2\n{good}\n"), 1),
        (format!("{good}\n"), 1),
        (format!("{LOCK_HEADER}\n{PADS} v1.1.0\n"), 2),
        (
            format!("{LOCK_HEADER}\n{PADS}  v1.1.0 sha256:{}\n", hex(&[7; 32])),
            2,
        ),
        (
            format!("{LOCK_HEADER}\n{PADS} 1.1.0 sha256:{}\n", hex(&[7; 32])),
            2,
        ),
        (
            format!(
                "{LOCK_HEADER}\n{PADS} v1.1.0 sha256:{}\n",
                hex(&[0xab; 32]).to_uppercase()
            ),
            2,
        ),
        (
            format!("{LOCK_HEADER}\n{PADS} v1.1.0 md5:{}\n", hex(&[7; 32])),
            2,
        ),
        (format!("{LOCK_HEADER}\n{PADS} v1.1.0 sha256:abc\n"), 2),
        (
            format!(
                "{LOCK_HEADER}\ngithub.com/X/y v1.1.0 sha256:{}\n",
                hex(&[7; 32])
            ),
            2,
        ),
        (format!("{LOCK_HEADER}\n{good}\n{good}\n"), 3),
        (
            format!(
                "{LOCK_HEADER}\n{good}\n{DRUMS} v1.0.0 sha256:{}\n",
                hex(&[1; 32])
            ),
            3,
        ),
        (format!("{LOCK_HEADER}\n\n{good}\n"), 2),
    ];
    for (text, line) in cases {
        let e = LockFile::parse(&text).expect_err(&text);
        assert_eq!(e.line, line, "{text:?}: {e}");
    }
}
