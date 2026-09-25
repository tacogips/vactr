//! Spec fixture evaluation (design 7.1.7): a block or case is checked as
//! one document (all its forms in one session scope), then run form by
//! form in one fresh `Evaluator` (`NoopHost`, `RecordingSink`) through
//! read -> expand -> check -> compile -> run.

use vactrol::expand::{expand, ExpandCx};
use vactrol::ns::evaluator::Evaluator;
use vactrol::ns::load::NoopHost;
use vactrol::ns::namespace::Prelude;
use vactrol::ns::stage::RecordingSink;
use vactrol::pattern::{query, InputCells, QueryCtx, QueryResult, TimeSpan};
use vactrol::reader::span::FileId;
use vactrol::reader::{read, AliasEnv, Node};
use vactrol::types::check;
use vactrol::types::diag::{Diagnostic, Severity};
use vactrol::types::ty::CheckEnv;
use vactrol::types::HostManifest;
use vactrol::value::value::Value;
use vactrol::vm::{Failure, VmQuery};

use super::multiset;

/// The 1-based line of a byte offset within `text`.
pub fn line_of(text: &str, at: u32) -> usize {
    let before = text.get(..at as usize).unwrap_or(text);
    before.matches('\n').count() + 1
}

/// The forms of `text` that read and expand clean (a form with a reader or
/// expander error is not checked or run, 7.1.1).
pub fn clean_forms(text: &str, file: FileId) -> Vec<Node> {
    let r = read(text, file, &AliasEnv::new());
    let mut cx = ExpandCx::new(r.next_node_id());
    r.nodes
        .iter()
        .filter(|n| !n.contains_error())
        .filter_map(|n| expand(n, &mut cx).ok())
        .collect()
}

/// `code@line` for every error and warning of checking `forms` as one
/// document (hints are the LSP's and never pinned).
pub fn check_diags(text: &str, forms: &[Node]) -> Vec<String> {
    let r = check(forms, &CheckEnv::empty(), &HostManifest::spec_default());
    let codes = r
        .diags
        .iter()
        .filter(|d| d.severity != Severity::Hint)
        .map(|d| code_at(text, d))
        .collect();
    multiset(codes)
}

fn code_at(text: &str, d: &Diagnostic) -> String {
    format!("{}@{}", d.code, line_of(text, d.span.start))
}

/// A fresh evaluator over the full prelude.
pub fn evaluator() -> Evaluator {
    Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    )
}

/// What running a document form by form produced.
pub struct Run {
    pub ev: Evaluator,
    /// Each form's first line and result, in order.
    pub results: Vec<(usize, Result<Value, Failure>)>,
}

impl Run {
    /// `code@line` of every failing form except the last.
    pub fn fails_before_last(&self) -> Vec<String> {
        let n = self.results.len().saturating_sub(1);
        multiset(fails(&self.results[..n]))
    }

    /// `code@line` of every failing form.
    pub fn fails(&self) -> Vec<String> {
        multiset(fails(&self.results))
    }

    /// The result of the last form.
    pub fn last(&self) -> Option<&Result<Value, Failure>> {
        self.results.last().map(|(_, r)| r)
    }
}

fn fails(results: &[(usize, Result<Value, Failure>)]) -> Vec<String> {
    results
        .iter()
        .filter_map(|(line, r)| r.as_ref().err().map(|e| format!("{}@{line}", e.code)))
        .collect()
}

/// Runs `forms` in order in one fresh evaluator.
pub fn run(text: &str, forms: &[Node]) -> Run {
    let mut ev = evaluator();
    let mut results = Vec::new();
    for form in forms {
        let out = ev.eval_form(form);
        results.push((line_of(text, form.span.start), out.value));
    }
    Run { ev, results }
}

/// `code@line` for every ERROR of checking `forms` as one document.
pub fn check_errors(text: &str, forms: &[Node]) -> Vec<String> {
    let r = check(forms, &CheckEnv::empty(), &HostManifest::spec_default());
    let codes = r
        .diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| code_at(text, d))
        .collect();
    multiset(codes)
}

/// Queries a pattern value over cycle `c` through the evaluator's VM.
pub fn query_cycle(ev: &mut Evaluator, v: &Value, c: i64) -> Option<QueryResult> {
    let Value::Pattern(p) = v else {
        return None;
    };
    let cells = InputCells::new();
    let (vm, ns) = ev.vm_and_ns();
    let mut handle = VmQuery::new(vm, ns);
    let mut cx = QueryCtx::new(&mut handle, &cells, 1);
    Some(query(p, TimeSpan::cycle(c).ok()?, &mut cx))
}
