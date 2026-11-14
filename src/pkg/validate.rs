//! Package entry validation (design 5.7 revised "Extraction safety and
//! containment", 14.5.7). It runs FIRST on every store, before hashing and
//! before any cache write: a matching digest never makes unsafe content
//! safe.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

/// What an entry of a staged tree or an archive is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    /// A device, a fifo, a socket or a hard link.
    Other,
}

/// The entry-count and uncompressed-size caps, checked during extraction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub max_entries: usize,
    pub max_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_entries: 10_000,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

/// An unsafe entry: `package-integrity` naming it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IntegrityError {
    pub entry: String,
    pub reason: String,
}

impl IntegrityError {
    /// An error about `entry`.
    #[must_use]
    pub fn new(entry: &str, reason: impl Into<String>) -> IntegrityError {
        IntegrityError {
            entry: entry.to_string(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsafe package entry `{}`: {}",
            self.entry.escape_debug(),
            self.reason
        )
    }
}

impl std::error::Error for IntegrityError {}

/// The running entry count and byte total against `Limits`.
#[derive(Debug)]
pub struct Budget {
    limits: Limits,
    entries: usize,
    bytes: u64,
}

impl Budget {
    /// An empty budget under `limits`.
    #[must_use]
    pub fn new(limits: Limits) -> Budget {
        Budget {
            limits,
            entries: 0,
            bytes: 0,
        }
    }

    /// Counts one entry of `size` bytes.
    ///
    /// # Errors
    /// The entry that goes over either cap.
    pub fn admit(&mut self, entry: &str, size: u64) -> Result<(), IntegrityError> {
        self.entries += 1;
        if self.entries > self.limits.max_entries {
            return Err(IntegrityError::new(
                entry,
                format!("more than {} entries", self.limits.max_entries),
            ));
        }
        self.bytes = self.bytes.saturating_add(size);
        if self.bytes > self.limits.max_bytes {
            return Err(IntegrityError::new(
                entry,
                format!("more than {} uncompressed bytes", self.limits.max_bytes),
            ));
        }
        Ok(())
    }
}

/// Checks one entry path: relative, `/`-separated, no byte below 0x20 and
/// no 0x7f, no empty, `.` or `..` segment, no `\`, no drive or absolute
/// prefix.
///
/// # Errors
/// The reason.
pub fn check_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("the path is empty");
    }
    if path.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err("the path contains a control character");
    }
    if path.starts_with('/') {
        return Err("the path is absolute");
    }
    if path.contains('\\') {
        return Err("the path contains `\\`");
    }
    let b = path.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return Err("the path has a drive prefix");
    }
    for seg in path.split('/') {
        match seg {
            "" => return Err("the path has an empty segment"),
            "." | ".." => return Err("the path has a `.` or `..` segment"),
            _ => {}
        }
    }
    Ok(())
}

/// The simple case fold used for duplicate detection.
pub(crate) fn fold(path: &str) -> String {
    path.chars().flat_map(char::to_lowercase).collect()
}

/// Validates the entries of a staged tree or an archive
/// (`(path, kind, size)`): paths, kinds (only files and directories),
/// exact and case-fold duplicates, a file used as a directory, and the
/// caps.
///
/// # Errors
/// `package-integrity` naming the first bad entry.
pub fn validate_entries(
    entries: &[(String, EntryKind, u64)],
    limits: &Limits,
) -> Result<(), IntegrityError> {
    let mut budget = Budget::new(*limits);
    let mut folded: BTreeMap<String, &str> = BTreeMap::new();
    let mut files: BTreeSet<&str> = BTreeSet::new();
    for (path, kind, size) in entries {
        budget.admit(path, *size)?;
        check_path(path).map_err(|why| IntegrityError::new(path, why))?;
        match kind {
            EntryKind::File | EntryKind::Dir => {}
            EntryKind::Symlink => return Err(IntegrityError::new(path, "a symbolic link")),
            EntryKind::Other => {
                return Err(IntegrityError::new(path, "not a regular file or directory"))
            }
        }
        if let Some(prev) = folded.insert(fold(path), path) {
            let why = if prev == path {
                "a duplicate entry".to_string()
            } else {
                format!("duplicates `{}` under case folding", prev.escape_debug())
            };
            return Err(IntegrityError::new(path, why));
        }
        if *kind == EntryKind::File {
            files.insert(path);
        }
    }
    for (path, _, _) in entries {
        let mut at = 0;
        while let Some(i) = path[at..].find('/') {
            let parent = &path[..at + i];
            if files.contains(parent) {
                return Err(IntegrityError::new(
                    path,
                    format!("`{parent}` is a file, not a directory"),
                ));
            }
            at += i + 1;
        }
    }
    Ok(())
}

/// Normalizes manifest `assets` directories (a leading `./` and trailing
/// `/` dropped) and checks each lies strictly under the package root and
/// is not a file of `root_entries`.
///
/// # Errors
/// `package-integrity` naming the asset.
pub fn validate_assets(
    root_entries: &[(String, EntryKind, u64)],
    assets: &[Rc<str>],
) -> Result<Vec<Rc<str>>, IntegrityError> {
    let mut out = Vec::with_capacity(assets.len());
    for asset in assets {
        let mut p: &str = asset;
        while let Some(rest) = p.strip_prefix("./") {
            p = rest;
        }
        let p = p.trim_end_matches('/');
        check_path(p).map_err(|why| {
            IntegrityError::new(
                asset,
                format!("the asset is not under the package root: {why}"),
            )
        })?;
        let is_file = root_entries
            .iter()
            .any(|(e, k, _)| e == p && *k != EntryKind::Dir);
        if is_file {
            return Err(IntegrityError::new(asset, "the asset is not a directory"));
        }
        out.push(Rc::from(p));
    }
    Ok(out)
}
