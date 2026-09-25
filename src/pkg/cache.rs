//! Staged, atomic, verified cache publication (design 5.7 revised,
//! 14.5.7).
//!
//! `fetch_and_publish` runs the exact sequence: fetch into a fresh staging
//! location -> validate -> digest -> compare with the lock (when known) ->
//! stamp -> atomic publish. Every staged write is checked as it arrives
//! (path, reserved names, caps) and the whole entry list is validated
//! again before the digest, so the cache never holds unverified content
//! whatever the store did. On ANY error the staging location is removed
//! and nothing is published.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::ns::pkg::PackageId;
use crate::pkg::digest::tree_digest;
use crate::pkg::lock::{LockEntry, LockFile};
use crate::pkg::manifest::PkgManifest;
use crate::pkg::mvs::mvs_resolve;
use crate::pkg::semver::Version;
use crate::pkg::sha256::sha256;
use crate::pkg::store::{
    parse_manifest, PackageStore, PkgError, PkgSources, StagingSink, MANIFEST_FILE, STAMP_FILE,
};
use crate::pkg::validate::{
    check_path, fold, validate_assets, validate_entries, Budget, EntryKind, IntegrityError, Limits,
};

/// A staging location of a `CacheBackend`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StagingId(pub u64);

/// The package cache (native: `FsCache`; the browser's OPFS backend is
/// TASK-010's).
pub trait CacheBackend {
    /// A fresh, empty staging location (never reused).
    ///
    /// # Errors
    /// `Io`.
    fn create_staging(&mut self) -> Result<StagingId, PkgError>;

    /// Writes one file of a staged package.
    ///
    /// # Errors
    /// `Io`, or `Integrity` for an unsafe path.
    fn write(&mut self, staging: StagingId, path: &str, bytes: &[u8]) -> Result<(), PkgError>;

    /// Removes a staging location and everything in it.
    fn remove_staging(&mut self, staging: StagingId);

    /// Stamps the staged tree with `stamp` and atomically moves it to the
    /// entry `id@version`. Publishing an entry that already exists with the
    /// same stamp succeeds and drops the staging location.
    ///
    /// # Errors
    /// `Io`, or `Integrity` when the entry exists with another stamp.
    fn publish(
        &mut self,
        staging: StagingId,
        id: &PackageId,
        version: &Version,
        stamp: [u8; 32],
    ) -> Result<(), PkgError>;

    /// The sources of the entry `id@version`, whose stamp must equal
    /// `expect` (no re-hash).
    ///
    /// # Errors
    /// `NotFetched` with no entry, `Integrity` for a stamp mismatch.
    fn read_verified(
        &self,
        id: &PackageId,
        version: &Version,
        expect: [u8; 32],
    ) -> Result<PkgSources, PkgError>;
}

/// The staging sink `fetch_and_publish` hands the store: checks and
/// records every write before it reaches the cache.
struct Recorder<'a> {
    cache: &'a mut dyn CacheBackend,
    staging: StagingId,
    budget: Budget,
    entries: Vec<(String, EntryKind, u64)>,
    hashes: Vec<(Rc<str>, [u8; 32])>,
    manifest: Option<Vec<u8>>,
    /// Case-folded paths written so far: a duplicate is refused before it
    /// reaches a case-insensitive file system.
    folded: BTreeSet<String>,
}

impl StagingSink for Recorder<'_> {
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), PkgError> {
        check_path(path).map_err(|why| IntegrityError::new(path, why))?;
        if path == STAMP_FILE {
            return Err(IntegrityError::new(path, "a reserved file name").into());
        }
        self.budget.admit(path, bytes.len() as u64)?;
        if !self.folded.insert(fold(path)) {
            return Err(IntegrityError::new(path, "a duplicate entry (under case folding)").into());
        }
        self.cache.write(self.staging, path, bytes)?;
        self.entries
            .push((path.to_string(), EntryKind::File, bytes.len() as u64));
        self.hashes.push((Rc::from(path), sha256(bytes)));
        if path == MANIFEST_FILE {
            self.manifest = Some(bytes.to_vec());
        }
        Ok(())
    }
}

fn stage_and_verify(
    store: &mut dyn PackageStore,
    cache: &mut dyn CacheBackend,
    staging: StagingId,
    id: &PackageId,
    version: &Version,
    expect: Option<[u8; 32]>,
) -> Result<[u8; 32], PkgError> {
    let limits = Limits::default();
    let mut rec = Recorder {
        cache,
        staging,
        budget: Budget::new(limits),
        entries: Vec::new(),
        hashes: Vec::new(),
        manifest: None,
        folded: BTreeSet::new(),
    };
    store.fetch_into(id, version, &mut rec)?;
    validate_entries(&rec.entries, &limits)?;
    if let Some(bytes) = &rec.manifest {
        if let Some(meta) = parse_manifest(id, bytes)?.package {
            validate_assets(&rec.entries, &meta.assets)?;
        }
    }
    let digest = tree_digest(&rec.hashes);
    if let Some(expect) = expect {
        if expect != digest {
            return Err(PkgError::mismatch(id, version, &expect, &digest));
        }
    }
    Ok(digest)
}

/// Fetches `id@version` from `store` and publishes it into `cache`; the
/// canonical digest on success.
///
/// # Errors
/// Any store, validation, digest-mismatch (`Integrity`) or cache error;
/// the staging location is removed and nothing is published.
pub fn fetch_and_publish(
    store: &mut dyn PackageStore,
    cache: &mut dyn CacheBackend,
    id: &PackageId,
    version: &Version,
    expect: Option<[u8; 32]>,
) -> Result<[u8; 32], PkgError> {
    let staging = cache.create_staging()?;
    let r = stage_and_verify(store, cache, staging, id, version, expect)
        .and_then(|digest| cache.publish(staging, id, version, digest).map(|()| digest));
    if r.is_err() {
        cache.remove_staging(staging);
    }
    r
}

/// `vactrol get`'s core: MVS over the root manifest's requirements, then
/// fetch and publish every selected version, then the lock.
///
/// # Errors
/// The first resolution, fetch or cache error.
pub fn get_all(
    manifest: &PkgManifest,
    store: &mut dyn PackageStore,
    cache: &mut dyn CacheBackend,
) -> Result<LockFile, PkgError> {
    let selected = mvs_resolve(&manifest.deps, store)?;
    let mut entries = Vec::with_capacity(selected.len());
    for (id, version) in selected {
        let sha256 = fetch_and_publish(store, cache, &id, &version, None)?;
        entries.push(LockEntry {
            id,
            version,
            sha256,
        });
    }
    Ok(LockFile::new(entries))
}
