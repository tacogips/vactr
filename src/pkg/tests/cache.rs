//! Staged atomic publication into `FsCache`: verified reads, malicious
//! fixtures that leave the cache byte-identical, interrupted extraction,
//! stamp and lock mismatches.

use std::fs;

use crate::ns::pkg::PackageId;
use crate::pkg::cache::{fetch_and_publish, get_all, CacheBackend, StagingId};
use crate::pkg::digest::{hex, tree_digest};
use crate::pkg::load::locked_sources;
use crate::pkg::manifest::parse;
use crate::pkg::native::fs_cache::STAGING_DIR;
use crate::pkg::native::{DirStore, FsCache};
use crate::pkg::proxy::ProxyStore;
use crate::pkg::semver::Version;
use crate::pkg::store::{PackageStore, PkgError, PkgSources, MANIFEST_FILE, STAMP_FILE};
use crate::pkg::tests::support::{
    as_refs, deps_manifest, dir_package, hashed, id, pads_files, snapshot, v, write_zip,
    MapTransport, MemStore, TempDir, ZipSpec, DRUMS, PADS,
};
use crate::pkg::validate::Limits;
use crate::types::diag::DiagCode;

/// A cache whose `write` fails after `left` successful writes.
struct FailAfter {
    inner: FsCache,
    left: usize,
}

impl CacheBackend for FailAfter {
    fn create_staging(&mut self) -> Result<StagingId, PkgError> {
        self.inner.create_staging()
    }
    fn write(&mut self, s: StagingId, path: &str, bytes: &[u8]) -> Result<(), PkgError> {
        if self.left == 0 {
            return Err(PkgError::Io("injected failure".into()));
        }
        self.left -= 1;
        self.inner.write(s, path, bytes)
    }
    fn remove_staging(&mut self, s: StagingId) {
        self.inner.remove_staging(s);
    }
    fn publish(
        &mut self,
        s: StagingId,
        pkg: &PackageId,
        ver: &Version,
        stamp: [u8; 32],
    ) -> Result<(), PkgError> {
        self.inner.publish(s, pkg, ver, stamp)
    }
    fn read_verified(
        &self,
        pkg: &PackageId,
        ver: &Version,
        expect: [u8; 32],
    ) -> Result<PkgSources, PkgError> {
        self.inner.read_verified(pkg, ver, expect)
    }
}

fn staging_is_empty(cache: &FsCache) -> bool {
    fs::read_dir(cache.root().join(STAGING_DIR))
        .expect("the staging dir")
        .next()
        .is_none()
}

fn pads_store(tmp: &TempDir) -> DirStore {
    let files = pads_files();
    dir_package(&tmp.join("store"), PADS, "v1.0.0", &as_refs(&files));
    DirStore::new(&tmp.join("store"))
}

#[test]
fn publishes_a_verified_entry_and_reads_it_back() {
    let tmp = TempDir::new();
    let mut store = pads_store(&tmp);
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let d = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.0.0"), None).expect("ok");
    let files = pads_files();
    assert_eq!(d, tree_digest(&hashed(&as_refs(&files))));
    let entry = cache.entry(&id(PADS), &v("v1.0.0"));
    assert!(entry.ends_with("github.com/someone/vactr-pads@v1.0.0"));
    let stamp = fs::read_to_string(entry.join(STAMP_FILE)).expect("stamp");
    assert_eq!(stamp, format!("{}\n", hex(&d)));
    assert!(staging_is_empty(&cache));
    let src = cache
        .read_verified(&id(PADS), &v("v1.0.0"), d)
        .expect("verified");
    let paths: Vec<&str> = src.files.iter().map(|(p, _)| &**p).collect();
    assert_eq!(
        paths,
        [
            "pads.vact",
            "samples/README",
            "samples/warm/a.wav",
            "samples/warm/b.wav",
            MANIFEST_FILE
        ]
    );
    assert_eq!(src.manifest.package.expect("meta").path, id(PADS));
    assert_eq!(src.root.as_deref(), entry.to_str());
    // Publishing the same content again is idempotent.
    let again = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.0.0"), None);
    assert_eq!(again, Ok(d));
    assert!(staging_is_empty(&cache));
}

