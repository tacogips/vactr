//! The browser package driver (design 15.1.2 G6, 15.1.10): resolves or
//! restores packages over a JS-supplied [`Prefetched`] transport, wholly
//! in memory and with no I/O of its own. The editor drives it step by
//! step (design `command.md` `pkg_resolve`/`pkg_supply`): each [`drive`]
//! call re-runs the whole resolve or restore using whatever bodies have
//! been supplied so far, and stops at the first URL the host has not
//! fetched, or fails with the code its `PkgError` maps to.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;

use crate::pkg::cache::{fetch_and_publish, get_all, CacheBackend};
use crate::pkg::digest::hex;
use crate::pkg::lock::LockFile;
use crate::pkg::manifest::{package_id, ManifestError, PkgManifest};
use crate::pkg::proxy::{ProxyStore, ProxyTransport};
use crate::pkg::semver::{latest_release, Version};
use crate::pkg::store::{PackageStore, PkgError};

/// A `ProxyTransport` whose bodies are supplied by the host instead of
/// fetched: `url -> Ok(bytes)` for a successful (`200`) response, `Err`
/// with the status for a not-found (`404`) or any other failed response.
/// `get` of a URL with no supplied body records it as missing (the first
/// one per [`drive`] call) and fails, so the in-flight resolve or restore
/// is abandoned and its staging already discarded by the existing
/// `fetch_and_publish`/`get_all` pipeline (14.5.7).
#[derive(Default, Debug)]
pub struct Prefetched {
    bodies: BTreeMap<String, Result<Vec<u8>, u16>>,
    missing: Option<String>,
}

impl Prefetched {
    /// An empty transport.
    #[must_use]
    pub fn new() -> Prefetched {
        Prefetched::default()
    }

    /// Supplies a successful (`200`) response body for `url`.
    pub fn supply(&mut self, url: impl Into<String>, body: Vec<u8>) {
        self.bodies.insert(url.into(), Ok(body));
    }

    /// Supplies a failed response for `url`: `404` or any other status.
    pub fn supply_status(&mut self, url: impl Into<String>, status: u16) {
        self.bodies.insert(url.into(), Err(status));
    }

    fn fetch(&mut self, url: &str) -> Result<Vec<u8>, PkgError> {
        match self.bodies.get(url) {
            Some(Ok(bytes)) => Ok(bytes.clone()),
            Some(Err(status)) => Err(status_error(url, *status)),
            None => {
                if self.missing.is_none() {
                    self.missing = Some(url.to_string());
                }
                Err(PkgError::Network(format!("`{url}`: not yet supplied")))
            }
        }
    }
}

fn status_error(url: &str, status: u16) -> PkgError {
    if status == 404 {
        PkgError::Unresolvable(format!("`{url}`: not found (404)"))
    } else {
        PkgError::Network(format!("`{url}`: status {status}"))
    }
}

impl ProxyTransport for Prefetched {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, PkgError> {
        self.fetch(url)
    }
}

impl ProxyTransport for &mut Prefetched {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, PkgError> {
        self.fetch(url)
    }
}

/// One step of the package driver (design `command.md` `pkg_resolve`).
#[derive(Clone, Debug)]
pub enum DriverRequest {
    /// Resolves root `requirements` (package path -> version, or `""` for
    /// the latest release) against `proxy` and fetches every selected
    /// package.
    Resolve {
        proxy: String,
        requirements: BTreeMap<String, String>,
    },
    /// Re-fetches and re-verifies every entry of a previously rendered
    /// `lock` against `proxy`.
    Restore { proxy: String, lock: String },
}

/// The `0x73` reply (design `command.md`).
#[derive(Clone, Debug)]
pub enum DriverReply {
    /// The host must fetch `url` and supply it before the next call.
    Need { url: String },
    /// Every package resolved and published; `lock` is the rendered lock
    /// text, `resolved` one `(path, version, sha256 hex)` per entry.
    Done {
        lock: String,
        resolved: Vec<(String, String, String)>,
    },
    /// The step failed; `code` is the `PkgError`'s diagnostic code.
    Error { code: String, message: String },
}

type Resolved = Vec<(String, String, String)>;

