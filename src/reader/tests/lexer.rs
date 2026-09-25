//! Lexer rules: numbers, names, strings, colon classes.

use super::{codes, reads_as};
use crate::reader::node::{Atom, NodeKind};
use crate::value::ratio::Ratio64;

fn first_arg(src: &str) -> Atom {
    let r = super::rd(src);
    assert!(r.diags.is_empty(), "{src:?}: {:?}", r.diags);
    match &r.nodes[0].children[1].kind {
        NodeKind::Atom(a) => a.clone(),
        other => panic!("{src:?}: not an atom: {other:?}"),
    }
}

#[test]
fn numbers() {
    assert_eq!(first_arg("f 12"), Atom::Int(12));
    assert_eq!(first_arg("f -3"), Atom::Int(-3));
    assert_eq!(
        first_arg("f 2.5"),
        Atom::Float {
            value: 2.5,
            exact: Ratio64::new(5, 2).ok()
        }
    );
    assert_eq!(
        first_arg("f -0.25"),
        Atom::Float {
            value: -0.25,
            exact: Ratio64::new(-1, 4).ok()
        }
    );
    assert_eq!(first_arg("f 1/4"), Atom::Ratio(Ratio64::new(1, 4).unwrap()));
    assert_eq!(
        first_arg("f -1/4"),
        Atom::Ratio(Ratio64::new(-1, 4).unwrap())
    );
    assert_eq!(first_arg("f -9223372036854775808"), Atom::Int(i64::MIN));
    reads_as("f 90.0 1.5 4/2", "(f 90.0 1.5 2/1)");
    reads_as("0..8", "(.. 0 8)");
    assert_eq!(codes("f 1."), ["bad-number"]);
    assert_eq!(codes("f 1/4/5"), ["bad-number"]);
}

#[test]
fn names() {
    reads_as("fn1 12", "(fn1 12)");
    reads_as("my-long-name", "my-long-name");
    reads_as("a->b", "(-> (a) b)");
    reads_as("f x-1", "(f x-1)");
    assert_eq!(codes("-foo"), Vec::<&str>::new());
    reads_as("-foo", "(#neg foo)");
    assert_eq!(codes("foo?"), ["misplaced-fallback"]);
    assert_eq!(codes("f _0"), ["bad-identifier"]);
    assert_eq!(codes("f a.b.c"), ["stray-char"]);
    assert_eq!(first_arg("f _"), Atom::Wildcard);
}

#[test]
fn strings_and_escapes() {
    reads_as(
        r#"print "a\"b\\c\nd\te\{f\}""#,
        r#"(print "a\"b\\c\nd\te\{f\}")"#,
    );
    reads_as(r#"print """#, r#"(print "")"#);
    reads_as(r#"print "{f "x"}""#, r#"(print (#interp (f "x")))"#);
    reads_as(r#"print "a{[1 2]}b""#, r#"(print (#interp "a" [1 2] "b"))"#);
    reads_as(
        "print \"caf\u{e9} # not a comment\"",
        "(print \"caf\u{e9} # not a comment\")",
    );
    assert_eq!(codes(r#"print "{}""#), ["empty-group"]);
    assert_eq!(codes(r#"print "{x"#), ["unterminated-string"]);
    assert_eq!(codes(r#"print "{x # y}""#), ["unterminated-string"]);
}

#[test]
fn colon_classes() {
    reads_as("d1:\n\ts [:bd :sd]", "(d1 {(s [:bd :sd])})");
    reads_as(
        "slot :drums:\n\ts [:bd :sd]",
        "(slot :drums {(s [:bd :sd])})",
    );
    reads_as("f 0.8:\n\tx", "(f 0.8 {x})");
    reads_as("f:   # comment\n\tx", "(f {x})");
    reads_as("[:c :maj7]", "[:c :maj7]");
    assert_eq!(codes("f a:b"), ["misplaced-colon"]);
    assert_eq!(codes("f :a: 1"), ["misplaced-colon"]);
    assert_eq!(codes("[:g :7]"), ["misplaced-colon"]);
    assert_eq!(codes("f :bad_kw"), ["bad-identifier"]);
}
