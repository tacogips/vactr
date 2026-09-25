//! Each expander code, with the span of its originating source node.

use super::*;

use crate::reader::span::Span;
use crate::types::diag::DiagCode;

fn check(src: &str, code: DiagCode, origin: Span) {
    let d = expand_err(src);
    assert_eq!(d.code, code, "{src:?}: {d}");
    assert_eq!(d.span, origin, "{src:?}: origin span of {code}");
}

/// The span of the whole first line.
fn line1(src: &str) -> Span {
    span_of(src, src.lines().next().expect("a line"))
}

#[test]
fn malformed_if() {
    check("if c", DiagCode::MalformedIf, span_of("if c", "if c"));
    check(
        "if a b c d",
        DiagCode::MalformedIf,
        span_of("if a b c d", "if a b c d"),
    );
    let src = "if a b c\nelse:\n\td";
    check(src, DiagCode::MalformedIf, line1(src));
    let src = "if a:\n\tb\nelse c d";
    check(src, DiagCode::MalformedIf, span_of(src, "else c d"));
    check(
        "if s -> x",
        DiagCode::MalformedIf,
        span_of("if s -> x", "if s -> x"),
    );
}

#[test]
fn else_without_if() {
    let src = "print 1\nelse:\n\tx";
    let d = expand_err(src);
    assert_eq!(d.code, DiagCode::ElseWithoutIf);
    assert_eq!(d.span.start, span_of(src, "else").start);
    check(
        "elif c x",
        DiagCode::ElseWithoutIf,
        span_of("elif c x", "elif c x"),
    );
    let src = "f {else x}";
    check(src, DiagCode::ElseWithoutIf, span_of(src, "else x"));
}

#[test]
fn if_guard() {
    let src = "if s x if c -> 1";
    let at = src.rfind("if").expect("guard");
    let at = u32::try_from(at).expect("small");
    check(src, DiagCode::IfGuard, Span::new(FILE, at, at + 2));
}

#[test]
fn malformed_for() {
    let src = "for x arr print x";
    check(src, DiagCode::MalformedFor, line1(src));
    check("for x", DiagCode::MalformedFor, span_of("for x", "for x"));
}

#[test]
fn malformed_match() {
    let src = "match x";
    check(src, DiagCode::MalformedMatch, line1(src));
    let src = "match x:\n\tprint 1";
    check(src, DiagCode::MalformedMatch, span_of(src, "print 1"));
    let src = "match x:\n\tx if a b -> 1";
    check(src, DiagCode::MalformedMatch, span_of(src, "x if a b -> 1"));
    let src = "match x:\n\t-> 1";
    check(src, DiagCode::MalformedMatch, span_of(src, "-> 1"));
}

#[test]
fn malformed_fn() {
    // U4: an inline body reads as a pair, so the name is not a Sym.
    let src = "fn kick-sound: :bd-haus";
    check(src, DiagCode::MalformedFn, line1(src));
    let src = "fn f a b";
    check(src, DiagCode::MalformedFn, line1(src));
}

#[test]
fn malformed_binding() {
    let src = "let 1 2";
    check(src, DiagCode::MalformedBinding, line1(src));
    let src = "let \"a\": 1 2";
    check(src, DiagCode::MalformedBinding, line1(src));
    let src = "upd [a] 1";
    check(src, DiagCode::MalformedBinding, line1(src));
    let src = "f {let a}";
    check(src, DiagCode::MalformedBinding, span_of(src, "let a"));
}

#[test]
fn malformed_enum() {
    let src = "enum shape";
    check(src, DiagCode::MalformedEnum, line1(src));
    let src = "enum shape:\n\ta -> b";
    check(src, DiagCode::MalformedEnum, span_of(src, "a -> b"));
}

