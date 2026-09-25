//! The native package cache (design 14.5.7): `<root>/<path>@<version>/`
//! with the stamp `.vactrol-digest` (hex). Packages are staged under
//! `<root>/.staging/<unique>` (created with `create_dir`, never reused)
//! and published with one `rename`. The root is `$VACTROL_HOME/pkg`
//! (`native::pkg_cache_root`).

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ns::pkg::PackageId;
use crate::pkg::cache::{CacheBackend, StagingId};
use crate::pkg::digest::{hex, parse_hex};
use crate::pkg::native::{io, unique_name, walk};
use crate::pkg::semver::Version;
use crate::pkg::store::{PkgError, PkgSources, STAMP_FILE};
use crate::pkg::validate::{check_path, validate_entries, EntryKind, IntegrityError, Limits};

/// The staging directory under a cache root.
pub const STAGING_DIR: &str = ".staging";

/// A package cache on the file system.
#[derive(Debug)]
pub struct FsCache {
    root: PathBuf,
    staging: BTreeMap<u64, PathBuf>,
    next: u64,
    pub limits: Limits,
}

impl FsCache {
    /// A cache at `root`; creates `root` and its staging directory.
    ///
    /// # Errors
    /// `Io`.
    pub fn new(root: &Path) -> Result<FsCache, PkgError> {
        let staging = root.join(STAGING_DIR);
        fs::create_dir_all(&staging).map_err(|e| io("cannot create", &staging, &e))?;
        Ok(FsCache {
            root: root.to_path_buf(),
            staging: BTreeMap::new(),
            next: 0,
            limits: Limits::default(),
        })
    }

    /// The cache root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The entry directory of `id@version`.
    #[must_use]
    pub fn entry(&self, id: &PackageId, version: &Version) -> PathBuf {
        self.root.join(format!("{}@{version}", id.0))
    }

    fn staged(&self, s: StagingId) -> Result<&PathBuf, PkgError> {
        self.staging
            .get(&s.0)
            .ok_or_else(|| PkgError::Io(format!("unknown staging location {}", s.0)))
    }
}

fn read_stamp(entry: &Path) -> Option<[u8; 32]> {
    let text = fs::read_to_string(entry.join(STAMP_FILE)).ok()?;
    parse_hex(text.trim_end())
}

impl CacheBackend for FsCache {
    fn create_staging(&mut self) -> Result<StagingId, PkgError> {
        let base = self.root.join(STAGING_DIR);
        for _ in 0..8 {
            let dir = base.join(unique_name(""));
            match fs::create_dir(&dir) {
                Ok(()) => {
                    let id = self.next;
                    self.next += 1;
                    self.staging.insert(id, dir);
                    return Ok(StagingId(id));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(io("cannot create", &dir, &e)),
            }
        }
        Err(PkgError::Io("cannot create a staging directory".into()))
    }

    fn write(&mut self, staging: StagingId, path: &str, bytes: &[u8]) -> Result<(), PkgError> {
        check_path(path).map_err(|why| IntegrityError::new(path, why))?;
        let file = self.staged(staging)?.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(|e| io("cannot create", parent, &e))?;
        }
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .map_err(|e| io("cannot create", &file, &e))?;
        f.write_all(bytes)
            .map_err(|e| io("cannot write", &file, &e))
    }

    fn remove_staging(&mut self, staging: StagingId) {
        if let Some(dir) = self.staging.remove(&staging.0) {
            let _ = fs::remove_dir_all(dir);
        }
    }

    fn publish(
        &mut self,
        staging: StagingId,
        id: &PackageId,
        version: &Version,
        stamp: [u8; 32],
    ) -> Result<(), PkgError> {
        let dir = self.staged(staging)?.clone();
        let target = self.entry(id, version);
        if fs::symlink_metadata(&target).is_ok() {
            return if read_stamp(&target) == Some(stamp) {
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
        let stamp_file = dir.join(STAMP_FILE);
        fs::write(&stamp_file, format!("{}\n", hex(&stamp)))
            .map_err(|e| io("cannot write", &stamp_file, &e))?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io("cannot create", parent, &e))?;
        }
        fs::rename(&dir, &target).map_err(|e| io("cannot publish", &target, &e))?;
        self.staging.remove(&staging.0);
        Ok(())
    }

    fn read_verified(
        &self,
        id: &PackageId,
        version: &Version,
        expect: [u8; 32],
    ) -> Result<PkgSources, PkgError> {
        let entry = self.entry(id, version);
        if !entry.is_dir() {
            return Err(PkgError::NotFetched(format!(
                "package `{}@{version}`",
                id.0
            )));
        }
        let Some(stamp) = read_stamp(&entry) else {
            return Err(IntegrityError::new(
                &format!("{}@{version}", id.0),
                "the cache entry has no valid digest stamp",
            )
            .into());
        };
        if stamp != expect {
            return Err(PkgError::mismatch(id, version, &expect, &stamp));
        }
        let entries = walk(&entry, &[STAMP_FILE], &self.limits)?;
        validate_entries(&entries, &self.limits)?;
        let mut files = Vec::new();
        for (path, kind, _) in &entries {
            if *kind == EntryKind::File {
                let file = entry.join(path);
                let bytes = fs::read(&file).map_err(|e| io("cannot read", &file, &e))?;
                files.push((Rc::from(path.as_str()), Rc::from(bytes)));
            }
        }
        let root = entry.to_str().map(Rc::from);
        PkgSources::from_files(id.clone(), version.clone(), files, root)
    }
}
