//! Statement grammar: the canonical expectations and the negative examples.

use super::{codes, printed, rd, reads_as, recovers};

#[test]
fn canonical_expectations() {
    let table = [
        ("+ 43 32 > * 12 > print", "(print (* (+ 43 32) 12))"),
        ("* 12 {+ 43 32}", "(* 12 {(+ 43 32)})"),
        ("amp: 0.5", "[:amp 0.5]"),
        ("\"a\": 1", "[\"a\" 1]"),
        ("[amp: 0.5 pan: -1]", "[[:amp 0.5] [:pan -1]]"),
        (
            "once {s :crash} at: 4 gain: 0.5",
            "(once {(s :crash)} [:at 4] [:gain 0.5])",
        ),
        ("let a 12", "(let a 12)"),
        ("a > fn1 12 > fn2 32 43", "(fn2 (fn1 a 12) 32 43)"),
        (
            "fn some-fn a:\n\tfn1 a 12\n\t\t> fn2 32 43",
            "(fn some-fn a {(fn2 (fn1 a 12) 32 43)})",
        ),
        ("d :gain ? 1.0 > * 2", "(* (#? (d :gain) 1.0) 2)"),
        ("x b -> * x b", "(-> (x b) (* x b))"),
        ("map arr {x -> * x 2}", "(map arr (-> (x) (* x 2)))"),
        (
            "if {> a 10}:\n\tprint \"big\"",
            "(if {(> a 10)} {(print \"big\")})",
        ),
        (
            "if {> a 10} {print \"big\"}",
            "(if {(> a 10)} {(print \"big\")})",
        ),
        ("print -x", "(print (#neg x))"),
        (
            "\"note {n} at beat {beat}\"",
            "(#interp \"note \" n \" at beat \" beat)",
        ),
        ("for i 0..8:\n\tprint i", "(for i (.. 0 8) {(print i)})"),
        (
            "let label if {> a 10} \"big\" \"small\"",
            "(let label (if {(> a 10)} \"big\" \"small\"))",
        ),
    ];
    for (src, expected) in table {
        reads_as(src, expected);
    }
}

#[test]
fn head_position_gt_is_greater_than() {
    reads_as("> a 10", "(> a 10)");
    reads_as("{> a 10}", "{(> a 10)}");
    reads_as("filter arr {x -> > x 20}", "(filter arr (-> (x) (> x 20)))");
}

#[test]
fn bindings_read_the_rest_of_the_line() {
    reads_as("let big: int64 1", "(let [:big int64] 1)");
    reads_as("let [a b] arr", "(let [a b] arr)");
    reads_as(
        "let line note [base {+ base 7}] > s :pluck",
        "(let line (s (note [base {(+ base 7)}]) :pluck))",
    );
    reads_as("upd hits {+ hits 1}", "(upd hits {(+ hits 1)})");
    reads_as("let f x -> * x 2", "(let f (-> (x) (* x 2)))");
    assert_eq!(codes("let x"), ["binding-without-value"]);
    assert_eq!(codes("var"), ["binding-without-value"]);
}

#[test]
fn fn_reads_a_flat_item_list() {
    reads_as("fn kick-sound: :bd-haus", "(fn [:kick-sound :bd-haus])");
    reads_as(
        "fn pluck pitch: int amp: float = 1.0 -> voice:\n\tnote pitch",
        "(fn pluck [:pitch int] [:amp float] = 1.0 -> voice {(note pitch)})",
    );
}

#[test]
fn arrows_splats_ranges_and_pairs() {
    reads_as(
        "x ->:\n\tlet y {* x 2}\n\t+ y 1",
        "(-> (x) {(let y {(* x 2)}) (+ y 1)})",
    );
    reads_as("-> 1", "(-> () 1)");
    reads_as("a -> b -> c", "(-> (a) (-> (b) c))");
    reads_as("once {s :crash} & p", "(once {(s :crash)} (& p))");
    reads_as("put d & other", "(put d (& other))");
    reads_as("take 0.. 3", "(take (.. 0) 3)");
    reads_as(".. 0 8", "(.. 0 8)");
    reads_as("print -{f x}", "(print (#neg {(f x)}))");
    reads_as("- x 1", "(- x 1)");
    reads_as("1: \"one\"", "[1 \"one\"]");
    reads_as(
        "x if {> x 100} -> \"big\"",
        "(-> (x if {(> x 100)}) \"big\")",
    );
    reads_as(
        ":kick | :snare -> \"drum\"",
        "(-> (:kick | :snare) \"drum\")",
    );
    reads_as("[first & rest] -> first", "(-> ([first (& rest)]) first)");
    reads_as("print nil true false _", "(print nil true false _)");
    reads_as(
        "slot :drums gain: 0.8:\n\ts [:bd :sd]",
        "(slot :drums [:gain 0.8] {(s [:bd :sd])})",
    );
    reads_as("d :gain ? 1.0", "(#? (d :gain) 1.0)");
    reads_as("a ? b ? c", "(#? (#? a b) c)");
    reads_as("a > f ? 1", "(#? (f a) 1)");
    reads_as("[> a b]", "[> a b]");
    reads_as("[]", "[]");
}

