//! `MemCache` (SS-PKG-adjacent G6 required tests): staging ids are
//! unique, `publish` is atomic (same-stamp republish succeeds and drops
//! the staging, another stamp is rejected and the entry is unchanged),
//! and a failure midway through `fetch_and_publish` leaves no entry and
//! no staging.

use crate::ns::pkg::PackageId;
use crate::pkg::cache::{fetch_and_publish, CacheBackend, StagingId};
use crate::pkg::mem_cache::MemCache;
use crate::pkg::semver::Version;
use crate::pkg::store::{PkgError, PkgSources};
use crate::pkg::tests::support::{id, v, MemStore, PADS};
use crate::types::diag::DiagCode;

/// A cache whose `write` fails after `left` successful writes (mirrors
/// `tests::cache::FailAfter`, adapted to wrap `MemCache`).
struct FailAfter {
    inner: MemCache,
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

#[test]
fn staging_ids_are_never_reused() {
    let mut cache = MemCache::new();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..5 {
        let s = cache.create_staging().expect("stage");
        assert!(seen.insert(s.0), "id {} reused", s.0);
    }
    // Removing a staging location does not free its id for reuse.
    let s = cache.create_staging().expect("stage");
    cache.remove_staging(s);
    let next = cache.create_staging().expect("stage");
    assert!(seen.insert(s.0));
    assert!(seen.insert(next.0));
    assert_ne!(s.0, next.0);
}

#[test]
fn publish_is_atomic() {
    let mut cache = MemCache::new();
    let pkg = id(PADS);
    let ver = v("v1.0.0");
    let stamp = [7u8; 32];
    let other_stamp = [9u8; 32];

    // A first publish moves the staged tree into the entry.
    let s1 = cache.create_staging().expect("stage");
    cache.write(s1, "mod.vact", b"let x 1\n").expect("write");
    cache.publish(s1, &pkg, &ver, stamp).expect("publish");
    assert!(cache.is_staging_empty());
    assert!(cache.is_published(&pkg, &ver));
    let src = cache.read_verified(&pkg, &ver, stamp).expect("verified");
    assert_eq!(src.files.len(), 1);
    assert_eq!(&*src.files[0].0, "mod.vact");

    // A republish with the SAME stamp succeeds and drops its own staging,
    // leaving the existing entry untouched.
    let s2 = cache.create_staging().expect("stage");
    cache.write(s2, "mod.vact", b"let x 1\n").expect("write");
    cache.publish(s2, &pkg, &ver, stamp).expect("republish");
    assert!(cache.is_staging_empty());

    // A publish with ANOTHER stamp is rejected as Integrity, naming the
    // entry, and the original entry is unchanged.
    let s3 = cache.create_staging().expect("stage");
    cache
        .write(s3, "mod.vact", b"let x 2\n")
        .expect("write different content");
    let e = cache
        .publish(s3, &pkg, &ver, other_stamp)
        .expect_err("stamp mismatch");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
    assert!(e.to_string().contains(&format!("{}@{ver}", pkg.0)), "{e}");
    // The entry under the original stamp is exactly as it was.
    let src = cache.read_verified(&pkg, &ver, stamp).expect("verified");
    assert_eq!(&*src.files[0].1, b"let x 1\n".as_slice());
    // The mismatched stamp does not verify.
    let e = cache
        .read_verified(&pkg, &ver, other_stamp)
        .expect_err("mismatch");
    assert_eq!(e.code(), DiagCode::PackageIntegrity);
}

#[test]
fn unpublished_entry_is_not_fetched() {
    let cache = MemCache::new();
    let e = cache
        .read_verified(&id(PADS), &v("v1.0.0"), [0u8; 32])
        .expect_err("not fetched");
    assert_eq!(e.code(), DiagCode::PackageNotFetched);
}

#[test]
fn a_failure_midway_leaves_no_entry() {
    let mut store = MemStore::default();
    store.add(
        PADS,
        "v1.0.0",
        vec![
            ("vactrol.toml".into(), b"[deps]\n".to_vec()),
            ("a.vact".into(), b"let a 1\n".to_vec()),
            ("b.vact".into(), b"let b 2\n".to_vec()),
        ],
    );
    let mut cache = FailAfter {
        inner: MemCache::new(),
        left: 1,
    };
    let pkg = id(PADS);
    let ver = v("v1.0.0");
    let r = fetch_and_publish(&mut store, &mut cache, &pkg, &ver, None);
    assert!(r.is_err(), "{r:?}");
    assert!(cache.inner.is_staging_empty());
    assert!(!cache.inner.is_published(&pkg, &ver));
    let e = cache
        .inner
        .read_verified(&pkg, &ver, [0u8; 32])
        .expect_err("no entry");
    assert_eq!(e.code(), DiagCode::PackageNotFetched);
}
