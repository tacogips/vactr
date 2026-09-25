//! The git store (design 14.5.7). It runs the `git` binary with fixed argv
//! arrays (no shell), `--` before operands, `GIT_TERMINAL_PROMPT=0`,
//! `GIT_CONFIG_NOSYSTEM=1`, `-c core.hooksPath=/dev/null` and
//! `-c protocol.file.allow=user`. Tags come from `ls-remote --tags`; a
//! version is a shallow single-branch clone of its tag, whose `.git/` is
//! removed before validation and never enters the digest.
//!
//! `<base>` is a constructor argument: the CLI passes `https://github.com`
//! style bases; the tests pass a `file://` URL of local bare repositories.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::ns::pkg::PackageId;
use crate::pkg::manifest::PkgManifest;
use crate::pkg::native::dir_store::write_tree;
use crate::pkg::native::{io, ScratchDir};
use crate::pkg::semver::Version;
use crate::pkg::store::{parse_manifest, PackageStore, PkgError, StagingSink, MANIFEST_FILE};
use crate::pkg::validate::Limits;

/// A store over git repositories at `<base>/<path>`.
#[derive(Clone, Debug)]
pub struct GitStore {
    base: String,
    pub limits: Limits,
}

impl GitStore {
    /// A store over repositories under `base` (a URL without a trailing
    /// `/`, e.g. `https://` + host, or `file:///tmp/repos`).
    #[must_use]
    pub fn new(base: &str) -> GitStore {
        GitStore {
            base: base.trim_end_matches('/').to_string(),
            limits: Limits::default(),
        }
    }

    fn url(&self, id: &PackageId) -> String {
        format!("{}/{}", self.base, id.0)
    }

    /// Runs `git <args>` and returns its stdout.
    fn git(&self, args: &[&str]) -> Result<Vec<u8>, PkgError> {
        let out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "protocol.file.allow=user",
            ])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| PkgError::Network(format!("cannot run `git`: {e}")))?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(PkgError::Network(format!(
                "`git {}` failed: {}",
                args.first().unwrap_or(&""),
                err.trim()
            )))
        }
    }

    /// A shallow clone of the tag of `version` into `dir`, without `.git/`.
    fn checkout(&self, id: &PackageId, version: &Version, dir: &Path) -> Result<(), PkgError> {
        let tag = version.to_string();
        let url = self.url(id);
        let target = dir
            .to_str()
            .ok_or_else(|| PkgError::Io(format!("`{}` is not a UTF-8 path", dir.display())))?;
        self.git(&[
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--branch",
            &tag,
            "--single-branch",
            "--",
            &url,
            target,
        ])?;
        let git = dir.join(".git");
        fs::remove_dir_all(&git).map_err(|e| io("cannot remove", &git, &e))
    }
}

impl PackageStore for GitStore {
    fn list_versions(&mut self, id: &PackageId) -> Result<Vec<Version>, PkgError> {
        let out = self.git(&["ls-remote", "--tags", "--", &self.url(id)])?;
        let text = String::from_utf8_lossy(&out);
        let mut versions: Vec<Version> = text
            .lines()
            .filter_map(|l| l.split('\t').nth(1)?.strip_prefix("refs/tags/"))
            .filter(|t| !t.ends_with("^{}"))
            .filter_map(Version::parse_tag)
            .collect();
        versions.sort();
        versions.dedup();
        Ok(versions)
    }

    fn manifest(&mut self, id: &PackageId, version: &Version) -> Result<PkgManifest, PkgError> {
        let scratch = ScratchDir::new("vactrol-git-")?;
        let dir = scratch.path().join("src");
        self.checkout(id, version, &dir)?;
        let file = dir.join(MANIFEST_FILE);
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
        let scratch = ScratchDir::new("vactrol-git-")?;
        let dir = scratch.path().join("src");
        self.checkout(id, version, &dir)?;
        write_tree(&dir, &[], &self.limits, staging)
    }
}
