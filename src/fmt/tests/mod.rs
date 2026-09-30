//! Shared formatter guarantees and fixture registry.

mod corpus;
mod mutation;
mod rules;

use crate::directives::attach::{attach, Doc};
use crate::reader::import::AliasEnv;
use crate::reader::node::{Trivia, TriviaKind};
use crate::reader::span::FileId;
use crate::reader::{print_all, read};
use crate::types::diag::Severity;
use crate::types::HostManifest;

pub(super) const FIXTURES: &[(&str, &str)] = &[
    ("continuation", include_str!("fixtures/continuation.in")),
    ("blocks", include_str!("fixtures/blocks.in")),
    ("pairs", include_str!("fixtures/pairs.in")),
    ("lambda", include_str!("fixtures/lambda.in")),
    ("interp", include_str!("fixtures/interp.in")),
    ("comments", include_str!("fixtures/comments.in")),
    ("directives", include_str!("fixtures/directives.in")),
    ("blank-lines", include_str!("fixtures/blank-lines.in")),
    ("crlf", include_str!("fixtures/crlf.in")),
    ("empty", include_str!("fixtures/empty.in")),
    ("mixed-indent", include_str!("fixtures/mixed-indent.in")),
    ("reader-error", include_str!("fixtures/reader-error.in")),
];

/// Checks formatter guarantees G1–G7 for a reader-clean input, and G1–G2
/// for a refused input.
pub(super) fn check_guarantees(src: &str) {
    let file = FileId::new(1);
    let formatted = crate::fmt::format(src);
    let parsed = read(src, file, &AliasEnv::new());
    let refused = parsed
        .diags
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    if refused {
        assert_eq!(formatted.outcome, crate::fmt::Outcome::Refused);
        assert_eq!(formatted.text, src);
        return;
    }

    assert_ne!(formatted.outcome, crate::fmt::Outcome::Refused);
    let reparsed = read(&formatted.text, file, &AliasEnv::new());
    assert!(!reparsed
        .diags
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error));
    assert_eq!(print_all(&reparsed.nodes), print_all(&parsed.nodes));

    let fixed = crate::fmt::format(&formatted.text);
    assert_eq!(fixed.text, formatted.text);
    assert_eq!(fixed.outcome, crate::fmt::Outcome::Unchanged);

    assert_eq!(
        trivia_signature(src, &parsed.trivia),
        trivia_signature(&formatted.text, &reparsed.trivia)
    );
    assert_eq!(
        directive_signature(src, &parsed.nodes, &parsed.trivia),
        directive_signature(&formatted.text, &reparsed.nodes, &reparsed.trivia)
    );
    assert_line_changes_only_layout(src, &formatted.text);
}

fn trivia_signature(src: &str, trivia: &Trivia) -> Vec<(TriviaKind, String)> {
    trivia
        .items
        .iter()
        .map(|item| {
            let text = src
                .get(item.span.start as usize..item.span.end as usize)
                .unwrap_or("");
            let text = if item.kind == TriviaKind::Directive {
                text.to_owned()
            } else {
                text.trim_end_matches([' ', '\t']).to_owned()
            };
            (item.kind, text)
        })
        .collect()
}

fn directive_signature(
    src: &str,
    nodes: &[crate::reader::node::Node],
    trivia: &Trivia,
) -> Vec<(Option<usize>, bool)> {
    let doc = Doc::new(src, FileId::new(1), nodes, &HostManifest::spec_default());
    let (placed, _) = attach(src, &doc, trivia, &mut Vec::new());
    placed
        .iter()
        .map(|directive| (directive.attach, directive.trailing))
        .collect()
}

fn assert_line_changes_only_layout(input: &str, output: &str) {
    let input_lines: Vec<&str> = input.split_inclusive('\n').collect();
    let output_lines: Vec<&str> = output.split_inclusive('\n').collect();
    assert!(output_lines.len() <= input_lines.len());
    for (before, after) in input_lines.iter().zip(&output_lines) {
        assert_eq!(
            line_core(before),
            line_core(after),
            "line differs beyond layout"
        );
    }
    for removed in input_lines.iter().skip(output_lines.len()) {
        assert!(
            line_core(removed).is_empty(),
            "only trailing blank lines may be removed"
        );
    }
}

fn line_core(line: &str) -> &str {
    let body = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line);
    body.trim_start_matches([' ', '\t'])
        .trim_end_matches([' ', '\t'])
}
