//! The local-directory store (design 14.5.7): `<root>/<path>@<version>/`,
//! used by fixtures and by `vactrol get --store dir:<root>`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ns::pkg::PackageId;
use crate::pkg::manifest::PkgManifest;
use crate::pkg::native::{io, walk};
use crate::pkg::semver::Version;
use crate::pkg::store::{parse_manifest, PackageStore, PkgError, StagingSink, MANIFEST_FILE};
use crate::pkg::validate::{validate_entries, EntryKind, Limits};

/// A store over a local directory of `<path>@<version>` trees.
#[derive(Clone, Debug)]
pub struct DirStore {
    root: PathBuf,
    pub limits: Limits,
}

impl DirStore {
    /// A store over `root`.
    #[must_use]
    pub fn new(root: &Path) -> DirStore {
        DirStore {
            root: root.to_path_buf(),
            limits: Limits::default(),
        }
    }

    /// The tree of `id@version`.
    #[must_use]
    pub fn tree(&self, id: &PackageId, version: &Version) -> PathBuf {
        self.root.join(format!("{}@{version}", id.0))
    }

    fn existing(&self, id: &PackageId, version: &Version) -> Result<PathBuf, PkgError> {
        let dir = self.tree(id, version);
        if dir.is_dir() {
            Ok(dir)
        } else {
            Err(PkgError::Unresolvable(format!(
                "cannot resolve `{}`: no version {version}",
                id.0
            )))
        }
    }
}

impl PackageStore for DirStore {
    fn list_versions(&mut self, id: &PackageId) -> Result<Vec<Version>, PkgError> {
        let (owner, name) = id.0.rsplit_once('/').unwrap_or(("", &id.0));
        let parent = self.root.join(owner);
        let rd = fs::read_dir(&parent).map_err(|_| {
            PkgError::Unresolvable(format!("cannot resolve `{}`: not in the store", id.0))
        })?;
        let prefix = format!("{name}@");
        let mut out: Vec<Version> = rd
            .filter_map(Result::ok)
            .filter_map(|e| {
                let n = e.file_name();
                Version::parse_tag(n.to_str()?.strip_prefix(&prefix)?)
            })
            .collect();
        out.sort();
        Ok(out)
    }

    fn manifest(&mut self, id: &PackageId, version: &Version) -> Result<PkgManifest, PkgError> {
        let file = self.existing(id, version)?.join(MANIFEST_FILE);
        match fs::symlink_metadata(&file) {
            Ok(m) if m.file_type().is_file() => {
                let bytes = fs::read(&file).map_err(|e| io("cannot read", &file, &e))?;
                parse_manifest(id, &bytes)
            }
            Ok(_) => Err(PkgError::Integrity(crate::pkg::IntegrityError::new(
                MANIFEST_FILE,
                "not a regular file",
            ))),
            Err(_) => Ok(PkgManifest::default()),
        }
    }

    fn fetch_into(
        &mut self,
        id: &PackageId,
        version: &Version,
        staging: &mut dyn StagingSink,
    ) -> Result<(), PkgError> {
        let dir = self.existing(id, version)?;
        write_tree(&dir, &[], &self.limits, staging)
    }
}

/// Walks and validates the tree under `dir`, then writes its files into
/// `staging` (shared with the git store).
///
/// # Errors
/// `Integrity` for an unsafe tree, `Io`, or the sink's error.
pub(crate) fn write_tree(
    dir: &Path,
    skip: &[&str],
    limits: &Limits,
    staging: &mut dyn StagingSink,
) -> Result<(), PkgError> {
    let entries = walk(dir, skip, limits)?;
    validate_entries(&entries, limits)?;
    for (path, kind, _) in &entries {
        if *kind == EntryKind::File {
            let file = dir.join(path);
            let bytes = fs::read(&file).map_err(|e| io("cannot read", &file, &e))?;
            staging.write(path, &bytes)?;
        }
    }
    Ok(())
}
