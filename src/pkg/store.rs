//! The package store capability and the package error (design 5.7,
//! 14.5.7). Stores run on the IO side (`vactr get`), never on the
//! evaluator; a running session reads only the verified cache.

use std::fmt;
use std::rc::Rc;

use crate::ns::pkg::PackageId;
use crate::pkg::digest::hex;
use crate::pkg::manifest::{self, ManifestError, PkgManifest};
use crate::pkg::semver::Version;
use crate::pkg::validate::{validate_assets, EntryKind, IntegrityError};
use crate::types::diag::DiagCode;

/// The manifest file at a package root.
pub const MANIFEST_FILE: &str = "vactr.toml";
/// The lock file next to a root manifest.
pub const LOCK_FILE: &str = "vactr.lock";
/// The digest stamp of a published cache entry; reserved at a package root.
pub const STAMP_FILE: &str = ".vactr-digest";

/// A package error, mapped to its 14.5.12 code by [`PkgError::code`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PkgError {
    /// No such version, or no such package.
    Unresolvable(String),
    /// A store or transport failure.
    Network(String),
    Integrity(IntegrityError),
    Manifest(ManifestError),
    /// The import is not in `vactr.lock`.
    NotLocked(PackageId),
    /// The lock entry has no verified cache entry.
    NotFetched(String),
    Io(String),
}

impl PkgError {
    /// The diagnostic code the error is reported with.
    #[must_use]
    pub fn code(&self) -> DiagCode {
        match self {
            PkgError::Unresolvable(_) | PkgError::Network(_) | PkgError::Io(_) => {
                DiagCode::PackageResolve
            }
            PkgError::Integrity(_) => DiagCode::PackageIntegrity,
            PkgError::Manifest(_) => DiagCode::PackageLoadFailed,
            PkgError::NotLocked(_) => DiagCode::PackageNotLocked,
            PkgError::NotFetched(_) => DiagCode::PackageNotFetched,
        }
    }

    /// A digest mismatch.
    #[must_use]
    pub fn mismatch(id: &PackageId, version: &Version, expect: &[u8; 32], got: &[u8; 32]) -> Self {
        PkgError::Integrity(IntegrityError::new(
            &format!("{}@{version}", id.0),
            format!(
                "digest mismatch: expected {}, got {}",
                hex(expect),
                hex(got)
            ),
        ))
    }
}

impl fmt::Display for PkgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PkgError::Unresolvable(m) | PkgError::Network(m) | PkgError::Io(m) => f.write_str(m),
            PkgError::Integrity(e) => e.fmt(f),
            PkgError::Manifest(e) => write!(f, "{MANIFEST_FILE} {e}"),
            PkgError::NotLocked(id) => write!(f, "package `{}` is not in `{LOCK_FILE}`", id.0),
            PkgError::NotFetched(m) => {
                write!(f, "{m} is locked but not fetched; run `vactr get`")
            }
        }
    }
}

impl std::error::Error for PkgError {}

impl From<IntegrityError> for PkgError {
    fn from(e: IntegrityError) -> Self {
        PkgError::Integrity(e)
    }
}

impl From<ManifestError> for PkgError {
    fn from(e: ManifestError) -> Self {
        PkgError::Manifest(e)
    }
}

/// Where a store writes a fetched package's files.
pub trait StagingSink {
    /// Writes one regular file at the package-relative `path`.
    ///
    /// # Errors
    /// An unsafe path, a cap exceeded, or a write failure.
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), PkgError>;
}

/// A package store (local directory, git, proxy).
pub trait PackageStore {
    /// The versions (semver tags) of `id`; other tags are ignored.
    ///
    /// # Errors
    /// `Unresolvable` for an unknown package, `Network` for a transport
    /// failure.
    fn list_versions(&mut self, id: &PackageId) -> Result<Vec<Version>, PkgError>;

    /// The manifest of `id@version` (the default manifest when the package
    /// has none).
    ///
    /// # Errors
    /// `Unresolvable`, `Network`, or `Manifest`.
    fn manifest(&mut self, id: &PackageId, version: &Version) -> Result<PkgManifest, PkgError>;

    /// Writes the files of `id@version` into `staging`, validated first.
    ///
    /// # Errors
    /// `Unresolvable`, `Network`, `Integrity`, or the sink's error.
    fn fetch_into(
        &mut self,
        id: &PackageId,
        version: &Version,
        staging: &mut dyn StagingSink,
    ) -> Result<(), PkgError>;
}

/// A verified package's sources, as read from the cache.
#[derive(Clone, Debug)]
pub struct PkgSources {
    pub id: PackageId,
    pub version: Version,
    /// `(package-relative path, contents)`, sorted bytewise by path.
    pub files: Vec<(Rc<str>, Rc<[u8]>)>,
    pub manifest: PkgManifest,
    /// The directory the files live in, when they are on a file system
    /// (a native cache entry), so asset banks can name real files.
    pub root: Option<Rc<str>>,
}

impl PkgSources {
    /// Sources from verified files: sorts them, parses the manifest, checks
    /// its `path` equals `id` and validates its assets.
    ///
    /// # Errors
    /// `Manifest` for a bad manifest or a path mismatch, `Integrity` for an
    /// asset outside the root.
    pub fn from_files(
        id: PackageId,
        version: Version,
        mut files: Vec<(Rc<str>, Rc<[u8]>)>,
        root: Option<Rc<str>>,
    ) -> Result<PkgSources, PkgError> {
        files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let manifest = match files.iter().find(|(p, _)| &**p == MANIFEST_FILE) {
            Some((_, bytes)) => parse_manifest(&id, bytes)?,
            None => PkgManifest::default(),
        };
        if let Some(meta) = &manifest.package {
            let entries: Vec<(String, EntryKind, u64)> = files
                .iter()
                .map(|(p, b)| (p.to_string(), EntryKind::File, b.len() as u64))
                .collect();
            validate_assets(&entries, &meta.assets)?;
        }
        Ok(PkgSources {
            id,
            version,
            files,
            manifest,
            root,
        })
    }
}

/// Parses a fetched package's manifest bytes and checks its `path` is
/// `id` (the path it was fetched as).
///
/// # Errors
/// `Manifest`.
pub fn parse_manifest(id: &PackageId, bytes: &[u8]) -> Result<PkgManifest, PkgError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ManifestError::at(0, format!("`{MANIFEST_FILE}` is not UTF-8")))?;
    let m = manifest::parse(text)?;
    if let Some(meta) = &m.package {
        if meta.path != *id {
            return Err(PkgError::Manifest(ManifestError::at(
                0,
                format!(
                    "the package path `{}` is not the import path `{}`",
                    meta.path.0, id.0
                ),
            )));
        }
    }
    Ok(m)
}
