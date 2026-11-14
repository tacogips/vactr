//! Layout: levels, continuation lines, blocks, recovery.

use super::{codes, printed, reads_as};

#[test]
fn pipe_continuation_lines() {
    reads_as(
        "s [:bd-haus :sn-dub]\n\t> every 4 {p -> fast p 2}\n\t> whenmod 8 6 rev\n\t> d1",
        "(d1 (whenmod (every (s [:bd-haus :sn-dub]) 4 (-> (p) (fast p 2))) 8 6 rev))",
    );
    reads_as(
        "inst pluck freq: float = 440:\n\tsaw freq\n\t\t> lpf cutoff\n\t\t> * amp",
        "(inst pluck [:freq float] = 440 {(* (lpf (saw freq) cutoff) amp)})",
    );
    // A continuation line may open the block.
    reads_as(
        "s x\n\t> every 4:\n\t\tfast 2",
        "(every (s x) 4 {(fast 2)})",
    );
    // Continuations may sit deeper than one level.
    reads_as("a\n\t\t> f", "(f a)");
}

#[test]
fn layout_diagnostics() {
    assert_eq!(codes("let a 1\n  print a\nprint 2"), ["indent-space"]);
    assert_eq!(
        printed("let a 1\n  print a\nprint 2"),
        "(let a 1)\n(#error)\n(print 2)"
    );
    // A deeper line without `>` spoils the statement it is indented under.
    assert_eq!(codes("print 1\n\tprint 2\nprint 3"), ["unexpected-indent"]);
    assert_eq!(
        printed("print 1\n\tprint 2\nprint 3"),
        "(#error)\n(print 3)"
    );
    // Its own deeper lines go with it, and a later `>` line still continues.
    assert_eq!(
        codes("f a\n\tg:\n\t\th\n\t> k\nprint 3"),
        ["unexpected-indent"]
    );
    // With no statement to attach to, the deeper lines form their own `Error`.
    assert_eq!(codes("d1:\n\t\tx\n\t\ty\nprint 3"), ["unexpected-indent"]);
    assert_eq!(
        printed("d1:\n\t\tx\n\t\ty\nprint 3"),
        "(d1 {(#error)})\n(print 3)"
    );
    assert_eq!(codes("d1:\nprint 1"), ["empty-block"]);
    assert_eq!(printed("d1:\nprint 1"), "(#error)\n(print 1)");
    assert_eq!(
        codes("d1 x\n\t> every 4:\n\t\tfast 2\n\t> d2\nprint 1"),
        ["continuation-after-block"]
    );
    assert_eq!(
        printed("d1 x\n\t> every 4:\n\t\tfast 2\n\t> d2\nprint 1"),
        "(#error)\n(print 1)"
    );
    // Blank and comment-only lines (with any indentation) only affect trivia.
    reads_as(
        "print 1\n\n   # note\n\t\t# deeper note\nprint 2",
        "(print 1)\n(print 2)",
    );
}

#[test]
fn errors_inside_blocks_stay_local() {
    let r = super::rd("fn f:\n\tprint (x)\n\tprint 2\nprint 3");
    assert_eq!(
        super::print_all(&r.nodes),
        "(fn f {(#error) (print 2)})\n(print 3)"
    );
    assert_eq!(r.diags.len(), 1);
    // A header error still reports the diagnostics of its block.
    assert_eq!(codes("f!:\n\tprint (x)"), ["stray-char", "paren-form"]);
}

#[test]
fn block_nesting_is_capped() {
    let mut src = String::new();
    for level in 0..130 {
        src.push_str(&"\t".repeat(level));
        src.push_str("f:\n");
    }
    src.push_str(&"\t".repeat(130));
    src.push_str("x\nprint 1\n");
    let r = super::rd(&src);
    assert!(r
        .diags
        .iter()
        .any(|d| d.code.as_str() == "nesting-too-deep"));
    assert_eq!(super::print_all(&r.nodes[1..]), "(print 1)");
}

#[test]
fn crlf_line_ends() {
    reads_as(
        "print 1\r\nfn f a:\r\n\t* a 2\r\n",
        "(print 1)\n(fn f a {(* a 2)})",
    );
}
