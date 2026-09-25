//! Packages (design 5.7, 14.5.7): Go-style repository-path imports, git-tag
//! semver versions with minimal version selection, `vactrol.toml`,
//! `vactrol.lock`, the canonical injective content digest, archive safety,
//! and staged atomic cache publication.
//!
//! Everything here except `native` (file system, `git`) is wasm-safe: no
//! file system, network, process or thread use. Fetching happens only in
//! `vactrol get` (`cache::get_all`); a running session reads the verified
//! cache (`load::locked_sources`) and evaluates the sources into a `PkgNs`
//! (`ns::pkg::PkgNs::load`).

pub mod cache;
pub mod digest;
pub mod driver;
pub mod load;
pub mod lock;
pub mod manifest;
pub mod mem_cache;
pub mod mvs;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
pub mod proxy;
pub mod semver;
pub mod sha256;
pub mod store;
pub mod validate;
pub mod zip;

#[cfg(test)]
mod tests;

pub use cache::{fetch_and_publish, get_all, CacheBackend, StagingId};
pub use digest::{canonical_bytes, tree_digest};
pub use driver::{drive, DriverReply, DriverRequest, Prefetched};
pub use load::{asset_banks, default_prefix, locked_sources};
pub use lock::{LockEntry, LockFile};
pub use manifest::{ManifestError, PackageMeta, PkgManifest};
pub use mem_cache::MemCache;
pub use mvs::mvs_resolve;
pub use proxy::{ProxyStore, ProxyTransport};
pub use semver::{latest_release, Version};
pub use store::{PackageStore, PkgError, PkgSources, StagingSink, LOCK_FILE, MANIFEST_FILE};
pub use validate::{EntryKind, IntegrityError, Limits};
