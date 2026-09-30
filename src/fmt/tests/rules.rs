//! Fixture and inline rule tests for the formatter contract.

use super::{check_guarantees, FIXTURES};
use crate::fmt::{
    format, format_bytes, Outcome, STATUS_FORMATTED, STATUS_NOT_UTF8, STATUS_REFUSED,
};

macro_rules! fixture_test {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            let input = include_str!(concat!("fixtures/", $name, ".in"));
            let expected = include_str!(concat!("fixtures/", $name, ".out"));
            let result = format(input);
            assert_ne!(result.outcome, Outcome::Refused, "{}", $name);
            assert_eq!(result.text, expected);
            check_guarantees(input);
        }
    };
}

fixture_test!(continuation_fixture, "continuation");
fixture_test!(blocks_fixture, "blocks");
fixture_test!(pairs_fixture, "pairs");
fixture_test!(lambda_fixture, "lambda");
fixture_test!(interpolation_fixture, "interp");
fixture_test!(comments_fixture, "comments");
fixture_test!(directives_fixture, "directives");
fixture_test!(blank_lines_fixture, "blank-lines");
fixture_test!(crlf_fixture, "crlf");
fixture_test!(empty_fixture, "empty");

#[test]
fn all_blank_input_formats_to_empty() {
    let blank = format("\n\n\t\n");
    assert_eq!(blank.text, "");
    assert_eq!(blank.outcome, Outcome::Changed);
    let empty = format("");
    assert_eq!(empty.text, "");
    assert_eq!(empty.outcome, Outcome::Unchanged);
}

#[test]
fn refused_fixtures_keep_the_exact_source_and_diagnostics() {
    for name in ["mixed-indent", "reader-error"] {
        let (input, expected) = FIXTURES
            .iter()
            .find(|(fixture, _)| *fixture == name)
            .map(|(_, input)| {
                let output = match name {
                    "mixed-indent" => include_str!("fixtures/mixed-indent.out"),
                    _ => include_str!("fixtures/reader-error.out"),
                };
                (*input, output)
            })
            .expect("required refused fixture exists");
        let result = format(input);
        assert_eq!(result.outcome, Outcome::Refused);
        assert_eq!(result.text, expected);
        assert!(!result.diags.is_empty());
        check_guarantees(input);
    }
}

#[test]
fn design_comment_level_examples_match_the_spec() {
    let cases = [
        ("inst a:\n\t# c\n\tbody\n", "inst a:\n\t# c\n\tbody\n"),
        (
            "inst a:\n\ts :bd\n\t# c\nnext\n",
            "inst a:\n\ts :bd\n\t# c\nnext\n",
        ),
        ("let a 1\n\t\t\t# c\nlet b 2\n", "let a 1\n# c\nlet b 2\n"),
        (
            "let x foo\n\t\t\t# c\n\t\t\t> bar\n",
            "let x foo\n\t# c\n\t> bar\n",
        ),
        (
            "let x foo\n\t\t\t> f:\n\t\t\t\ts\n\t\t\t\t#@ lfo\n",
            "let x foo\n\t> f:\n\t\ts\n\t\t#@ lfo\n",
        ),
        (
            "inst a:\n\ts :bd\n    # c\nnext\n",
            "inst a:\n\ts :bd\n\t# c\nnext\n",
        ),
        (
            "if true:\n\tinst a:\n\t\ts :bd\nelse:\n\t\t#@ label: x\n\tlet b 2\n",
            "if true:\n\tinst a:\n\t\ts :bd\nelse:\n\t\t#@ label: x\n\tlet b 2\n",
        ),
        (
            "if a:\n\tlet x 1\nelif b:\n\t\t#@ label: x\n\tlet y 2\n",
            "if a:\n\tlet x 1\nelif b:\n\t#@ label: x\n\tlet y 2\n",
        ),
    ];
    for (input, expected) in cases {
        let result = format(input);
        assert_ne!(result.outcome, Outcome::Refused, "{input:?}");
        assert_eq!(result.text, expected, "{input:?}");
        check_guarantees(input);
    }
}

#[test]
fn format_bytes_reports_status_and_preserves_refused_input() {
    assert_eq!(
        format_bytes(&[0xff, 0xfe]),
        (STATUS_NOT_UTF8, vec![0xff, 0xfe])
    );
    let refused = include_bytes!("fixtures/reader-error.in");
    assert_eq!(format_bytes(refused), (STATUS_REFUSED, refused.to_vec()));
    let input = include_bytes!("fixtures/continuation.in");
    let expected = include_bytes!("fixtures/continuation.out");
    assert_eq!(format_bytes(input), (STATUS_FORMATTED, expected.to_vec()));
}
