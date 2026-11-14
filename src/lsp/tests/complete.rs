use std::collections::BTreeSet;

use tower_lsp::lsp_types::{CompletionItemKind, CompletionTextEdit, Position, Url};

use crate::complete::{self, CandidateKind, ContextKind, DocName, Snapshot, MAX_LIMIT};
use crate::lsp::analysis::{Analyzer, PkgConfig};
use crate::lsp::convert::offset;
use crate::reader::node::{Node, NodeKind};
use crate::reader::span::FileId;
use crate::reader::{import, read, AliasEnv};
use crate::types::manifest::HostManifest;
use crate::types::natives::NativeTable;
use crate::types::ty::KeySet;

fn open(text: &str) -> (Analyzer, Url) {
    let mut analyzer = Analyzer::new(PkgConfig::default());
    let uri = Url::parse("file:///work/completion.vact").expect("url");
    analyzer.open(uri.clone(), 1, text.to_string());
    (analyzer, uri)
}

#[test]
fn local_parameter_is_ranked_first_and_maps_to_variable() {
    let (mut analyzer, uri) = open("fn f alpha:\n\tlet beta 1\n\tal");
    let items = analyzer.complete(&uri, Position::new(2, 3));
    assert_eq!(items.first().map(|item| item.label.as_str()), Some("alpha"));
    assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
    assert_eq!(items[0].sort_text.as_deref(), Some("0000"));
}

#[test]
fn analyzed_document_function_keeps_function_kind() {
    let (mut analyzer, uri) = open("fn sample:\n\t1\nsam");
    let items = analyzer.complete(&uri, Position::new(2, 3));
    let sample = items
        .iter()
        .find(|item| item.label == "sample")
        .expect("document function completion");
    assert_eq!(sample.kind, Some(CompletionItemKind::FUNCTION));
}

#[test]
fn keyword_candidates_are_context_specific() {
    let (mut analyzer, uri) = open("s :an");
    let items = analyzer.complete(&uri, Position::new(0, 5));
    assert!(items.iter().any(|item| item.label == ":analog"));
    assert!(items
        .iter()
        .all(|item| item.kind == Some(CompletionItemKind::KEYWORD)));
}

#[test]
fn pipe_control_is_ranked_before_other_candidates() {
    let (mut analyzer, uri) = open("s :analog > cu");
    let items = analyzer.complete(&uri, Position::new(0, 14));
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("cutoff")
    );
    assert_eq!(items[0].kind, Some(CompletionItemKind::PROPERTY));
}

#[test]
fn text_edit_range_uses_utf16_after_japanese_comment() {
    let text = "# 日本\nsi";
    let (mut analyzer, uri) = open(text);
    let items = analyzer.complete(&uri, Position::new(1, 2));
    let sine = items
        .iter()
        .find(|item| item.label == "sine")
        .expect("sine");
    let Some(CompletionTextEdit::Edit(edit)) = &sine.text_edit else {
        panic!("completion text edit expected");
    };
    assert_eq!(
        edit.range,
        tower_lsp::lsp_types::Range::new(Position::new(1, 0), Position::new(1, 2))
    );
    assert_eq!(
        offset(text, edit.range.start),
        text.find("si").expect("prefix")
    );
}

#[test]
fn empty_head_reports_incomplete_list() {
    let (mut analyzer, uri) = open("");
    assert!(
        analyzer
            .complete_list(&uri, Position::new(0, 0))
            .is_incomplete
    );
}

#[test]
fn strings_and_comments_have_no_completion_items() {
    let (mut analyzer, uri) = open("let s \"s x\"\n# s");
    assert!(analyzer.complete(&uri, Position::new(0, 9)).is_empty());
    assert!(analyzer.complete(&uri, Position::new(1, 3)).is_empty());
}

fn legacy_labels(text: &str, at: usize) -> BTreeSet<String> {
    fn is_word(c: char) -> bool {
        c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '?' | '!' | '*' | '+' | '/')
    }

    let start = text[..at]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(at, |(index, _)| index);
    let prefix = &text[start..at];
    let mut labels = BTreeSet::new();
    let parsed = read(text, FileId::new(1), &AliasEnv::new());
    for form in &parsed.nodes {
        if !matches!(form.kind, NodeKind::Call) {
            continue;
        }
        let is_definition = matches!(
            form.children.first().and_then(Node::sym_name),
            Some("let" | "var" | "fn" | "inst" | "struct" | "enum" | "bus" | "look")
        );
        if is_definition {
            if let Some(name) = form.children.get(1).and_then(Node::sym_name) {
                labels.insert(name.to_string());
            }
        }
    }
    labels.extend(
        NativeTable::global()
            .iter()
            .map(|(_, signature)| signature.name.to_string()),
    );
    let manifest = HostManifest::spec_default();
    for set in [&manifest.sounds, &manifest.synths, &manifest.controls] {
        if let KeySet::Of(keys) = set {
            labels.extend(keys.iter().map(|key| format!(":{key}")));
        }
    }
    labels.extend(
        import::prescan_imports(text)
            .into_iter()
            .map(|entry| entry.prefix.to_string()),
    );
    labels
        .into_iter()
        .filter(|label| label.starts_with(prefix))
        .collect()
}

#[test]
fn engine_results_are_a_context_appropriate_superset_of_legacy_labels() {
    let cases = [
        ("let tempo-x 12\nsi", ContextKind::Head),
        ("let tempo-x 12\nga", ContextKind::Head),
        ("let tempo-x 12\nte", ContextKind::Head),
        ("let tempo-x 12\ns :b", ContextKind::Keyword),
        ("let tempo-x 12\ns :a", ContextKind::Keyword),
        ("let tempo-x 12\n", ContextKind::Head),
    ];
    let snapshot = Snapshot {
        document: vec![DocName {
            label: "tempo-x".to_string(),
            kind: CandidateKind::Variable,
            detail: "document".to_string(),
        }],
        ..Snapshot::builtin()
    };
    for (text, expected_context) in cases {
        let at = text.len();
        let engine = complete::complete(text, at, &snapshot, MAX_LIMIT);
        assert_eq!(engine.context, expected_context, "{text:?}");
        let labels: BTreeSet<_> = engine.items.iter().map(|item| item.label.clone()).collect();
        let legacy = legacy_labels(text, at);
        let applicable = legacy.into_iter().filter(|label| {
            if expected_context == ContextKind::Keyword {
                label.starts_with(':')
            } else {
                !label.starts_with(':')
            }
        });
        for label in applicable {
            assert!(labels.contains(&label), "{label} missing for {text:?}");
        }
    }
}
