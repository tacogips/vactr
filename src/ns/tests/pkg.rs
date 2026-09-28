//! Package namespaces (design 5.7): qualified names through `PkgNs`, the
//! lookup order and re-import replacement.

use std::rc::Rc;

use crate::ns::namespace::{FormGen, SlotKind};
use crate::ns::pkg::{ImportBinding, PackageId, PkgNs};
use crate::value::intern::intern_sym;
use crate::value::value::Value;
use crate::vm::tests::Sess;

const PADS: &str = "github.com/someone/vactr-pads";

fn pkg(s: &Sess, path: &str, names: &[(&str, i32)]) -> Rc<PkgNs> {
    let p = PkgNs::new(PackageId::new(path), Rc::clone(s.ns.prelude()));
    for (name, v) in names {
        p.ns.define(
            intern_sym(name),
            SlotKind::Let,
            Value::Int(*v),
            FormGen::new(0),
        );
    }
    Rc::new(p)
}

fn import(s: &mut Sess, prefix: &str, path: &str, open: bool, p: Rc<PkgNs>) {
    s.ns.import(
        ImportBinding {
            prefix: intern_sym(prefix),
            pkg: PackageId::new(path),
            open,
        },
        p,
    );
    s.aliases.bind(Rc::from(prefix), Rc::from(path));
}

#[test]
fn a_qualified_name_compiles_through_the_package_namespace() {
    let mut s = Sess::new();
    let p = pkg(&s, PADS, &[("warm", 3)]);
    import(&mut s, "pads", PADS, false, p);
    assert_eq!(s.show("pads.warm"), "3");
    assert_eq!(s.show("+ pads.warm 1"), "4");
    // Not opened: the bare name is not visible.
    assert_eq!(s.show("warm"), "fail: undefined-name");
    assert_eq!(s.show("pads.cold"), "fail: undefined-name");
}

#[test]
fn lookup_order_locals_session_opens_most_recent_prelude() {
    let mut s = Sess::new();
    let a = pkg(&s, "github.com/a/one", &[("warm", 1), ("len", 99)]);
    import(&mut s, "one", "github.com/a/one", true, a);
    assert_eq!(s.show("warm"), "1");
    // An open import comes before the prelude.
    assert_eq!(s.show("len"), "99");
    let b = pkg(&s, "github.com/b/two", &[("warm", 2)]);
    import(&mut s, "two", "github.com/b/two", true, b);
    // The most recent open import wins; the qualified spelling stays.
    assert_eq!(s.show("warm"), "2");
    assert_eq!(s.show("one.warm"), "1");
    // The session comes before every import.
    s.eval("let warm 0").expect("session");
    assert_eq!(s.show("warm"), "0");
    // Locals come first of all.
    assert_eq!(s.show("fn f warm:\n\t+ warm 0\nf 7"), "7");
}

#[test]
fn re_import_replaces_the_whole_package_namespace() {
    let mut s = Sess::new();
    let old = pkg(&s, PADS, &[("warm", 1), ("gone", 5)]);
    import(&mut s, "pads", PADS, true, old);
    assert_eq!(s.show("pads.gone"), "5");
    let new = pkg(&s, PADS, &[("warm", 7)]);
    import(&mut s, "pads", PADS, true, new);
    assert_eq!(s.show("pads.warm"), "7");
    assert_eq!(s.show("warm"), "7");
    assert_eq!(s.show("pads.gone"), "fail: undefined-name");
    assert_eq!(s.show("gone"), "fail: undefined-name");
}

// Package evaluation (`PkgNs::load`, design 14.5.7 "Loading").

use crate::pkg::semver::Version;
use crate::pkg::store::PkgSources;
use crate::reader::span::FileId;
use crate::reader::ImportDecl;
use crate::types::diag::{DiagCode, Diagnostic, Severity};
use crate::types::manifest::HostManifest;

const PKG_FILE: u32 = 0x2000_0000;

fn sources(path: &str, files: &[(&str, &str)]) -> PkgSources {
    let files = files
        .iter()
        .map(|(p, t)| (Rc::from(*p), Rc::from(t.as_bytes())))
        .collect();
    PkgSources::from_files(PackageId::new(path), Version::new(1, 0, 0), files, None)
        .expect("sources")
}

/// Loads `src` into a `PkgNs`; returns it, its diagnostics and the file
/// names in `FileId` order.
fn load_with(
    s: &mut Sess,
    src: &PkgSources,
    imports: &mut dyn FnMut(&ImportDecl) -> Option<Rc<PkgNs>>,
) -> (Rc<PkgNs>, Vec<Diagnostic>, Vec<String>) {
    let mut names = Vec::new();
    let mut next_file = |p: &str| {
        names.push(p.to_string());
        FileId::new(PKG_FILE + u32::try_from(names.len()).expect("few files"))
    };
    let (p, diags) = PkgNs::load(
        src.id.clone(),
        Rc::clone(s.ns.prelude()),
        &mut s.vm,
        src,
        &HostManifest::spec_default(),
        &mut next_file,
        imports,
    );
    (p, diags, names)
}

