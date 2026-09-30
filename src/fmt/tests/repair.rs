//! Behavioral coverage for conservative space-indent repair.

use super::super::repair::space_indent;
use super::{directive_signature, trivia_signature};
use crate::fmt::{format, Outcome};
use crate::reader::import::AliasEnv;
use crate::reader::span::FileId;
use crate::reader::{print_all, read};
use crate::types::diag::{DiagCode, Severity};

const TAB_FIXTURES: [&str; 3] = [
    include_str!("fixtures/blocks.in"),
    include_str!("fixtures/continuation.in"),
    include_str!("fixtures/directives.in"),
];

#[test]
fn two_and_four_space_fixtures_repair_like_the_tab_sources() {
    let tab_source = TAB_FIXTURES.concat();
    let tab_result = format(&tab_source);
    let tab_read = read(&tab_result.text, FileId::new(1), &AliasEnv::new());
    assert!(!has_errors(&tab_read.diags));

    for (name, input, expected) in [
        (
            "space2",
            include_str!("fixtures/space2.in"),
            include_str!("fixtures/space2.out"),
        ),
        (
            "space4",
            include_str!("fixtures/space4.in"),
            include_str!("fixtures/space4.out"),
        ),
    ] {
        let result = format(input);
        assert_eq!(result.outcome, Outcome::Changed, "{name}");
        assert_eq!(result.text, tab_result.text, "{name}");
        assert_eq!(result.text, expected, "{name}");

        let parsed = read(&result.text, FileId::new(1), &AliasEnv::new());
        assert!(!has_errors(&parsed.diags), "{name}");
        assert_eq!(
            print_all(&parsed.nodes),
            print_all(&tab_read.nodes),
            "{name}"
        );
        assert_eq!(
            trivia_signature(&result.text, &parsed.trivia),
            trivia_signature(&tab_result.text, &tab_read.trivia),
            "trivia changed for {name}"
        );
        assert_eq!(
            directive_signature(&result.text, &parsed.nodes, &parsed.trivia),
            directive_signature(&tab_result.text, &tab_read.nodes, &tab_read.trivia),
            "directive binding changed for {name}"
        );

        let second = format(&result.text);
        assert_eq!(second.outcome, Outcome::Unchanged, "{name}");
        assert_eq!(second.text, result.text, "{name}");
    }
}

#[test]
fn small_space_indentation_and_continuation_are_converted_to_tabs() {
    let block = format("fn f a:\n  let b 1\n  + a b\n");
    assert_eq!(block.outcome, Outcome::Changed);
    assert_eq!(block.text, "fn f a:\n\tlet b 1\n\t+ a b\n");

    let continuation = format("x\n    > f 1\n");
    assert_eq!(continuation.outcome, Outcome::Changed);
    assert_eq!(continuation.text, "x\n\t> f 1\n");
}

#[test]
fn ambiguous_mixed_and_other_error_inputs_are_refused_unchanged() {
    for (name, input) in [
        (
            "space-ambiguous",
            include_str!("fixtures/space-ambiguous.in"),
        ),
        (
            "space-mixed-lines",
            include_str!("fixtures/space-mixed-lines.in"),
        ),
        (
            "space-other-error",
            include_str!("fixtures/space-other-error.in"),
        ),
        ("mixed-indent", include_str!("fixtures/mixed-indent.in")),
        ("one-space", "fn f a:\n let b 1\n"),
        ("ten-space", "fn f a:\n          let b 1\n"),
    ] {
        assert_refused_unchanged(name, input);
    }
}

#[test]
fn repair_helper_rejects_reader_clean_tab_input() {
    let source = "fn f a:\n\tlet b 1\n";
    let parsed = read(source, FileId::new(1), &AliasEnv::new());
    assert!(!has_errors(&parsed.diags));
    assert_eq!(space_indent(source, &parsed.diags), None);
}

fn assert_refused_unchanged(name: &str, input: &str) {
    let parsed = read(input, FileId::new(1), &AliasEnv::new());
    assert!(has_errors(&parsed.diags), "{name} must have reader errors");
    assert!(
        parsed
            .diags
            .iter()
            .any(|diagnostic| diagnostic.code == DiagCode::IndentSpace),
        "{name} must report indent-space"
    );

    let result = format(input);
    assert_eq!(result.outcome, Outcome::Refused, "{name}");
    assert_eq!(result.text, input, "{name}");
    assert_eq!(result.diags, parsed.diags, "{name}");
    assert!(
        result.diags.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Error && diagnostic.code == DiagCode::IndentSpace
        }),
        "{name} must retain the original diagnostics"
    );
}

fn has_errors(diags: &[crate::types::diag::Diagnostic]) -> bool {
    diags
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
