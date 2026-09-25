//! An in-memory package cache (design 15.1.2 G6, 14.5.7): the same
//! staged, atomic-publish contract as `native::fs_cache::FsCache`, wholly
//! in memory. `create_staging` never reuses an id; `publish` moves a
//! staging location to its entry atomically, and a same-stamp republish
//! succeeds and drops the staging, exactly as `FsCache`'s
//! `.vactrol-digest` stamp file behaves, but as a plain field instead of
//! a file. Every operation is synchronous, allocation-only and uses no
//! `std::{fs,net,process,thread,time}`, so this module is wasm-safe.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ns::pkg::PackageId;
use crate::pkg::cache::{CacheBackend, StagingId};
use crate::pkg::semver::Version;
use crate::pkg::store::{PkgError, PkgSources};
use crate::pkg::validate::{check_path, validate_entries, EntryKind, IntegrityError, Limits};

/// One staged or published package tree: `path -> bytes`.
type FileMap = BTreeMap<String, Vec<u8>>;

/// A published entry: the digest it was stamped with, and its files.
struct Entry {
    stamp: [u8; 32],
    files: FileMap,
}

/// A package cache wholly in memory (design 14.5.7).
#[derive(Default)]
pub struct MemCache {
    staging: BTreeMap<u64, FileMap>,
    next: u64,
    published: BTreeMap<(PackageId, Version), Entry>,
    pub limits: Limits,
}

impl MemCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> MemCache {
        MemCache::default()
    }

    /// True when no staging location is open.
    #[must_use]
    pub fn is_staging_empty(&self) -> bool {
        self.staging.is_empty()
    }

    /// True when `id@version` has a published entry.
    #[must_use]
    pub fn is_published(&self, id: &PackageId, version: &Version) -> bool {
        self.published.contains_key(&(id.clone(), version.clone()))
    }

    fn staged(&self, s: StagingId) -> Result<&FileMap, PkgError> {
        self.staging
            .get(&s.0)
            .ok_or_else(|| PkgError::Io(format!("unknown staging location {}", s.0)))
    }
}

impl CacheBackend for MemCache {
    fn create_staging(&mut self) -> Result<StagingId, PkgError> {
        let id = self.next;
        self.next += 1;
        self.staging.insert(id, FileMap::new());
        Ok(StagingId(id))
    }

    fn write(&mut self, staging: StagingId, path: &str, bytes: &[u8]) -> Result<(), PkgError> {
        check_path(path).map_err(|why| IntegrityError::new(path, why))?;
        let files = self
            .staging
            .get_mut(&staging.0)
            .ok_or_else(|| PkgError::Io(format!("unknown staging location {}", staging.0)))?;
        if files.contains_key(path) {
            return Err(PkgError::Io(format!(
                "`{path}` already exists in the staging location"
            )));
        }
        files.insert(path.to_string(), bytes.to_vec());
        Ok(())
    }

    fn remove_staging(&mut self, staging: StagingId) {
        self.staging.remove(&staging.0);
    }

    fn publish(
        &mut self,
        staging: StagingId,
        id: &PackageId,
        version: &Version,
        stamp: [u8; 32],
    ) -> Result<(), PkgError> {
        let key = (id.clone(), version.clone());
        if let Some(existing) = self.published.get(&key) {
            return if existing.stamp == stamp {
                self.remove_staging(staging);
                Ok(())
            } else {
                Err(IntegrityError::new(
                    &format!("{}@{version}", id.0),
                    "the cache entry exists with another digest",
                )
                .into())
            };
        }
        let files = self.staged(staging)?.clone();
        self.published.insert(key, Entry { stamp, files });
        self.staging.remove(&staging.0);
        Ok(())
    }

    fn read_verified(
        &self,
        id: &PackageId,
        version: &Version,
        expect: [u8; 32],
    ) -> Result<PkgSources, PkgError> {
        let key = (id.clone(), version.clone());
        let Some(entry) = self.published.get(&key) else {
            return Err(PkgError::NotFetched(format!(
                "package `{}@{version}`",
                id.0
            )));
        };
        if entry.stamp != expect {
            return Err(PkgError::mismatch(id, version, &expect, &entry.stamp));
        }
        let entries: Vec<(String, EntryKind, u64)> = entry
            .files
            .iter()
            .map(|(p, b)| (p.clone(), EntryKind::File, b.len() as u64))
            .collect();
        validate_entries(&entries, &self.limits)?;
        let files: Vec<(Rc<str>, Rc<[u8]>)> = entry
            .files
            .iter()
            .map(|(p, b)| (Rc::from(p.as_str()), Rc::from(b.as_slice())))
            .collect();
        PkgSources::from_files(id.clone(), version.clone(), files, None)
    }
}
