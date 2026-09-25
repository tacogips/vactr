//! Native package stores and the file-system cache (design 14.5.7);
//! non-wasm32 only. `vactrol get` is the only native fetch path.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::pkg::store::PkgError;
use crate::pkg::validate::{Budget, EntryKind, IntegrityError, Limits};

pub mod dir_store;
pub mod fs_cache;
pub mod git_store;

pub use dir_store::DirStore;
pub use fs_cache::FsCache;
pub use git_store::GitStore;

/// `$VACTROL_HOME`, else `$HOME/.vactrol` (else `.vactrol`).
#[must_use]
pub fn vactrol_home() -> PathBuf {
    if let Some(h) = std::env::var_os("VACTROL_HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(h);
    }
    match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        Some(home) => PathBuf::from(home).join(".vactrol"),
        None => PathBuf::from(".vactrol"),
    }
}

/// The package cache root, `<home>/pkg`.
#[must_use]
pub fn pkg_cache_root() -> PathBuf {
    vactrol_home().join("pkg")
}

pub(crate) fn io(what: &str, path: &Path, e: &std::io::Error) -> PkgError {
    PkgError::Io(format!("{what} `{}`: {e}", path.display()))
}

/// A process-unique name: time, pid and a counter.
pub(crate) fn unique_name(tag: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{tag}{nanos:x}-{:x}-{n:x}", std::process::id())
}

/// A scratch directory created fresh (`create_dir`) and removed on drop.
#[derive(Debug)]
pub(crate) struct ScratchDir(PathBuf);

impl ScratchDir {
    pub(crate) fn new(tag: &str) -> Result<ScratchDir, PkgError> {
        let base = std::env::temp_dir();
        for _ in 0..8 {
            let path = base.join(unique_name(tag));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(ScratchDir(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(io("cannot create", &path, &e)),
            }
        }
        Err(PkgError::Io("cannot create a scratch directory".into()))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn kind_of(meta: &fs::Metadata) -> EntryKind {
    let ft = meta.file_type();
    if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() && !hard_linked(meta) {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

#[cfg(unix)]
fn hard_linked(meta: &fs::Metadata) -> bool {
    std::os::unix::fs::MetadataExt::nlink(meta) > 1
}

#[cfg(not(unix))]
fn hard_linked(_: &fs::Metadata) -> bool {
    false
}

/// Lists the tree under `root` (`symlink_metadata`, never following a
/// link) as `(relative path, kind, size)`, sorted bytewise; `skip` names
/// root-level entries to leave out. The caps are checked while walking.
///
/// # Errors
/// `Io`, or `Integrity` for a non-UTF-8 name or a cap exceeded.
pub(crate) fn walk(
    root: &Path,
    skip: &[&str],
    limits: &Limits,
) -> Result<Vec<(String, EntryKind, u64)>, PkgError> {
    let mut budget = Budget::new(*limits);
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, String)> = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let rd = fs::read_dir(&dir).map_err(|e| io("cannot read", &dir, &e))?;
        for entry in rd {
            let entry = entry.map_err(|e| io("cannot read", &dir, &e))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                let lossy = name.to_string_lossy().into_owned();
                return Err(
                    IntegrityError::new(&format!("{rel}{lossy}"), "the name is not UTF-8").into(),
                );
            };
            if rel.is_empty() && skip.contains(&name) {
                continue;
            }
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| io("cannot read", &path, &e))?;
            let kind = kind_of(&meta);
            let rel_path = format!("{rel}{name}");
            let size = if kind == EntryKind::File {
                meta.len()
            } else {
                0
            };
            budget.admit(&rel_path, size)?;
            if kind == EntryKind::Dir {
                stack.push((path, format!("{rel_path}/")));
            }
            out.push((rel_path, kind, size));
        }
    }
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(out)
}
