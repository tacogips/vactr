//! The browser package driver (design 15.1.2 G6, 15.1.10 required tests):
//! the `Need` sequence over `list` -> `manifest.toml` -> `zip` is
//! deterministic, a 404 surfaces as `package-resolve`, an unsafe zip entry
//! is rejected as `package-integrity` before the cache is touched, and a
//! `Restore` against a tampered body is `package-integrity` too.

use std::collections::BTreeMap;

use crate::pkg::driver::{drive, DriverReply, DriverRequest, Prefetched};
use crate::pkg::mem_cache::MemCache;
use crate::pkg::store::MANIFEST_FILE;
use crate::pkg::tests::support::{id, proxy_entries, v, write_zip, ZipSpec, PADS};

fn list_url() -> String {
    format!("/{PADS}/@v/list")
}

/// The full proxy fixture for one pinned version, in the order the driver
/// reaches them: the version list, the manifest, then the zip.
fn pinned_fixture(files: &[(&str, &[u8])]) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![(list_url(), b"v1.0.0\n".to_vec())];
    out.extend(proxy_entries(PADS, "v1.0.0", files));
    out
}

fn resolve_req() -> DriverRequest {
    DriverRequest::Resolve {
        proxy: String::new(),
        requirements: BTreeMap::from([(PADS.to_string(), "v1.0.0".to_string())]),
    }
}

/// Drives `req` to completion, supplying every `Need` from `fixture` (a
/// panic names any url the fixture has no body for); the urls asked, in
/// order, and the final reply.
fn drive_to_done(
    req: impl Fn() -> DriverRequest,
    fixture: &[(String, Vec<u8>)],
    supplied: &mut Prefetched,
    cache: &mut MemCache,
) -> (Vec<String>, DriverReply) {
    let mut needs = Vec::new();
    loop {
        match drive(req(), supplied, cache) {
            DriverReply::Need { url } => {
                let body = fixture
                    .iter()
                    .find(|(u, _)| *u == url)
                    .unwrap_or_else(|| panic!("the fixture has no body for `{url}`"))
                    .1
                    .clone();
                supplied.supply(url.clone(), body);
                needs.push(url);
            }
            done => return (needs, done),
        }
    }
}

#[test]
fn the_need_sequence_is_deterministic() {
    let files: &[(&str, &[u8])] = &[(MANIFEST_FILE, b"[deps]\n"), ("a.vact", b"let a 1\n")];
    let fixture = pinned_fixture(files);
    let mut cache = MemCache::new();
    let mut supplied = Prefetched::new();
    let (needs, done) = drive_to_done(resolve_req, &fixture, &mut supplied, &mut cache);
    let want: Vec<String> = fixture.iter().map(|(u, _)| u.clone()).collect();
    assert_eq!(needs, want, "list, then manifest, then zip, in that order");
    assert!(matches!(done, DriverReply::Done { .. }), "{done:?}");
    // A fresh run over the now-fully-supplied transport needs nothing more:
    // the sequence and its outcome are deterministic.
    let mut cache2 = MemCache::new();
    let redo = drive(resolve_req(), &mut supplied, &mut cache2);
    assert!(matches!(redo, DriverReply::Done { .. }), "{redo:?}");
}

#[test]
fn a_404_gives_package_resolve() {
    let mut cache = MemCache::new();
    let mut supplied = Prefetched::new();
    supplied.supply_status(list_url(), 404);
    let reply = drive(resolve_req(), &mut supplied, &mut cache);
    let DriverReply::Error { code, .. } = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(code, "package-resolve");
}

#[test]
fn a_traversal_entry_is_package_integrity_and_the_cache_is_untouched() {
    let zip = write_zip(
        &[
            ZipSpec::file(MANIFEST_FILE, b"[deps]\n"),
            ZipSpec::file("../evil.vact", b"let x 1\n"),
        ],
        0,
    );
    let fixture = vec![
        (list_url(), b"v1.0.0\n".to_vec()),
        (format!("/{PADS}/@v/v1.0.0.toml"), b"[deps]\n".to_vec()),
        (format!("/{PADS}/@v/v1.0.0.zip"), zip),
    ];
    let mut cache = MemCache::new();
    let mut supplied = Prefetched::new();
    for (url, body) in &fixture {
        supplied.supply(url.clone(), body.clone());
    }
    let reply = drive(resolve_req(), &mut supplied, &mut cache);
    let DriverReply::Error { code, message } = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(code, "package-integrity");
    assert!(message.contains("evil.vact"), "{message}");
    assert!(cache.is_staging_empty());
    assert!(!cache.is_published(&id(PADS), &v("v1.0.0")));
}

#[test]
fn restore_with_a_tampered_body_is_package_integrity() {
    let files: &[(&str, &[u8])] = &[(MANIFEST_FILE, b"[deps]\n"), ("a.vact", b"let a 1\n")];
    let fixture = pinned_fixture(files);
    let mut cache = MemCache::new();
    let mut supplied = Prefetched::new();
    let (_, done) = drive_to_done(resolve_req, &fixture, &mut supplied, &mut cache);
    let DriverReply::Done { lock, .. } = done else {
        panic!("{done:?}")
    };
    // A tampered zip: same manifest name, different (still valid) bytes, so
    // only the digest check can catch it.
    let tampered = write_zip(&[ZipSpec::file(MANIFEST_FILE, b"[deps]\n# tampered\n")], 0);
    let mut restore_supplied = Prefetched::new();
    restore_supplied.supply(format!("/{PADS}/@v/v1.0.0.zip"), tampered);
    let mut cache2 = MemCache::new();
    let reply = drive(
        DriverRequest::Restore {
            proxy: String::new(),
            lock,
        },
        &mut restore_supplied,
        &mut cache2,
    );
    let DriverReply::Error { code, .. } = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(code, "package-integrity");
}
