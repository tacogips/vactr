//! The session side of package loading (design 5.7 "Loading", 14.5.7):
//! the default prefix, the locked verified read, and the asset banks. The
//! evaluation of a package's files into its `PkgNs` is `PkgNs::load`
//! (`ns/pkg.rs`).

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::ns::pkg::PackageId;
use crate::pkg::cache::CacheBackend;
use crate::pkg::lock::LockFile;
use crate::pkg::store::{PkgError, PkgSources};
use crate::pkg::validate::{validate_assets, EntryKind};
use crate::value::value::PathVal;

/// The default import prefix: the last path segment without `vactrol-`.
#[must_use]
pub fn default_prefix(id: &PackageId) -> Rc<str> {
    let last = id.0.rsplit('/').next().unwrap_or(&id.0);
    Rc::from(last.strip_prefix("vactrol-").unwrap_or(last))
}

/// The verified cached sources of the locked package `id`. A running
/// session never fetches; this is its only way to a package's files.
///
/// # Errors
/// `NotLocked` when `id` is not in the lock, else `read_verified`'s error.
pub fn locked_sources(
    lock: &LockFile,
    cache: &dyn CacheBackend,
    id: &PackageId,
) -> Result<PkgSources, PkgError> {
    let entry = lock
        .get(id)
        .ok_or_else(|| PkgError::NotLocked(id.clone()))?;
    cache.read_verified(&entry.id, &entry.version, entry.sha256)
}

/// The sample banks of a package: every immediate subdirectory `<bank>` of
/// each validated manifest asset directory is the bank keyword
/// `:<prefix>-<bank>` (returned without the `:`), holding its `.wav` files
/// in bytewise path order. With a file-system root the paths are absolute.
#[must_use]
pub fn asset_banks(sources: &PkgSources, prefix: &str) -> Vec<(Rc<str>, Vec<PathVal>)> {
    let Some(meta) = &sources.manifest.package else {
        return Vec::new();
    };
    let entries: Vec<(String, EntryKind, u64)> = sources
        .files
        .iter()
        .map(|(p, b)| (p.to_string(), EntryKind::File, b.len() as u64))
        .collect();
    let assets = validate_assets(&entries, &meta.assets).unwrap_or_default();
    let mut banks: BTreeMap<String, Vec<Rc<str>>> = BTreeMap::new();
    for asset in &assets {
        let dir = format!("{asset}/");
        for (path, _) in &sources.files {
            let Some((bank, file)) = path.strip_prefix(&dir).and_then(|r| r.split_once('/')) else {
                continue;
            };
            let wav = file
                .rsplit_once('.')
                .is_some_and(|(_, x)| x.eq_ignore_ascii_case("wav"));
            if !file.contains('/') && wav {
                banks
                    .entry(bank.to_string())
                    .or_default()
                    .push(Rc::clone(path));
            }
        }
    }
    banks
        .into_iter()
        .map(|(bank, mut files)| {
            files.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            let files = files
                .into_iter()
                .map(|p| PathVal {
                    text: match &sources.root {
                        Some(root) => Rc::from(format!("{root}/{p}")),
                        None => p,
                    },
                    file: None,
                })
                .collect();
            (Rc::from(format!("{prefix}-{bank}")), files)
        })
        .collect()
}