/// The malicious fixtures, as proxy zips: `(label, entries, named entry,
/// store limits)`.
fn malicious() -> Vec<(&'static str, Vec<ZipSpec>, &'static str, Limits)> {
    let manifest = format!("[package]\npath = \"{PADS}\"\nassets = [\"samples\"]\n");
    let base = || {
        vec![
            ZipSpec::file(MANIFEST_FILE, manifest.as_bytes()),
            ZipSpec::file("pads.vact", b"let warm 3\n"),
        ]
    };
    let plus = |extra: ZipSpec| {
        let mut b = base();
        b.push(extra);
        b
    };
    let small = Limits {
        max_entries: 10_000,
        max_bytes: 1024,
    };
    vec![
        (
            "traversal",
            plus(ZipSpec::file("../evil.vact", b"x")),
            "../evil.vact",
            Limits::default(),
        ),
        (
            "absolute",
            plus(ZipSpec::file("/etc/passwd", b"x")),
            "/etc/passwd",
            Limits::default(),
        ),
        (
            "symlink asset",
            plus(ZipSpec::symlink("samples", "../../../outside")),
            "samples",
            Limits::default(),
        ),
        (
            "duplicate",
            plus(ZipSpec::file("pads.vact", b"again")),
            "pads.vact",
            Limits::default(),
        ),
        (
            "case fold",
            plus(ZipSpec::file("Pads.vact", b"x")),
            "Pads.vact",
            Limits::default(),
        ),
        (
            "over limit",
            plus(ZipSpec::file("big.bin", &[0; 4096]).deflated()),
            "big.bin",
            small,
        ),
    ]
}

#[test]
fn each_malicious_fixture_is_rejected_and_leaves_the_cache_unchanged() {
    let tmp = TempDir::new();
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    // A cache that already holds an entry.
    let mut good = MemStore::default();
    good.add(
        DRUMS,
        "v0.2.0",
        vec![("drums.vact".into(), b"let kick 1\n".to_vec())],
    );
    fetch_and_publish(&mut good, &mut cache, &id(DRUMS), &v("v0.2.0"), None).expect("drums");
    let before = snapshot(cache.root());
    for (label, specs, entry, limits) in malicious() {
        // The "correct" hash of the malicious tree: matching it does not help.
        let files: Vec<(String, Vec<u8>)> = specs
            .iter()
            .map(|s| {
                (
                    String::from_utf8_lossy(&s.name).into_owned(),
                    s.data.clone(),
                )
            })
            .collect();
        let correct = tree_digest(&hashed(&as_refs(&files)));
        let mut t = MapTransport::default();
        t.0.insert(
            format!("http://proxy/{PADS}/@v/v1.0.0.zip"),
            write_zip(&specs, 0),
        );
        let mut proxy = ProxyStore::new("http://proxy", t);
        proxy.limits = limits;
        let e = fetch_and_publish(
            &mut proxy,
            &mut cache,
            &id(PADS),
            &v("v1.0.0"),
            Some(correct),
        )
        .expect_err(label);
        assert_eq!(e.code(), DiagCode::PackageIntegrity, "{label}: {e}");
        match &e {
            PkgError::Integrity(i) => assert_eq!(i.entry, entry, "{label}"),
            other => panic!("{label}: {other:?}"),
        }
        assert_eq!(snapshot(cache.root()), before, "{label}: the cache changed");
    }
    // A store that skips validation: the cache's own checks refuse it.
    let mut lax = MemStore::default();
    lax.add(
        PADS,
        "v1.0.0",
        vec![
            ("ok.vact".into(), b"1".to_vec()),
            ("../evil".into(), b"x".to_vec()),
        ],
    );
    let e =
        fetch_and_publish(&mut lax, &mut cache, &id(PADS), &v("v1.0.0"), None).expect_err("lax");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    lax.add(
        PADS,
        "v1.0.1",
        vec![("a".into(), b"1".to_vec()), ("A".into(), b"2".to_vec())],
    );
    let e =
        fetch_and_publish(&mut lax, &mut cache, &id(PADS), &v("v1.0.1"), None).expect_err("fold");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    lax.add(
        PADS,
        "v1.0.2",
        vec![(STAMP_FILE.into(), b"forged".to_vec())],
    );
    let e =
        fetch_and_publish(&mut lax, &mut cache, &id(PADS), &v("v1.0.2"), None).expect_err("stamp");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    assert_eq!(snapshot(cache.root()), before);
}

#[cfg(unix)]
#[test]
fn a_directory_symlink_escaping_the_root_is_rejected() {
    let tmp = TempDir::new();
    let outside = tmp.join("outside");
    fs::create_dir_all(outside.join("warm")).expect("mkdir");
    fs::write(outside.join("warm/a.wav"), b"RIFF").expect("write");
    let manifest = format!("[package]\npath = \"{PADS}\"\nassets = [\"samples\"]\n");
    let dir = dir_package(
        &tmp.join("store"),
        PADS,
        "v1.0.0",
        &[
            (MANIFEST_FILE, manifest.as_bytes()),
            ("pads.vact", b"let warm 3\n"),
        ],
    );
    std::os::unix::fs::symlink(&outside, dir.join("samples")).expect("symlink");
    let mut store = DirStore::new(&tmp.join("store"));
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let before = snapshot(cache.root());
    let e = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.0.0"), None)
        .expect_err("symlink");
    match e {
        PkgError::Integrity(i) => assert_eq!(i.entry, "samples"),
        other => panic!("{other:?}"),
    }
    assert_eq!(snapshot(cache.root()), before);
}

