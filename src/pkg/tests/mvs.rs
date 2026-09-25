//! Minimal version selection over an in-memory store.

use crate::pkg::mvs::mvs_resolve;
use crate::pkg::store::PkgError;
use crate::pkg::tests::support::{id, v, MemStore};
use crate::types::diag::DiagCode;

const A: &str = "github.com/t/a";
const B: &str = "github.com/t/b";
const C: &str = "github.com/t/c";
const D: &str = "github.com/t/d";

fn store() -> MemStore {
    let mut s = MemStore::default();
    s.add_req(A, "v1.0.0", &[(C, "v1.1.0")]);
    s.add_req(B, "v1.0.0", &[(C, "v1.3.0")]);
    s.add_req(C, "v1.1.0", &[]);
    s.add_req(C, "v1.3.0", &[(D, "v0.2.0")]);
    s.add_req(C, "v1.4.0", &[]);
    s.add_req(D, "v0.2.0", &[]);
    s
}

#[test]
fn competing_requirements_pick_the_maximum_of_the_minimums() {
    let mut s = store();
    let got = mvs_resolve(&[(id(A), v("v1.0.0")), (id(B), v("v1.0.0"))], &mut s).expect("resolves");
    // C v1.3 wins over v1.1; v1.4 exists but nobody requires it. D comes
    // transitively through C v1.3.
    assert_eq!(
        got,
        vec![
            (id(A), v("v1.0.0")),
            (id(B), v("v1.0.0")),
            (id(C), v("v1.3.0")),
            (id(D), v("v0.2.0")),
        ]
    );
}

#[test]
fn a_transitive_dependency_is_selected() {
    let mut s = store();
    let got = mvs_resolve(&[(id(A), v("v1.0.0"))], &mut s).expect("resolves");
    assert_eq!(got, vec![(id(A), v("v1.0.0")), (id(C), v("v1.1.0"))]);
}

#[test]
fn an_unknown_version_is_unresolvable() {
    let mut s = store();
    s.add_req(B, "v2.0.0", &[(C, "v9.9.9")]);
    let e = mvs_resolve(&[(id(B), v("v2.0.0"))], &mut s).expect_err("no C v9.9.9");
    assert!(matches!(e, PkgError::Unresolvable(_)), "{e:?}");
    assert_eq!(e.code(), DiagCode::PackageResolve);
    assert!(e.to_string().contains("v9.9.9"), "{e}");
    let e = mvs_resolve(&[(id("github.com/t/none"), v("v1.0.0"))], &mut s).expect_err("unknown");
    assert_eq!(e.code(), DiagCode::PackageResolve);
}
