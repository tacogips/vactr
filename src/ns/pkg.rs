//! Package namespaces and import bindings (design 5.7).
//!
//! The package LOADING pipeline (fetch, lock, read -> expand -> check ->
//! compile into the package) is TASK-009's. This module owns the namespace
//! side: one child namespace per package, the alias table, and qualified
//! lookup (`pads.warm`).

use std::rc::Rc;

use crate::ns::namespace::{Namespace, Prelude};
use crate::value::intern::SymId;

/// A package path such as `github.com/owner/vactrol-pads`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PackageId(pub Rc<str>);

impl PackageId {
    /// A package id from its path.
    #[must_use]
    pub fn new(path: &str) -> PackageId {
        PackageId(Rc::from(path))
    }
}

/// One child namespace per imported package. Its parent is the prelude,
/// never the importing session.
#[derive(Debug)]
pub struct PkgNs {
    pub id: PackageId,
    pub ns: Namespace,
}

impl PkgNs {
    /// An empty package namespace over the shared prelude.
    #[must_use]
    pub fn new(id: PackageId, prelude: Rc<Prelude>) -> PkgNs {
        PkgNs {
            id,
            ns: Namespace::with_prelude(prelude),
        }
    }
}

/// `import PATH [as ALIAS] [open]`: the prefix a package is bound to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportBinding {
    pub prefix: SymId,
    pub pkg: PackageId,
    pub open: bool,
}
