//! `ProxyStore` over the loopback HTTP fixture: list, manifest, fetch,
//! validate, publish; the lock digest equals the `DirStore` digest.

use std::collections::BTreeMap;

use crate::pkg::cache::{fetch_and_publish, get_all};
use crate::pkg::load::locked_sources;
use crate::pkg::manifest::parse;
use crate::pkg::mvs::mvs_resolve;
use crate::pkg::native::{DirStore, FsCache};
use crate::pkg::proxy::{ProxyStore, ProxyTransport};
use crate::pkg::store::{PackageStore, PkgError};
use crate::pkg::tests::http_fixture::{HttpFixture, LocalClient};
use crate::pkg::tests::support::{
    as_refs, deps_manifest, dir_package, id, pads_files, proxy_entries, v, TempDir, DRUMS, PADS,
};
use crate::types::diag::DiagCode;

fn pads_v11() -> Vec<(String, Vec<u8>)> {
    let mut files = pads_files();
    files[0].1 = format!(
        "[package]\npath = \"{PADS}\"\nassets = [\"samples\"]\n\n[deps]\n\"{DRUMS}\" = \"v0.2.0\"\n"
    )
    .into_bytes();
    files[1].1 = b"let warm 4\n".to_vec();
    files
}

fn drums() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "vactr.toml".into(),
            deps_manifest(Some(DRUMS), &[]).into_bytes(),
        ),
        ("drums.vact".into(), b"let kick 1\n".to_vec()),
    ]
}

fn fixture() -> HttpFixture {
    let mut files = BTreeMap::new();
    files.insert(
        format!("/{PADS}/@v/list"),
        b"v1.0.0\nv1.1.0\nnot-a-tag\n".to_vec(),
    );
    files.insert(format!("/{DRUMS}/@v/list"), b"v0.2.0\n".to_vec());
    for (path, ver, fs) in [
        (PADS, "v1.0.0", pads_files()),
        (PADS, "v1.1.0", pads_v11()),
        (DRUMS, "v0.2.0", drums()),
    ] {
        for (url, body) in proxy_entries(path, ver, &as_refs(&fs)) {
            files.insert(url, body);
        }
    }
    HttpFixture::start(files)
}

#[test]
fn proxy_store_over_the_local_http_fixture() {
    let http = fixture();
    let mut store = ProxyStore::new(&http.base(), LocalClient);
    assert_eq!(
        store.list_versions(&id(PADS)).expect("list"),
        vec![v("v1.0.0"), v("v1.1.0")]
    );
    let m = store.manifest(&id(PADS), &v("v1.1.0")).expect("manifest");
    assert_eq!(m.deps, vec![(id(DRUMS), v("v0.2.0"))]);

    // The MVS walk reads manifests only, never zips.
    let root = parse(&deps_manifest(None, &[(PADS, "v1.1.0")])).expect("root");
    let selected = mvs_resolve(&root.deps, &mut store).expect("mvs");
    assert_eq!(
        selected,
        vec![(id(DRUMS), v("v0.2.0")), (id(PADS), v("v1.1.0"))]
    );
    let zips = |h: &HttpFixture| {
        h.requests
            .lock()
            .expect("log")
            .iter()
            .filter(|p| p.ends_with(".zip"))
            .count()
    };
    assert_eq!(zips(&http), 0);

    let tmp = TempDir::new();
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let lock = get_all(&root, &mut store, &mut cache).expect("get over the proxy");
    assert_eq!(zips(&http), 2);
    for e in &lock.entries {
        let src = locked_sources(&lock, &cache, &e.id).expect("verified");
        assert_eq!(src.id, e.id);
    }

    // The same sources through a local directory store give the same digests.
    dir_package(&tmp.join("store"), PADS, "v1.1.0", &as_refs(&pads_v11()));
    dir_package(&tmp.join("store"), DRUMS, "v0.2.0", &as_refs(&drums()));
    let mut dir = DirStore::new(&tmp.join("store"));
    let mut other = FsCache::new(&tmp.join("cache-dir")).expect("cache");
    for e in &lock.entries {
        let d = fetch_and_publish(&mut dir, &mut other, &e.id, &e.version, Some(e.sha256));
        assert_eq!(d, Ok(e.sha256), "{}", e.id.0);
    }
}

#[test]
fn not_found_and_non_loopback_urls() {
    let http = fixture();
    let mut store = ProxyStore::new(&http.base(), LocalClient);
    let e = store
        .list_versions(&id("github.com/nobody/none"))
        .expect_err("404");
    assert!(matches!(e, PkgError::Unresolvable(_)), "{e:?}");
    assert_eq!(e.code(), DiagCode::PackageResolve);
    let e = store.manifest(&id(PADS), &v("v9.0.0")).expect_err("404");
    assert_eq!(e.code(), DiagCode::PackageResolve);
    let e = LocalClient
        .get("https://example.com/x")
        .expect_err("refused");
    assert!(matches!(e, PkgError::Network(_)), "{e:?}");
}
