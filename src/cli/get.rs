//! `vactrol get [<path>[@version]] [--store dir:<root>]` (design 14.5.7,
//! `command.md`): adds or raises a requirement in `./vactrol.toml`, then
//! resolves, fetches and verifies every selected package into the cache
//! and writes `./vactrol.lock`.

use std::path::Path;

use crate::cli::args::StoreSpec;
use crate::ns::pkg::PackageId;
use crate::pkg::manifest::{self, PkgManifest};
use crate::pkg::native::{pkg_cache_root, DirStore, FsCache, GitStore};
use crate::pkg::semver::{latest_release, Version};
use crate::pkg::store::{PackageStore, PkgError, LOCK_FILE, MANIFEST_FILE};
use crate::pkg::{get_all, LockFile};

/// The git store base the CLI always passes (design 14.5.7): the store
/// resolves `<base>/<path>`, and every package path already carries its
/// own host (`github.com/...`).
const GIT_BASE: &str = "https://";

/// Exit 0, or 1 for any `PkgError` (`error[<code>]: <message>`).
pub fn main(target: Option<(PackageId, Option<Version>)>, store: StoreSpec, cwd: &Path) -> i32 {
    let manifest_path = cwd.join(MANIFEST_FILE);
    let mut pkg_manifest = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => match manifest::parse(&text) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("vactrol: {MANIFEST_FILE}: {e}");
                return 1;
            }
        },
        Err(_) => PkgManifest::default(),
    };

    let mut store_obj: Box<dyn PackageStore> = match &store {
        StoreSpec::Git => Box::new(GitStore::new(GIT_BASE)),
        StoreSpec::Dir(root) => Box::new(DirStore::new(root)),
    };

    if let Some((id, version)) = target {
        let version = match version {
            Some(v) => v,
            None => match resolve_latest(store_obj.as_mut(), &id) {
                Ok(v) => v,
                Err(e) => {
                    print_pkg_error(&e);
                    return 1;
                }
            },
        };
        pkg_manifest.add_or_raise(id, version);
    }

    let mut cache = match FsCache::new(&pkg_cache_root()) {
        Ok(c) => c,
        Err(e) => {
            print_pkg_error(&e);
            return 1;
        }
    };

    let lock = match get_all(&pkg_manifest, store_obj.as_mut(), &mut cache) {
        Ok(l) => l,
        Err(e) => {
            print_pkg_error(&e);
            return 1;
        }
    };

    if let Err(e) = write_atomic(&manifest_path, &manifest::render(&pkg_manifest)) {
        eprintln!("vactrol: cannot write `{MANIFEST_FILE}`: {e}");
        return 1;
    }
    if let Err(e) = write_atomic(&cwd.join(LOCK_FILE), &lock.render()) {
        eprintln!("vactrol: cannot write `{LOCK_FILE}`: {e}");
        return 1;
    }
    print_resolved(&lock);
    0
}

fn resolve_latest(store: &mut dyn PackageStore, id: &PackageId) -> Result<Version, PkgError> {
    let versions = store.list_versions(id)?;
    latest_release(&versions)
        .cloned()
        .ok_or_else(|| PkgError::Unresolvable(format!("`{}` has no release version", id.0)))
}

fn print_resolved(lock: &LockFile) {
    for e in &lock.entries {
        println!("{} {}", e.id.0, e.version);
    }
}

fn print_pkg_error(e: &PkgError) {
    eprintln!("vactrol: error[{}]: {e}", e.code());
}

/// Writes `contents` to `path` through a temp file and a same-directory
/// rename (14.5.7): the manifest and the lock never appear half-written.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}