#[test]
fn if_chains_group_in_order() {
    reads_as(
        "if {> a 10}:\n\tprint \"big\"\nelif {> a 5}:\n\tprint \"medium\"\nelse:\n\tprint \"small\"",
        "(#if-chain (if {(> a 10)} {(print \"big\")}) (elif {(> a 5)} {(print \"medium\")}) (else {(print \"small\")}))",
    );
    reads_as(
        "if {d :gain} g ->:\n\tonce {s :bd} gain: g\nelse:\n\tonce {s :bd}",
        "(#if-chain (-> (if {(d :gain)} g) {(once {(s :bd)} [:gain g])}) (else {(once {(s :bd)})}))",
    );
    // A stray `else` stays alone; a second `else` does not join a closed chain.
    reads_as("else:\n\tx", "(else {x})");
    reads_as(
        "if a:\n\tx\nelse:\n\ty\nelse:\n\tz",
        "(#if-chain (if a {x}) (else {y}))\n(else {z})",
    );
    // Chains form inside blocks too.
    reads_as(
        "fn f k:\n\tif {<= k 1}:\n\t\t1\n\telse:\n\t\t2",
        "(fn f k {(#if-chain (if {(<= k 1)} {1}) (else {2}))})",
    );
}

#[test]
fn negative_examples_recover() {
    recovers("( + 1 2 )", "paren-form");
    recovers("let _tmp 1", "bad-identifier");
    recovers("let foo- 1", "bad-identifier");
    recovers("print _1", "console-register-in-file");
    let console = crate::reader::read(
        "print _1\nprint 1\n",
        crate::reader::span::FileId::CONSOLE,
        &crate::reader::AliasEnv::new(),
    );
    assert!(console.diags.is_empty());
    assert_eq!(
        crate::reader::print_all(&console.nodes),
        "(print _1)\n(print 1)"
    );
}

#[test]
fn each_reader_code_is_produced() {
    let table = [
        ("\"abc", "unterminated-string"),
        ("print \"a\\qb\"", "bad-escape"),
        ("amp:0.5", "misplaced-colon"),
        ("[amp: ]", "bad-pair"),
        ("once gain: > d1", "bad-pair"),
        ("? 1", "misplaced-fallback"),
        ("f a ?", "misplaced-fallback"),
        ("print {+ 1", "unclosed-group"),
        ("print [1 2", "unclosed-bracket"),
        ("print {}", "empty-group"),
        ("print 1st", "bad-number"),
        ("print 1/0", "bad-number"),
        ("print 99999999999999999999", "bad-number"),
        ("swap!", "stray-char"),
        ("print 1 }", "stray-char"),
        ("print \u{e9}", "stray-char"),
        ("[a -> b]", "misplaced-arrow"),
        ("[a ? b]", "misplaced-fallback"),
        ("f &", "misplaced-splat"),
        ("x ->", "arrow-without-body"),
        ("print 1 >", "empty-pipe"),
        ("let my_name 1", "bad-identifier"),
    ];
    for (src, code) in table {
        recovers(src, code);
    }
}

#[test]
fn nesting_cap_is_128() {
    let deep = |n: usize| format!("print {}x{}", "{".repeat(n), "}".repeat(n));
    assert!(codes(&deep(128)).is_empty());
    recovers(&deep(129), "nesting-too-deep");
    let lists = format!("print {}{}", "[".repeat(129), "]".repeat(129));
    recovers(&lists, "nesting-too-deep");
    let strings = format!("print {}x{}", "\"{".repeat(129), "}\"".repeat(129));
    assert!(codes(&strings).contains(&"nesting-too-deep"));
    // A pipe chain far longer than any real one stays bounded.
    let pipes = format!("a{}", " > f".repeat(5000));
    recovers(&pipes, "nesting-too-deep");
    let arrows = format!("{}x", "a -> ".repeat(5000));
    recovers(&arrows, "nesting-too-deep");
}

#[test]
fn a_bad_line_does_not_spoil_its_neighbours() {
    let r = rd("print 1\nprint (2)\nprint 3\n");
    assert_eq!(super::print_all(&r.nodes), "(print 1)\n(#error)\n(print 3)");
    assert_eq!(r.diags.len(), 1);
    assert!(r.nodes[1].contains_error());
    assert!(!r.nodes[0].contains_error());
    assert_eq!(printed("print 1  # trailing"), "(print 1)");
}
