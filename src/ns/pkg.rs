//! Package namespaces, import bindings and package evaluation (design
//! 5.7, 14.5.7 "Loading").
//!
//! One child namespace per package, the alias table, and qualified lookup
//! (`pads.warm`). `PkgNs::load` runs a package's verified sources through
//! read -> expand -> check -> compile -> run into a fresh `PkgNs`, like
//! `ns/load.rs` runs a loaded file; fetching and the lock are `crate::pkg`.

use std::rc::Rc;

use crate::compile::{compile, CompileCx};
use crate::expand::{expand, ExpandCx};
use crate::ns::namespace::{FormGen, Namespace, Prelude};
use crate::pkg::store::PkgSources;
use crate::reader::span::{FileId, Span};
use crate::reader::{prescan_imports, read, AliasEnv, ImportDecl};
use crate::types::check::check;
use crate::types::diag::{DiagCode, Diagnostic, Severity};
use crate::types::manifest::HostManifest;
use crate::value::intern::{intern_sym, SymId};
use crate::vm::vm::Vm;

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

    /// Evaluates a package's verified sources into a fresh `PkgNs` (a
    /// re-import replaces the whole namespace). The `.vact` files run in
    /// bytewise path order, each with its own `FileId` from `next_file`, so
    /// the package's diagnostics sit at the package's own files. A file's
    /// `import` forms bind their prefixes for its qualified names and are
    /// resolved through `imports` (the caller resolves them through the
    /// same lock); an unresolved one stays bound-but-broken. A failing form
    /// is recorded and the load continues; any reader, expander, check,
    /// compile or run error adds one `package-load-failed` at the first
    /// failure. Effects of the successful forms stay staged on `vm`.
    pub fn load(
        id: PackageId,
        prelude: Rc<Prelude>,
        vm: &mut Vm,
        sources: &PkgSources,
        manifest: &HostManifest,
        next_file: &mut dyn FnMut(&str) -> FileId,
        imports: &mut dyn FnMut(&ImportDecl) -> Option<Rc<PkgNs>>,
    ) -> (Rc<PkgNs>, Vec<Diagnostic>) {
        let pkg = PkgNs::new(id, prelude);
        let mut diags = Vec::new();
        let mut failures: Vec<(Span, String)> = Vec::new();
        let mut gen = 0u64;
        let observer = vm.take_read_observer();
        for (path, bytes) in &sources.files {
            if !path.ends_with(".vact") {
                continue;
            }
            let file = next_file(path);
            let Ok(text) = std::str::from_utf8(bytes) else {
                failures.push((Span::new(file, 0, 0), format!("`{path}` is not UTF-8")));
                continue;
            };
            let mut aliases = AliasEnv::new();
            for decl in prescan_imports(text) {
                aliases.bind(Rc::clone(&decl.prefix), Rc::clone(&decl.path));
                if let Some(dep) = imports(&decl) {
                    let binding = ImportBinding {
                        prefix: intern_sym(&decl.prefix),
                        pkg: PackageId(Rc::clone(&decl.path)),
                        open: decl.open,
                    };
                    pkg.ns.import(binding, dep);
                }
            }
            let r = read(text, file, &aliases);
            let mut ecx = ExpandCx::new(r.next_node_id());
            let mut forms = Vec::new();
            let note = |d: &Diagnostic, failures: &mut Vec<(Span, String)>| {
                if d.severity == Severity::Error {
                    failures.push((d.span, format!("`{path}`: {}", d.message)));
                }
            };
            for d in r.diags {
                note(&d, &mut failures);
                diags.push(d);
            }
            for node in &r.nodes {
                match expand(node, &mut ecx) {
                    Ok(form) => forms.push(form),
                    Err(d) => {
                        note(&d, &mut failures);
                        diags.push(d);
                    }
                }
            }
            for d in check(&forms, &pkg.ns.check_env(), manifest).diags {
                note(&d, &mut failures);
                diags.push(d);
            }
            for form in &forms {
                gen += 1;
                let mut cx = CompileCx::new(&pkg.ns, FormGen::new(gen));
                cx.tweak_sites = false;
                let proto = match compile(form, &mut cx) {
                    Ok(proto) => proto,
                    Err(d) => {
                        note(&d, &mut failures);
                        diags.push(d);
                        continue;
                    }
                };
                diags.append(&mut cx.diags);
                if let Err(e) = vm.run(proto, &pkg.ns) {
                    failures.push((form.span, format!("`{path}`: {e}")));
                }
            }
        }
        vm.set_read_observer(observer);
        if let Some((span, _)) = failures.first() {
            let reasons: Vec<&str> = failures.iter().map(|(_, m)| m.as_str()).collect();
            diags.push(Diagnostic::error(
                DiagCode::PackageLoadFailed,
                *span,
                format!(
                    "package `{}` failed to load: {}",
                    pkg.id.0,
                    reasons.join("; ")
                ),
            ));
        }
        (Rc::new(pkg), diags)
    }
}

/// `import PATH [as ALIAS] [open]`: the prefix a package is bound to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportBinding {
    pub prefix: SymId,
    pub pkg: PackageId,
    pub open: bool,
}
