//! Spec fixture runner (design 6.5.6): pins reader and expander behavior
//! against `lang-reference.md` and `design-music.md` through
//! `tests/fixtures/spec/manifest.toml`.

mod support;

use std::collections::BTreeMap;

use support::toml_subset::{self, Table};
use support::{manifest, multiset, spec_blocks, spec_doc};
use vactrol::expand::{expand, is_kernel, ExpandCx};
use vactrol::reader::span::FileId;
use vactrol::reader::{print_all, read, AliasEnv, Node, ReadResult};
use vactrol::types::diag::{DiagCode, Diagnostic};

const DOCS: [&str; 2] = ["lang-reference.md", "design-music.md"];

/// `code@line` for every reader diagnostic, the line counted within `text`.
fn reader_diags(text: &str, file: FileId) -> Vec<String> {
    let r = read(text, file, &AliasEnv::new());
    let diags = r
        .diags
        .iter()
        .map(|d| {
            let before = text.get(..d.span.start as usize).unwrap_or(text);
            format!("{}@{}", d.code, before.matches('\n').count() + 1)
        })
        .collect();
    multiset(diags)
}

/// The expansion of every form of one read.
struct Expanded {
    /// The successful outputs, each asserted kernel-only.
    nodes: Vec<Node>,
    /// The expander diagnostics. Forms with a reader error are skipped;
    /// each is asserted to return `read-error-present`.
    diags: Vec<Diagnostic>,
}

fn expand_read(r: &ReadResult, what: &str) -> Expanded {
    let mut cx = ExpandCx::new(r.next_node_id());
    let mut out = Expanded {
        nodes: Vec::new(),
        diags: Vec::new(),
    };
    for n in &r.nodes {
        let result = expand(n, &mut cx);
        if n.contains_error() {
            let code = result.as_ref().err().map(|d| d.code);
            assert_eq!(
                code,
                Some(DiagCode::ReadErrorPresent),
                "{what}: a form with a reader error"
            );
            continue;
        }
        match result {
            Ok(k) => {
                assert!(is_kernel(&k), "{what}: output is not kernel-only");
                out.nodes.push(k);
            }
            Err(d) => out.diags.push(d),
        }
    }
    out
}

/// `code@line` for every expander diagnostic, the line counted within `text`.
fn expand_diags(text: &str, file: FileId) -> Vec<String> {
    let r = read(text, file, &AliasEnv::new());
    let e = expand_read(&r, "block");
    let diags = e
        .diags
        .iter()
        .map(|d| {
            let before = text.get(..d.span.start as usize).unwrap_or(text);
            format!("{}@{}", d.code, before.matches('\n').count() + 1)
        })
        .collect();
    multiset(diags)
}

fn case_file(case: &Table) -> FileId {
    match case.str("file").unwrap_or("file") {
        "file" => FileId::new(1),
        "console" => FileId::CONSOLE,
        other => panic!("case {}: unknown file kind {other:?}", case.req("id")),
    }
}

#[test]
fn manifest_has_eleven_blocks() {
    let m = manifest();
    assert_eq!(m.blocks.len(), 11);
    assert!(!m.cases.is_empty());
}

#[test]
fn every_spec_block_has_exactly_one_entry() {
    let m = manifest();
    let mut seen: BTreeMap<(String, i64), usize> = BTreeMap::new();
    for b in &m.blocks {
        let key = (b.req("doc").to_string(), b.int("ordinal").expect("ordinal"));
        *seen.entry(key).or_default() += 1;
    }
    let mut expected = BTreeMap::new();
    for doc in DOCS {
        let count = spec_blocks(&spec_doc(doc)).len();
        assert!(count > 0, "{doc}: no vactrol blocks");
        for ordinal in 1..=count {
            let ordinal = i64::try_from(ordinal).expect("small ordinal");
            expected.insert((doc.to_string(), ordinal), 1usize);
        }
    }
    assert_eq!(
        seen, expected,
        "block entries must match the documents one to one"
    );
}

#[test]
fn blocks_read_per_classification() {
    let m = manifest();
    for b in &m.blocks {
        let doc = b.req("doc");
        let ordinal = usize::try_from(b.int("ordinal").expect("ordinal")).expect("ordinal");
        let blocks = spec_blocks(&spec_doc(doc));
        let text = &blocks[ordinal - 1];
        let got = reader_diags(text, FileId::new(1));
        let want = multiset(b.array("reader_diags"));
        let class = b.req("reader");
        assert!(
            matches!(class, "clean" | "diagnostics"),
            "{doc} #{ordinal}: reader = {class:?}"
        );
        assert_eq!(
            class == "clean",
            want.is_empty(),
            "{doc} #{ordinal}: class and reader_diags disagree"
        );
        assert_eq!(
            got,
            want,
            "{doc} #{ordinal} (section {:?})",
            b.int("section")
        );
        assert!(b.str("eval").is_some(), "{doc} #{ordinal}: missing eval");
    }
}

