//! The local-directory store and the git store. The git test runs the
//! `git` CLI against local bare repositories behind a `file://` base; it
//! FAILS (never skips) when `git` is missing.

use std::path::Path;
use std::process::Command;

use crate::pkg::cache::{fetch_and_publish, CacheBackend};
use crate::pkg::digest::tree_digest;
use crate::pkg::manifest::PkgManifest;
use crate::pkg::native::{DirStore, FsCache, GitStore};
use crate::pkg::store::{PackageStore, PkgError};
use crate::pkg::tests::support::{
    as_refs, dir_package, hashed, id, pads_files, v, write_tree, TempDir, PADS,
};
use crate::types::diag::DiagCode;

fn require_git() {
    let ok = Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    assert!(
        ok,
        "the git store test requires a local `git` binary on PATH; install git (this test is never skipped)"
    );
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=vactrol-test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn git_store_over_a_local_bare_repository() {
    require_git();
    let tmp = TempDir::new();
    let bare = tmp.join("repos").join(PADS);
    std::fs::create_dir_all(&bare).expect("mkdir");
    git(&bare, &["init", "--quiet", "--bare"]);
    let work = tmp.join("work");
    std::fs::create_dir_all(&work).expect("mkdir");
    git(&work, &["init", "--quiet"]);
    let v10 = pads_files();
    write_tree(&work, &as_refs(&v10));
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "--quiet", "-m", "v1.0.0"]);
    git(&work, &["tag", "v1.0.0"]);
    let mut v11 = pads_files();
    v11[1].1 = b"let warm 4\n".to_vec();
    write_tree(&work, &as_refs(&v11));
    git(&work, &["commit", "--quiet", "-am", "v1.1.0"]);
    git(&work, &["tag", "-a", "v1.1.0", "-m", "v1.1.0"]);
    git(&work, &["tag", "latest"]);
    let bare_str = bare.to_str().expect("utf-8 temp path");
    git(&work, &["push", "--quiet", bare_str, "main", "--tags"]);

    let base = format!("file://{}", tmp.join("repos").to_str().expect("utf-8"));
    let mut store = GitStore::new(&base);
    assert_eq!(
        store.list_versions(&id(PADS)).expect("ls-remote"),
        vec![v("v1.0.0"), v("v1.1.0")]
    );
    let m = store.manifest(&id(PADS), &v("v1.1.0")).expect("manifest");
    assert_eq!(m.package.expect("meta").path, id(PADS));

    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let d =
        fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v1.1.0"), None).expect("fetch");
    // `.git/` never enters the digest: it equals the digest of the sources.
    assert_eq!(d, tree_digest(&hashed(&as_refs(&v11))));
    let src = cache
        .read_verified(&id(PADS), &v("v1.1.0"), d)
        .expect("verified");
    assert!(src.files.iter().all(|(p, _)| !p.starts_with(".git")));
    let warm = src
        .files
        .iter()
        .find(|(p, _)| &**p == "pads.vact")
        .expect("pads.vact");
    assert_eq!(&*warm.1, b"let warm 4\n");

    let e = fetch_and_publish(&mut store, &mut cache, &id(PADS), &v("v9.0.0"), None)
        .expect_err("no tag");
    assert_eq!(e.code(), DiagCode::PackageResolve);
}

#[test]
fn dir_store_lists_versions_and_reads_manifests() {
    let tmp = TempDir::new();
    let root = tmp.join("store");
    dir_package(&root, PADS, "v1.0.0", &as_refs(&pads_files()));
    dir_package(&root, PADS, "v1.2.0", &[("pads.vact", b"let warm 1\n")]);
    dir_package(&root, PADS, "junk", &[("x", b"")]);
    dir_package(
        &root,
        "github.com/someone/vactrol-padsx",
        "v9.0.0",
        &[("x", b"")],
    );
    let mut store = DirStore::new(&root);
    assert_eq!(
        store.list_versions(&id(PADS)).expect("list"),
        vec![v("v1.0.0"), v("v1.2.0")]
    );
    assert_eq!(
        store.manifest(&id(PADS), &v("v1.2.0")),
        Ok(PkgManifest::default())
    );
    let m = store.manifest(&id(PADS), &v("v1.0.0")).expect("manifest");
    assert_eq!(m.package.expect("meta").assets.len(), 1);
    let e = store
        .list_versions(&id("github.com/nobody/none"))
        .expect_err("absent");
    assert!(matches!(e, PkgError::Unresolvable(_)), "{e:?}");
    let e = store.manifest(&id(PADS), &v("v3.0.0")).expect_err("absent");
    assert!(matches!(e, PkgError::Unresolvable(_)), "{e:?}");
    // A manifest whose path is not the import path.
    let other = "github.com/someone/other";
    let text = format!("[package]\npath = \"{PADS}\"\n");
    dir_package(&root, other, "v1.0.0", &[("vactrol.toml", text.as_bytes())]);
    let e = store
        .manifest(&id(other), &v("v1.0.0"))
        .expect_err("mismatch");
    assert_eq!(e.code(), DiagCode::PackageLoadFailed);
}