/// Runs one driver step: `Resolve` builds a root manifest from
/// `requirements` (an empty version means the latest release) and runs
/// [`get_all`]; `Restore` re-fetches and re-verifies every entry of the
/// parsed `lock` with [`fetch_and_publish`]. Both run over a
/// `ProxyStore<&mut Prefetched>`; the driver itself performs no I/O.
///
/// On the first URL `supplied` has no body for, the reply is [`Need`],
/// and the pipeline has already discarded its staging (14.5.7). Any
/// other failure maps to [`Error`] with the `PkgError`'s diagnostic code.
///
/// [`Need`]: DriverReply::Need
/// [`Error`]: DriverReply::Error
#[must_use]
pub fn drive(
    req: DriverRequest,
    supplied: &mut Prefetched,
    cache: &mut dyn CacheBackend,
) -> DriverReply {
    supplied.missing = None;
    let outcome = match req {
        DriverRequest::Resolve {
            proxy,
            requirements,
        } => resolve(&proxy, &requirements, supplied, cache),
        DriverRequest::Restore { proxy, lock } => restore(&proxy, &lock, supplied, cache),
    };
    if let Some(url) = supplied.missing.take() {
        return DriverReply::Need { url };
    }
    match outcome {
        Ok((lock, resolved)) => DriverReply::Done { lock, resolved },
        Err(e) => DriverReply::Error {
            code: e.code().as_str().to_string(),
            message: e.to_string(),
        },
    }
}

fn resolve(
    proxy: &str,
    requirements: &BTreeMap<String, String>,
    supplied: &mut Prefetched,
    cache: &mut dyn CacheBackend,
) -> Result<(String, Resolved), PkgError> {
    let mut store = ProxyStore::new(proxy, &mut *supplied);
    let mut manifest = PkgManifest::default();
    for (path, version) in requirements {
        let pkg = package_id(path).map_err(|m| PkgError::Manifest(ManifestError::at(0, m)))?;
        let version = if version.is_empty() {
            let versions = store.list_versions(&pkg)?;
            latest_release(&versions)
                .cloned()
                .ok_or_else(|| PkgError::Unresolvable(format!("`{path}` has no release version")))?
        } else {
            Version::parse_tag(version).ok_or_else(|| {
                PkgError::Manifest(ManifestError::at(
                    0,
                    format!("`{version}` is not a `vMAJOR.MINOR.PATCH` version"),
                ))
            })?
        };
        manifest.add_or_raise(pkg, version);
    }
    let lock = get_all(&manifest, &mut store, cache)?;
    Ok(lock_reply(&lock))
}

fn restore(
    proxy: &str,
    lock_text: &str,
    supplied: &mut Prefetched,
    cache: &mut dyn CacheBackend,
) -> Result<(String, Resolved), PkgError> {
    let lock = LockFile::parse(lock_text)?;
    let mut store = ProxyStore::new(proxy, &mut *supplied);
    for entry in &lock.entries {
        fetch_and_publish(
            &mut store,
            cache,
            &entry.id,
            &entry.version,
            Some(entry.sha256),
        )?;
    }
    Ok(lock_reply(&lock))
}

fn lock_reply(lock: &LockFile) -> (String, Resolved) {
    let resolved = lock
        .entries
        .iter()
        .map(|e| (e.id.0.to_string(), e.version.to_string(), hex(&e.sha256)))
        .collect();
    (lock.render(), resolved)
}

impl DriverReply {
    /// The `0x73` reply JSON (design `command.md`): `{"status":"need",
    /// "url"}`, `{"status":"done","lock","resolved":[{"path","version",
    /// "sha256"}]}` or `{"status":"error","code","message"}`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let value = match self {
            DriverReply::Need { url } => json!({"status": "need", "url": url}),
            DriverReply::Done { lock, resolved } => {
                let resolved: Vec<serde_json::Value> = resolved
                    .iter()
                    .map(|(path, version, sha256)| {
                        json!({"path": path, "version": version, "sha256": sha256})
                    })
                    .collect();
                json!({"status": "done", "lock": lock, "resolved": resolved})
            }
            DriverReply::Error { code, message } => {
                json!({"status": "error", "code": code, "message": message})
            }
        };
        value.to_string()
    }
}

/// The `pkg_resolve` request JSON (design `command.md`): `{"proxy",
/// "requirements"}` or `{"proxy","lock"}` (restore).
#[derive(Deserialize)]
struct RawRequest {
    proxy: String,
    #[serde(default)]
    requirements: Option<BTreeMap<String, String>>,
    #[serde(default)]
    lock: Option<String>,
}

impl DriverRequest {
    /// Parses the `pkg_resolve` ABI JSON.
    ///
    /// # Errors
    /// Malformed JSON, or not exactly one of `requirements`/`lock`.
    pub fn from_json(text: &str) -> Result<DriverRequest, String> {
        let raw: RawRequest = serde_json::from_str(text).map_err(|e| e.to_string())?;
        match (raw.requirements, raw.lock) {
            (Some(requirements), None) => Ok(DriverRequest::Resolve {
                proxy: raw.proxy,
                requirements,
            }),
            (None, Some(lock)) => Ok(DriverRequest::Restore {
                proxy: raw.proxy,
                lock,
            }),
            _ => Err("expected exactly one of `requirements` or `lock`".to_string()),
        }
    }
}