fn load(s: &mut Sess, src: &PkgSources) -> (Rc<PkgNs>, Vec<Diagnostic>, Vec<String>) {
    load_with(s, src, &mut |_| None)
}

#[test]
fn a_fixture_package_loads_and_exposes_its_names_under_the_prefix() {
    let mut s = Sess::new();
    let src = sources(
        PADS,
        &[
            (
                "vactr.toml",
                "[package]\npath = \"github.com/someone/vactr-pads\"\n",
            ),
            ("b.vact", "let warm + base 2\n"),
            ("a.vact", "let base 1\n"),
            ("notes.txt", "not source"),
        ],
    );
    let (p, diags, names) = load(&mut s, &src);
    assert!(diags.is_empty(), "{diags:?}");
    // Bytewise path order: `a.vact` defines what `b.vact` reads.
    assert_eq!(names, ["a.vact", "b.vact"]);
    assert_eq!(p.id, PackageId::new(PADS));
    import(&mut s, "pads", PADS, false, p);
    assert_eq!(s.show("pads.warm"), "3");
    assert_eq!(s.show("warm"), "fail: undefined-name");
}

#[test]
fn a_package_compile_error_is_package_load_failed_at_the_package_file() {
    let mut s = Sess::new();
    let src = sources(
        PADS,
        &[("a.vact", "let warm 3\n"), ("b.vact", "let bad nope\n")],
    );
    let (p, diags, names) = load(&mut s, &src);
    assert_eq!(names, ["a.vact", "b.vact"]);
    let b = FileId::new(PKG_FILE + 2);
    let failed: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| d.code == DiagCode::PackageLoadFailed)
        .collect();
    assert_eq!(failed.len(), 1, "{diags:?}");
    assert_eq!(failed[0].span.file, b);
    assert!(failed[0].message.contains(PADS), "{}", failed[0].message);
    assert!(
        diags.iter().any(|d| d.code == DiagCode::UndefinedName
            && d.severity == Severity::Error
            && d.span.file == b),
        "{diags:?}"
    );
    // The load continued: the good form is in the package.
    import(&mut s, "pads", PADS, false, p);
    assert_eq!(s.show("pads.warm"), "3");
    assert_eq!(s.show("pads.bad"), "fail: undefined-name");
}

#[test]
fn re_loading_a_package_replaces_its_namespace() {
    let mut s = Sess::new();
    let v1 = sources(PADS, &[("a.vact", "let warm 1\nlet gone 5\n")]);
    let (p, diags, _) = load(&mut s, &v1);
    assert!(diags.is_empty(), "{diags:?}");
    import(&mut s, "pads", PADS, true, p);
    assert_eq!(s.show("pads.gone"), "5");
    let v2 = sources(PADS, &[("a.vact", "let warm 7\n")]);
    let (p, _, _) = load(&mut s, &v2);
    import(&mut s, "pads", PADS, true, p);
    assert_eq!(s.show("pads.warm"), "7");
    assert_eq!(s.show("warm"), "7");
    assert_eq!(s.show("pads.gone"), "fail: undefined-name");
}

#[test]
fn a_package_import_resolves_through_the_callers_resolver() {
    let mut s = Sess::new();
    let base_path = "github.com/someone/vactr-base";
    let (base, diags, _) = load(&mut s, &sources(base_path, &[("b.vact", "let x 40\n")]));
    assert!(diags.is_empty(), "{diags:?}");
    let src = sources(
        PADS,
        &[(
            "a.vact",
            "import github.com/someone/vactr-base\nlet warm + base.x 2\n",
        )],
    );
    let mut asked = Vec::new();
    let (p, diags, _) = load_with(&mut s, &src, &mut |d| {
        asked.push(d.path.to_string());
        Some(Rc::clone(&base))
    });
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(asked, [base_path]);
    import(&mut s, "pads", PADS, false, p);
    assert_eq!(s.show("pads.warm"), "42");
    // An unresolved import stays bound-but-broken: the qualified use is a
    // package failure, not a reader error.
    let (_, diags, _) = load(&mut s, &src);
    assert!(
        diags.iter().any(|d| d.code == DiagCode::PackageLoadFailed),
        "{diags:?}"
    );
    assert!(
        diags.iter().all(|d| d.code != DiagCode::UnboundQualifier),
        "{diags:?}"
    );
}
