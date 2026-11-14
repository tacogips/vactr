//! The canonical digest: record layout, portability (a directory tree and
//! a zip of the same sources give the same digest) and injectivity
//! (Astra's newline-name counterexample).

use std::rc::Rc;

use crate::pkg::cache::fetch_and_publish;
use crate::pkg::digest::{canonical_bytes, hex, parse_hex, sources_digest, tree_digest};
use crate::pkg::native::{DirStore, FsCache};
use crate::pkg::proxy::ProxyStore;
use crate::pkg::sha256::sha256;
use crate::pkg::store::PkgError;
use crate::pkg::tests::support::{
    as_refs, dir_package, hashed, id, pads_files, proxy_entries, v, MapTransport, TempDir, PADS,
};
use crate::pkg::validate::{validate_entries, EntryKind, Limits};
use crate::pkg::zip::read_zip;
use crate::types::diag::DiagCode;

#[test]
fn records_are_length_prefixed_and_sorted() {
    let h = [9u8; 32];
    let bytes = canonical_bytes(&[(Rc::from("b"), [1; 32]), (Rc::from("a/x"), h)]);
    let mut expect = vec![0, 0, 0, 3];
    expect.extend_from_slice(b"a/x");
    expect.extend_from_slice(&h);
    expect.extend_from_slice(&[0, 0, 0, 1, b'b']);
    expect.extend_from_slice(&[1; 32]);
    assert_eq!(bytes, expect);
    assert_eq!(tree_digest(&[]), sha256(b""));
    let d = sha256(b"x");
    assert_eq!(parse_hex(&hex(&d)), Some(d));
    assert_eq!(parse_hex(&hex(&d).to_uppercase()), None);
}

#[test]
fn portability_a_directory_tree_and_a_zip_give_the_same_digest() {
    let files = pads_files();
    let refs = as_refs(&files);
    let tmp = TempDir::new();
    dir_package(&tmp.join("store"), PADS, "v1.0.0", &refs);
    let mut dir_store = DirStore::new(&tmp.join("store"));
    let mut cache_a = FsCache::new(&tmp.join("cache-a")).expect("cache");
    let from_dir = fetch_and_publish(&mut dir_store, &mut cache_a, &id(PADS), &v("v1.0.0"), None)
        .expect("dir store publishes");

    let mut t = MapTransport::default();
    for (path, body) in proxy_entries(PADS, "v1.0.0", &refs) {
        t.0.insert(format!("http://proxy{path}"), body);
    }
    let mut proxy = ProxyStore::new("http://proxy", t);
    let mut cache_b = FsCache::new(&tmp.join("cache-b")).expect("cache");
    let from_zip = fetch_and_publish(&mut proxy, &mut cache_b, &id(PADS), &v("v1.0.0"), None)
        .expect("zip store publishes");

    assert_eq!(from_dir, from_zip);
    assert_eq!(from_dir, tree_digest(&hashed(&refs)));
}

/// The WITHDRAWN newline-record serialization, to show the fixture is the
/// real counterexample.
fn newline_records(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut sorted = files.to_vec();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut out = Vec::new();
    for (p, b) in sorted {
        out.extend_from_slice(format!("{p}\n{}\n", hex(&sha256(b))).as_bytes());
    }
    out
}

#[test]
fn injectivity_the_newline_name_tree_is_rejected_and_its_bytes_differ() {
    let (x, y): (&[u8], &[u8]) = (b"let warm 3\n", b"let cold 4\n");
    let honest: Vec<(&str, &[u8])> = vec![("a", x), ("b", y)];
    let forged_name = format!("a\n{}\nb", hex(&sha256(x)));
    let forged: Vec<(&str, &[u8])> = vec![(forged_name.as_str(), y)];
    // Under the withdrawn serialization the two trees collide.
    assert_eq!(newline_records(&honest), newline_records(&forged));

    // Validation rejects the forged tree, naming the entry ...
    let entries = vec![(forged_name.clone(), EntryKind::File, y.len() as u64)];
    let e = validate_entries(&entries, &Limits::default()).expect_err("a control character");
    assert_eq!(e.entry, forged_name);
    assert_eq!(PkgError::from(e).code(), DiagCode::PackageIntegrity);
    // ... as does the zip reader ...
    let zip = crate::pkg::tests::support::zip_of(&forged);
    let e = read_zip(&zip, &Limits::default()).expect_err("rejected");
    assert_eq!(e.entry, forged_name);

    // ... and, independently, the canonical bytes and digests differ.
    assert_ne!(
        canonical_bytes(&hashed(&honest)),
        canonical_bytes(&hashed(&forged))
    );
    assert_ne!(tree_digest(&hashed(&honest)), tree_digest(&hashed(&forged)));
    let as_sources = |fs: &[(&str, &[u8])]| -> Vec<(Rc<str>, Rc<[u8]>)> {
        fs.iter()
            .map(|(p, b)| (Rc::from(*p), Rc::from(*b)))
            .collect()
    };
    assert_ne!(
        sources_digest(&as_sources(&honest)),
        sources_digest(&as_sources(&forged))
    );
}
