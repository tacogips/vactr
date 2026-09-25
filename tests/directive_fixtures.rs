//! Directive vocabulary fixtures (design 13.5, 14.5.8): every case of
//! `tests/fixtures/directives/vocabulary.toml` is built through
//! `directives::build_table` and compared with its expected diagnostic codes
//! and merged bindings. The vocabulary is PROPOSED, so every case must carry
//! the authority-question channel (`class` and `question`).

#[allow(dead_code)]
mod support;

use support::toml_subset::{self, Table};
use support::{multiset, root};
use vactrol::directives::build_table;
use vactrol::directives::key::BindingIdent;
use vactrol::reader::span::FileId;
use vactrol::reader::{read, AliasEnv};
use vactrol::types::diag::Severity;
use vactrol::types::HostManifest;

const FILE: FileId = FileId::new(9);

/// The constructs the vocabulary defines; each needs at least one case.
const CONSTRUCTS: [&str; 11] = [
    "file-default",
    "name-label",
    "trailing-short-form",
    "positional-cc-skip",
    "fewer-numbers",
    "ch-override",
    "addressed-param",
    "addressed-call-site",
    "ordinal",
    "reserved-key",
    "duplicate-label",
];

fn cases() -> Vec<Table> {
    let path = root().join("tests/fixtures/directives/vocabulary.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml_subset::parse(&text)
        .unwrap_or_else(|e| panic!("{e}"))
        .cases
}

/// `key cc ch`, or `@line:col.param cc ch` for positional provenance.
fn bindings(src: &str) -> (Vec<String>, Vec<String>) {
    let r = read(src, FILE, &AliasEnv::new());
    let (t, diags) = build_table(
        src,
        FILE,
        &r.nodes,
        &r.trivia,
        &HostManifest::spec_default(),
    );
    assert!(diags.iter().all(|d| d.severity == Severity::Warning));
    let opt = |v: Option<u8>| v.map_or_else(|| "-".to_string(), |n| n.to_string());
    let shown = t
        .resolved
        .iter()
        .map(|b| {
            let id = match &b.ident {
                BindingIdent::Key(k) => k.to_string(),
                BindingIdent::Positional { span, param } => {
                    let before = &src[..span.0 as usize];
                    let line = before.matches('\n').count() + 1;
                    let col = span.0 as usize - before.rfind('\n').map_or(0, |k| k + 1) + 1;
                    format!("@{line}:{col}.{param}")
                }
            };
            format!("{id} cc:{} ch:{}", opt(b.cc), opt(b.ch))
        })
        .collect();
    let codes = diags.iter().map(|d| d.code.as_str().to_string()).collect();
    (shown, multiset(codes))
}

#[test]
fn every_case_is_in_the_authority_question_channel() {
    let cases = cases();
    assert!(!cases.is_empty());
    for c in &cases {
        assert_eq!(
            c.str("class"),
            Some("authority-question"),
            "case at line {} lacks the authority-question class",
            c.line
        );
        assert!(
            c.str("question").is_some_and(|q| q.trim().ends_with('?')),
            "case at line {} lacks its question",
            c.line
        );
    }
    let ids: Vec<&str> = cases.iter().map(|c| c.req("id")).collect();
    for want in CONSTRUCTS {
        assert!(ids.contains(&want), "no case for construct `{want}`");
    }
}

#[test]
fn vocabulary_cases_resolve_as_expected() {
    let mut failures = Vec::new();
    for c in cases() {
        let id = c.req("id");
        let (got, codes) = bindings(c.req("source"));
        let want = c.array("bindings");
        if got != want {
            failures.push(format!("{id}: bindings\n  got  {got:?}\n  want {want:?}"));
        }
        let want_codes = multiset(c.array("diags"));
        if codes != want_codes {
            failures.push(format!(
                "{id}: diags\n  got  {codes:?}\n  want {want_codes:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
