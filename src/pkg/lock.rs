//! `vactr.lock` (design 14.5.7): the header `# vactr.lock v1`, then one
//! line `<path> <version> sha256:<64 lowercase hex>` per package, sorted
//! bytewise by path. Deterministic and line-based.

use crate::ns::pkg::PackageId;
use crate::pkg::digest::{hex, parse_hex};
use crate::pkg::manifest::{package_id, ManifestError};
use crate::pkg::semver::Version;

/// The first line of every lock file.
pub const LOCK_HEADER: &str = "# vactr.lock v1";

/// One locked package.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LockEntry {
    pub id: PackageId,
    pub version: Version,
    /// The canonical content digest.
    pub sha256: [u8; 32],
}

/// A lock file: entries sorted bytewise by path, one per path.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LockFile {
    pub entries: Vec<LockEntry>,
}

impl LockFile {
    /// A lock over `entries`, sorted by path; a later entry for the same
    /// path replaces an earlier one.
    #[must_use]
    pub fn new(entries: Vec<LockEntry>) -> LockFile {
        let mut out: Vec<LockEntry> = Vec::with_capacity(entries.len());
        for e in entries {
            match out.binary_search_by(|x| x.id.0.as_bytes().cmp(e.id.0.as_bytes())) {
                Ok(i) => out[i] = e,
                Err(i) => out.insert(i, e),
            }
        }
        LockFile { entries: out }
    }

    /// The entry of `id`.
    #[must_use]
    pub fn get(&self, id: &PackageId) -> Option<&LockEntry> {
        self.entries
            .binary_search_by(|x| x.id.0.as_bytes().cmp(id.0.as_bytes()))
            .ok()
            .map(|i| &self.entries[i])
    }

    /// Parses a lock file.
    ///
    /// # Errors
    /// An unknown header, a malformed line, a duplicate or unsorted path,
    /// with its line number.
    pub fn parse(text: &str) -> Result<LockFile, ManifestError> {
        let body = text.strip_suffix('\n').unwrap_or(text);
        if body.split('\n').next() != Some(LOCK_HEADER) {
            return Err(ManifestError::at(
                1,
                format!("the first line must be `{LOCK_HEADER}`"),
            ));
        }
        let mut entries: Vec<LockEntry> = Vec::new();
        for (i, line) in body.split('\n').enumerate().skip(1) {
            let n = i + 1;
            let err = |m: String| ManifestError::at(n, m);
            let fields: Vec<&str> = line.split(' ').collect();
            let [path, version, digest] = fields[..] else {
                return Err(err(
                    "expected `<path> <version> sha256:<hex>` separated by single spaces".into(),
                ));
            };
            let id = package_id(path).map_err(err)?;
            let version = Version::parse_tag(version).ok_or_else(|| {
                err(format!(
                    "`{}` is not a `vMAJOR.MINOR.PATCH` version",
                    version.escape_debug()
                ))
            })?;
            let sha256 = digest
                .strip_prefix("sha256:")
                .and_then(parse_hex)
                .ok_or_else(|| err("the digest must be `sha256:` and 64 lowercase hex".into()))?;
            if let Some(prev) = entries.last() {
                match prev.id.0.as_bytes().cmp(id.0.as_bytes()) {
                    std::cmp::Ordering::Less => {}
                    std::cmp::Ordering::Equal => {
                        return Err(err(format!("`{path}` appears twice")))
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(err(format!("`{path}` is out of order")))
                    }
                }
            }
            entries.push(LockEntry {
                id,
                version,
                sha256,
            });
        }
        Ok(LockFile { entries })
    }

    /// Renders the lock: the header, then the lines sorted by path.
    #[must_use]
    pub fn render(&self) -> String {
        let mut sorted: Vec<&LockEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| a.id.0.as_bytes().cmp(b.id.0.as_bytes()));
        let mut out = String::from(LOCK_HEADER);
        out.push('\n');
        for e in sorted {
            out.push_str(&format!(
                "{} {} sha256:{}\n",
                e.id.0,
                e.version,
                hex(&e.sha256)
            ));
        }
        out
    }
}