#[test]
fn sugar_in_pattern() {
    let src = "map arr {-x -> x}";
    check(src, DiagCode::SugarInPattern, span_of(src, "-x"));
    let src = "let [a {b ? c}] d";
    check(src, DiagCode::SugarInPattern, span_of(src, "b ? c"));
    let src = "for \"{x}\" arr:\n\tprint x";
    check(src, DiagCode::SugarInPattern, span_of(src, "\"{x}\""));
    let src = "fn f -a:\n\ta";
    check(src, DiagCode::SugarInPattern, span_of(src, "-a"));
    let src = "if s [-x] -> 1";
    check(src, DiagCode::SugarInPattern, span_of(src, "-x"));
    let src = "enum e:\n\tv -x";
    check(src, DiagCode::SugarInPattern, span_of(src, "-x"));
}

#[test]
fn reserved_word() {
    let src = "let match 1";
    check(src, DiagCode::ReservedWord, span_of(src, "match"));
    let src = "var nil 1";
    check(src, DiagCode::ReservedWord, span_of(src, "nil"));
    let src = "upd for 1";
    check(src, DiagCode::ReservedWord, span_of(src, "for"));
    let src = "fn if x:\n\tx";
    check(src, DiagCode::ReservedWord, span_of(src, "if"));
    let src = "fn f else:\n\t1";
    check(src, DiagCode::ReservedWord, span_of(src, "else"));
    let src = "fn f let: int:\n\t1";
    check(src, DiagCode::ReservedWord, span_of(src, "let"));
    let src = "let [a enum] d";
    check(src, DiagCode::ReservedWord, span_of(src, "enum"));
    let src = "for import arr:\n\tx";
    check(src, DiagCode::ReservedWord, span_of(src, "import"));
    let src = "enum var:\n\ta";
    check(src, DiagCode::ReservedWord, span_of(src, "var"));
}

#[test]
fn struct_and_inst_are_not_reserved() {
    expands_as("let struct 1", "(let struct 1)");
    expands_as("let inst 1", "(let inst 1)");
    expands_as("while x", "(while x)");
}

#[test]
fn misplaced_splat() {
    let src = "let a & p";
    check(src, DiagCode::MisplacedSplat, span_of(src, "& p"));
    let src = "f {& p}";
    check(src, DiagCode::MisplacedSplat, span_of(src, "& p"));
    let src = "map arr {a & b -> a}";
    check(src, DiagCode::MisplacedSplat, span_of(src, "& b"));
}

#[test]
fn read_error_present() {
    let src = "print _tmp 1";
    let r = rd(src);
    assert!(!r.diags.is_empty());
    let mut cx = ExpandCx::new(r.next_node_id());
    let d = expand(&r.nodes[0], &mut cx).expect_err("error node");
    assert_eq!(d.code, DiagCode::ReadErrorPresent);
    assert_eq!(d.span, r.nodes[0].span);
}

#[test]
fn deep_nesting_is_a_diagnostic_not_a_panic() {
    let src = format!("a{}", " > f".repeat(600));
    let r = rd(&src);
    let mut cx = ExpandCx::new(r.next_node_id());
    let d = expand(&r.nodes[0], &mut cx).expect_err("too deep");
    assert_eq!(d.code, DiagCode::NestingTooDeep);
}

#[test]
fn every_prefix_expands_without_panic() {
    let src = "if {d :gain} g ->:\n\tonce {s :bd} gain: -g\nelif x:\n\tfor [k v] d:\n\t\tprint \"{k}\" v ? 1\nelse:\n\tmatch y:\n\t\tz if {> z 1} -> 1\nenum e:\n\ta b\n";
    for end in 0..=src.len() {
        let Some(prefix) = src.get(..end) else {
            continue;
        };
        let r = rd(prefix);
        let mut cx = ExpandCx::new(r.next_node_id());
        for n in &r.nodes {
            if let Ok(out) = expand(n, &mut cx) {
                assert!(is_kernel(&out), "{prefix:?}");
            }
        }
    }
}
