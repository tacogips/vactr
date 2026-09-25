//! Spec fixture runner (design 6.5.6, 7.1.7): pins reader, expander,
//! checker and evaluation behavior against `lang-reference.md` and
//! `design-music.md` through `tests/fixtures/spec/manifest.toml`.

mod support;

use std::collections::BTreeMap;

use support::eval::{check_diags, check_errors, clean_forms, query_cycle, run};
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
            case.str("read").is_some() || !got.is_empty() || is_eval_case(case),
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

/// The evaluation classes of a block (7.1.7).
const EVAL_CLASSES: [&str; 5] = [
    "positive",
    "diagnostic",
    "authority-question",
    "illustrative-excluded",
    "deferred",
];

/// The failures a block or case shows ONLY because of a recorded
/// implementation defect (`blocked_by`, a repair request of the ME-INTEGRATE
/// progress log). They are asserted to reproduce exactly, so a repair makes
/// the runner fail until the manifest drops them (a strict expected failure,
/// never a silent pin).
fn defects(t: &Table, what: &str) -> Vec<String> {
    let d = t.array("defect_run_fails");
    assert_eq!(
        t.str("blocked_by").is_some(),
        !d.is_empty(),
        "{what}: blocked_by and defect_run_fails go together"
    );
    d
}

/// A case with an evaluation expectation (`value` or `fail`).
fn is_eval_case(case: &Table) -> bool {
    case.str("value").is_some() || case.str("fail").is_some()
}

fn block_text(b: &Table) -> (String, String) {
    let doc = b.req("doc");
    let ordinal = usize::try_from(b.int("ordinal").expect("ordinal")).expect("ordinal");
    let blocks = spec_blocks(&spec_doc(doc));
    (format!("{doc} #{ordinal}"), blocks[ordinal - 1].clone())
}

#[test]
fn blocks_evaluate_per_classification() {
    let m = manifest();
    let mut evaluated = 0;
    for b in &m.blocks {
        let (what, text) = block_text(b);
        let class = b.req("eval");
        assert!(EVAL_CLASSES.contains(&class), "{what}: eval = {class:?}");
        assert!(
            b.str("note").is_some(),
            "{what}: a classification needs a note"
        );
        let file = FileId::new(1);
        match class {
            "positive" | "diagnostic" => {
                let forms = clean_forms(&text, file);
                assert!(!forms.is_empty(), "{what}: nothing to evaluate");
                let checked = check_diags(&text, &forms);
                assert_eq!(checked, multiset(b.array("check_diags")), "{what}: check");
                let ran = run(&text, &forms).fails();
                let mut want = b.array("run_fails");
                want.extend(defects(b, &what));
                assert_eq!(ran, multiset(want), "{what}: run");
                if class == "positive" {
                    assert!(check_errors(&text, &forms).is_empty(), "{what}");
                    assert!(ran.is_empty(), "{what}: a positive block runs clean");
                } else {
                    assert!(
                        !checked.is_empty() || !ran.is_empty(),
                        "{what}: a diagnostic block pins a code"
                    );
                }
                evaluated += 1;
            }
            "deferred" => {
                let to = b.req("deferred_to");
                assert!(to.starts_with("TASK-00"), "{what}: deferred_to = {to:?}");
                // No panic and no abort: the whole block is checked.
                let forms = clean_forms(&text, file);
                let checked = check_diags(&text, &forms);
                if b.bool("pin_check") == Some(true) {
                    assert_eq!(checked, multiset(b.array("check_diags")), "{what}: check");
                }
            }
            "authority-question" => {
                assert!(b.str("question").is_some(), "{what}: missing question");
            }
            _ => {}
        }
    }
    assert!(evaluated > 0, "no block was evaluated");
}

#[test]
fn cases_evaluate_per_expectation() {
    let m = manifest();
    let mut evaluated = 0;
    for case in m.cases.iter().filter(|c| is_eval_case(c)) {
        let id = case.req("id");
        let source = case.req("source");
        let forms = clean_forms(source, case_file(case));
        assert!(!forms.is_empty(), "case {id}: nothing to evaluate");
        let checked = check_diags(source, &forms);
        assert_eq!(
            checked,
            multiset(case.array("check_diags")),
            "case {id}: check"
        );
        let mut r = run(source, &forms);
        let mut want = case.array("run_fails");
        let blocked = defects(case, id);
        let is_blocked = !blocked.is_empty();
        want.extend(blocked);
        assert_eq!(
            r.fails_before_last(),
            multiset(want),
            "case {id}: earlier forms"
        );
        if is_blocked {
            // The defect reproduces exactly; the later expectations are the
            // design's and are asserted once the repair lands.
            continue;
        }
        let last = r.last().cloned().expect("a last form");
        match (case.str("value"), case.str("fail")) {
            (Some(v), None) => {
                let got = last.as_ref().map(ToString::to_string);
                assert_eq!(got.as_deref(), Ok(v), "case {id}: value");
            }
            (None, Some(code)) => {
                let got = last.as_ref().err().map(|e| e.code.as_str());
                assert_eq!(got, Some(code), "case {id}: fail");
            }
            _ => panic!("case {id}: exactly one of value and fail"),
        }
        let sounds = case.array("query_values");
        let faults = case.array("query_faults");
        if !sounds.is_empty() || !faults.is_empty() {
            let v = last.expect("a queried case returns a pattern");
            let q = query_cycle(&mut r.ev, &v, 0).expect("a pattern");
            let got: Vec<String> = q.events.iter().map(|e| e.value.to_string()).collect();
            assert_eq!(got, sounds, "case {id}: queried values");
            let got: Vec<String> = q.faults.iter().map(|f| f.code.to_string()).collect();
            assert_eq!(got, faults, "case {id}: query faults");
        }
        evaluated += 1;
    }
    assert!(evaluated > 0, "no case was evaluated");
}

/// Prints what every block and evaluation case produces (maintenance aid
/// for classifying fixtures): `cargo test --test spec_fixtures --
/// --ignored --nocapture evaluation_report`.
#[test]
#[ignore = "prints a report; run on demand"]
fn evaluation_report() {
    let m = manifest();
    for b in &m.blocks {
        let (what, text) = block_text(b);
        let forms = clean_forms(&text, FileId::new(1));
        println!("== {what} [{}]", b.req("eval"));
        println!("   check_diags = {:?}", check_diags(&text, &forms));
        println!("   run_fails = {:?}", run(&text, &forms).fails());
    }
    for case in m.cases.iter().filter(|c| is_eval_case(c)) {
        let source = case.req("source");
        let forms = clean_forms(source, case_file(case));
        let mut r = run(source, &forms);
        let last = match r.last() {
            Some(Ok(v)) => format!("value {v}"),
            Some(Err(e)) => format!("fail {}", e.code),
            None => "none".to_string(),
        };
        println!("== case {}", case.req("id"));
        println!("   check_diags = {:?}", check_diags(source, &forms));
        println!(
            "   before_last = {:?}; last = {last}",
            r.fails_before_last()
        );
        if let Some(Ok(v)) = r.last().cloned() {
            if let Some(q) = query_cycle(&mut r.ev, &v, 0) {
                let vals: Vec<String> = q.events.iter().map(|e| e.value.to_string()).collect();
                let faults: Vec<String> = q.faults.iter().map(|f| f.code.to_string()).collect();
                println!("   query_values = {vals:?}; query_faults = {faults:?}");
            }
        }
    }
}
