//! Package namespaces (design 5.7): qualified names through `PkgNs`, the
//! lookup order and re-import replacement.

use std::rc::Rc;

use crate::ns::namespace::{FormGen, SlotKind};
use crate::ns::pkg::{ImportBinding, PackageId, PkgNs};
use crate::value::intern::intern_sym;
use crate::value::value::Value;
use crate::vm::tests::Sess;

const PADS: &str = "github.com/someone/vactrol-pads";

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
