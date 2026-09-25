//! Package versions: git tags `vMAJOR.MINOR.PATCH[-pre]` with semver 2.0
//! precedence (design 14.5.7). Build metadata (`+meta`) and every other
//! tag shape are not versions.

use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

/// A package version.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// The prerelease identifiers after `-`, dot-separated.
    pub pre: Option<Rc<str>>,
}

impl Version {
    /// A release version.
    #[must_use]
    pub fn new(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
            pre: None,
        }
    }

    /// Parses a tag `v1.2.3` or `v1.2.3-pre.1`; any other tag is `None`.
    #[must_use]
    pub fn parse_tag(tag: &str) -> Option<Version> {
        let rest = tag.strip_prefix('v')?;
        let (core, pre) = match rest.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (rest, None),
        };
        let mut parts = core.split('.');
        let major = numeric(parts.next()?)?;
        let minor = numeric(parts.next()?)?;
        let patch = numeric(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(p) => {
                let ok = p.split('.').all(|id| {
                    !id.is_empty()
                        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        && (!id.bytes().all(|b| b.is_ascii_digit()) || numeric(id).is_some())
                });
                if !ok {
                    return None;
                }
                Some(Rc::from(p))
            }
            None => None,
        };
        Some(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    /// True for a prerelease.
    #[must_use]
    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }
}

/// A numeric identifier: ASCII digits, no leading zero (except `0`).
fn numeric(s: &str) -> Option<u64> {
    let digits = !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits || (s.len() > 1 && s.starts_with('0')) {
        return None;
    }
    s.parse().ok()
}

fn cmp_pre(a: &str, b: &str) -> Ordering {
    let mut xs = a.split('.');
    let mut ys = b.split('.');
    loop {
        match (xs.next(), ys.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let o = match (numeric(x), numeric(y)) {
                    (Some(m), Some(n)) => m.cmp(&n),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => x.cmp(y),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
        }
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => cmp_pre(a, b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// The highest non-prerelease version.
#[must_use]
pub fn latest_release(versions: &[Version]) -> Option<&Version> {
    versions.iter().filter(|v| !v.is_prerelease()).max()
}