#[test]
fn blocks_expand_per_classification() {
    let m = manifest();
    for b in &m.blocks {
        let doc = b.req("doc");
        let ordinal = usize::try_from(b.int("ordinal").expect("ordinal")).expect("ordinal");
        let blocks = spec_blocks(&spec_doc(doc));
        let text = &blocks[ordinal - 1];
        let want = multiset(b.array("expand_diags"));
        let class = b.req("expand");
        assert!(
            matches!(class, "clean" | "diagnostics"),
            "{doc} #{ordinal}: expand = {class:?}"
        );
        assert_eq!(
            class == "clean",
            want.is_empty(),
            "{doc} #{ordinal}: class and expand_diags disagree"
        );
        let got = expand_diags(text, FileId::new(1));
        assert_eq!(
            got,
            want,
            "{doc} #{ordinal} (section {:?})",
            b.int("section")
        );
    }
}

#[test]
fn cases_read_per_expectation() {
    let m = manifest();
    let mut ids = BTreeMap::new();
    for case in &m.cases {
        let id = case.req("id");
        assert!(
            ids.insert(id.to_string(), ()).is_none(),
            "duplicate case id {id}"
        );
        let source = case.req("source");
        let r = read(source, case_file(case), &AliasEnv::new());
        if let Some(expected) = case.str("read") {
            assert_eq!(print_all(&r.nodes), expected, "case {id}");
        }
        let e = expand_read(&r, id);
        if let Some(expected) = case.str("expand") {
            assert!(
                e.diags.is_empty(),
                "case {id}: expand string with a diagnostic"
            );
            assert_eq!(
                e.nodes.len(),
                r.nodes.len(),
                "case {id}: every form must expand"
            );
            assert_eq!(print_all(&e.nodes), expected, "case {id}: expand");
        }
        let codes = r.diags.iter().chain(e.diags.iter());
        let got = multiset(codes.map(|d| d.code.to_string()).collect());
        assert_eq!(got, multiset(case.array("diags")), "case {id}: diagnostics");
        assert!(
            case.str("read").is_some() || !got.is_empty(),
            "case {id}: nothing asserted"
        );
    }
}

#[test]
fn verbatim_cases_appear_in_their_document() {
    let m = manifest();
    let docs: BTreeMap<&str, String> = DOCS.iter().map(|d| (*d, spec_doc(d))).collect();
    let mut verbatim = 0;
    for case in &m.cases {
        let id = case.req("id");
        let doc = docs
            .get(case.req("doc"))
            .unwrap_or_else(|| panic!("case {id}: unknown doc"));
        let flag = case
            .bool("verbatim")
            .unwrap_or_else(|| panic!("case {id}: missing verbatim"));
        if flag {
            verbatim += 1;
            assert!(
                doc.contains(case.req("source")),
                "case {id}: source not found verbatim"
            );
        }
    }
    assert!(verbatim > 0);
}

#[test]
fn authority_questions_are_pinned() {
    let m = manifest();
    let aq: Vec<&Table> = m
        .cases
        .iter()
        .filter(|c| c.str("class") == Some("authority-question"))
        .collect();
    assert_eq!(aq.len(), 2);
    for c in aq {
        assert!(
            c.str("question").is_some(),
            "case {}: missing question",
            c.req("id")
        );
        assert!(
            c.str("read").is_some(),
            "case {}: missing read",
            c.req("id")
        );
        assert!(
            c.str("expand").is_some(),
            "case {}: missing expand",
            c.req("id")
        );
    }
    let u2 = m
        .cases
        .iter()
        .find(|c| c.str("id") == Some("u2-gt-statement-level"));
    assert_eq!(u2.and_then(|c| c.str("read")), Some("(> a 10)"));
    let u4 = m
        .cases
        .iter()
        .find(|c| c.str("id") == Some("u4-inline-fn-body"));
    assert_eq!(
        u4.map(|c| c.array("diags")),
        Some(vec!["malformed-fn".to_string()])
    );
    let u5 = m
        .cases
        .iter()
        .find(|c| c.str("id") == Some("u5-bare-variant-binding-if"));
    assert_eq!(
        u5.and_then(|c| c.str("expand")),
        Some("(match x {(-> (false | nil) nil) (-> (none) 1)})")
    );
}

#[test]
fn toml_subset_rejects_unsupported_constructs() {
    let bad = [
        ("[table]\n", 1),
        ("[[block]]\nkey = 1.5\n", 2),
        ("[[block]]\nkey = { a = 1 }\n", 2),
        ("key = 1\n", 1),
        ("[[case]]\nsource = '''\nnever closed\n", 2),
        ("[[case]]\nx = \"a\\q\"\n", 2),
        ("[[case]]\nx = 1\nx = 2\n", 3),
    ];
    for (text, line) in bad {
        let e = toml_subset::parse(text).expect_err(text);
        assert!(
            e.starts_with(&format!("manifest line {line}:")),
            "{text:?}: {e}"
        );
    }
    let ok = toml_subset::parse("# c\n[[case]]\na = \"x\\ty\" # c\nb = '''\n\tz'''\nc = ['p', ]\n");
    assert!(
        ok.is_err(),
        "single-quoted array items are outside the subset"
    );
    let ok = toml_subset::parse(
        "[[case]]\na = \"x\\ty\" # c\nb = '''\n\tz'''\nc = [\"p\", \"q\"]\nd = true\ne = -3\n",
    )
    .expect("valid subset");
    let t = &ok.cases[0];
    assert_eq!(t.str("a"), Some("x\ty"));
    assert_eq!(t.str("b"), Some("\tz"));
    assert_eq!(t.array("c"), ["p", "q"]);
    assert_eq!(t.bool("d"), Some(true));
    assert_eq!(t.int("e"), Some(-3));
}
