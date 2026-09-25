//! The proxy store (design 5.7, 14.5.7): Go-proxy-shaped URLs over a
//! `ProxyTransport`. `{base}/{path}/@v/list` lists the tags,
//! `{base}/{path}/@v/{version}.toml` is the manifest (so an MVS walk never
//! downloads a zip) and `{base}/{path}/@v/{version}.zip` the sources, whose
//! entry paths are relative to the package root. TASK-009 has no native
//! HTTPS client; tests drive the store over a local HTTP fixture.

use crate::ns::pkg::PackageId;
use crate::pkg::manifest::PkgManifest;
use crate::pkg::semver::Version;
use crate::pkg::store::{parse_manifest, PackageStore, PkgError, StagingSink};
use crate::pkg::validate::Limits;
use crate::pkg::zip::read_zip;

/// Fetches a URL's body.
pub trait ProxyTransport {
    /// The body of a successful GET of `url`.
    ///
    /// # Errors
    /// `Unresolvable` for a not-found, `Network` for any other failure.
    fn get(&mut self, url: &str) -> Result<Vec<u8>, PkgError>;
}

/// A package store over a proxy.
#[derive(Debug)]
pub struct ProxyStore<T> {
    /// The proxy URL, without a trailing `/`.
    pub base: String,
    pub transport: T,
    pub limits: Limits,
}

impl<T> ProxyStore<T> {
    /// A store over the proxy at `base`.
    #[must_use]
    pub fn new(base: &str, transport: T) -> ProxyStore<T> {
        ProxyStore {
            base: base.trim_end_matches('/').to_string(),
            transport,
            limits: Limits::default(),
        }
    }

    fn url(&self, id: &PackageId, file: &str) -> String {
        format!("{}/{}/@v/{file}", self.base, id.0)
    }
}

impl<T: ProxyTransport> PackageStore for ProxyStore<T> {
    fn list_versions(&mut self, id: &PackageId) -> Result<Vec<Version>, PkgError> {
        let url = self.url(id, "list");
        let body = self.transport.get(&url)?;
        let text = String::from_utf8(body)
            .map_err(|_| PkgError::Network(format!("`{url}` is not UTF-8 text")))?;
        let mut out: Vec<Version> = text
            .lines()
            .filter_map(|l| Version::parse_tag(l.trim()))
            .collect();
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn manifest(&mut self, id: &PackageId, version: &Version) -> Result<PkgManifest, PkgError> {
        let body = self
            .transport
            .get(&self.url(id, &format!("{version}.toml")))?;
        parse_manifest(id, &body)
    }

    fn fetch_into(
        &mut self,
        id: &PackageId,
        version: &Version,
        staging: &mut dyn StagingSink,
    ) -> Result<(), PkgError> {
        let body = self
            .transport
            .get(&self.url(id, &format!("{version}.zip")))?;
        for (path, bytes) in read_zip(&body, &self.limits)? {
            staging.write(&path, &bytes)?;
        }
        Ok(())
    }
}