#[test]
fn an_interrupted_extraction_leaves_no_entry() {
    let tmp = TempDir::new();
    let mut store = pads_store(&tmp);
    let mut cache = FailAfter {
        inner: FsCache::new(&tmp.join("cache")).expect("cache"),
        left: pads_files().len() / 2,
    };
    let e = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.0.0"), None)
        .expect_err("injected");
    assert_eq!(e, PkgError::Io("injected failure".into()));
    assert!(!cache.inner.entry(&id(PADS), &v("v1.0.0")).exists());
    assert!(staging_is_empty(&cache.inner));
}

#[test]
fn stamp_and_lock_mismatches_are_integrity_errors() {
    let tmp = TempDir::new();
    let mut store = pads_store(&tmp);
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    // A hash mismatch against the lock: nothing is published.
    let before = snapshot(cache.root());
    let e = fetch_and_publish(
        &mut store,
        &mut cache,
        &id(PADS),
        &v("v1.0.0"),
        Some([1; 32]),
    )
    .expect_err("mismatch");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    assert!(e.to_string().contains("digest mismatch"), "{e}");
    assert_eq!(snapshot(cache.root()), before);
    // Not fetched yet.
    let e = cache
        .read_verified(&id(PADS), &v("v1.0.0"), [1; 32])
        .expect_err("absent");
    assert_eq!(e.code(), DiagCode::PackageNotFetched);
    let d = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.0.0"), None).expect("ok");
    // A wrong expected stamp.
    let e = cache
        .read_verified(&id(PADS), &v("v1.0.0"), [0; 32])
        .expect_err("wrong");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    // A tampered, then a missing stamp.
    let stamp = cache.entry(&id(PADS), &v("v1.0.0")).join(STAMP_FILE);
    fs::write(&stamp, format!("{}\n", hex(&[0; 32]))).expect("tamper");
    let e = cache
        .read_verified(&id(PADS), &v("v1.0.0"), d)
        .expect_err("tampered");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    fs::remove_file(&stamp).expect("remove");
    let e = cache
        .read_verified(&id(PADS), &v("v1.0.0"), d)
        .expect_err("no stamp");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
}

#[test]
fn an_existing_entry_with_another_digest_is_not_replaced() {
    let tmp = TempDir::new();
    let mut store = pads_store(&tmp);
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let d = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.0.0"), None).expect("ok");
    let mut other = MemStore::default();
    other.add(
        PADS,
        "v1.0.0",
        vec![("pads.vact".into(), b"let warm 99\n".to_vec())],
    );
    let e = fetch_and_publish(&mut other, &mut cache, &id(PADS), &v("v1.0.0"), None)
        .expect_err("clash");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    assert!(cache.read_verified(&id(PADS), &v("v1.0.0"), d).is_ok());
    assert!(staging_is_empty(&cache));
}

#[test]
fn get_all_resolves_fetches_and_locks() {
    let tmp = TempDir::new();
    let mut store = MemStore::default();
    let mut pads = pads_files();
    pads[0].1 = format!(
        "[package]\npath = \"{PADS}\"\nassets = [\"samples\"]\n\n[deps]\n\"{DRUMS}\" = \"v0.2.0\"\n"
    )
    .into_bytes();
    store.add(PADS, "v1.1.0", pads);
    store.add(
        DRUMS,
        "v0.2.0",
        vec![("drums.vact".into(), b"let kick 1\n".to_vec())],
    );
    store.add(
        DRUMS,
        "v0.3.0",
        vec![("drums.vact".into(), b"let kick 2\n".to_vec())],
    );
    assert_eq!(store.list_versions(&id(DRUMS)).expect("listed").len(), 2);
    let root = parse(&deps_manifest(None, &[(PADS, "v1.1.0")])).expect("root");
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let lock = get_all(&root, &mut store, &mut cache).expect("get");
    let got: Vec<(String, String)> = lock
        .entries
        .iter()
        .map(|e| (e.id.0.to_string(), e.version.to_string()))
        .collect();
    assert_eq!(
        got,
        [
            (DRUMS.to_string(), "v0.2.0".to_string()),
            (PADS.to_string(), "v1.1.0".to_string())
        ]
    );
    for e in &lock.entries {
        let src = locked_sources(&lock, &cache, &e.id).expect("verified read");
        assert_eq!(src.version, e.version);
    }
    let e = locked_sources(&lock, &cache, &id("github.com/x/y")).expect_err("not locked");
    assert_eq!(e.code(), DiagCode::PackageNotLocked);
}
