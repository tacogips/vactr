//! Spec fixture evaluation (design 7.1.7): a block or case is checked as
//! one document (all its forms in one session scope), then run form by
//! form in one fresh `Evaluator` (`NoopHost`, `RecordingSink`) through
//! read -> expand -> check -> compile -> run. Blocks that need package
//! loading run through a `Session` instead (`session_eval`).

use vactr::dsp::caps::CapabilitySet;
use vactr::expand::{expand, ExpandCx};
use vactr::host::caps::Hosts;
use vactr::ns::evaluator::Evaluator;
use vactr::ns::load::NoopHost;
use vactr::ns::namespace::Prelude;
use vactr::ns::stage::RecordingSink;
use vactr::pattern::{query, InputCells, QueryCtx, QueryResult, TimeSpan};
use vactr::reader::span::FileId;
use vactr::reader::{read, AliasEnv, Node};
use vactr::session::session::{Session, SessionConfig};
use vactr::types::check;
use vactr::types::diag::{Diagnostic, Severity};
use vactr::types::ty::CheckEnv;
use vactr::types::HostManifest;
use vactr::value::value::Value;
use vactr::vm::{Failure, VmQuery};

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

/// What evaluating a block through a `Session` produced (design 14.5.4).
pub struct SessionRun {
    /// `code@line` of every error and warning the session reported
    /// (reader, expander, checker, load, package and directive).
    pub diags: Vec<String>,
    /// `code@line` of every failing form (its first line) and every drain
    /// fault (its span's line; line 0 when it has none).
    pub fails: Vec<String>,
}

/// Evaluates `text` as one document through `vactr::session::Session`
/// over `NoopHost` hosts with no lock and no package cache: the path of
/// blocks that need package loading (`eval_via = "session"`).
pub fn session_eval(text: &str) -> SessionRun {
    let cfg = SessionConfig::new(CapabilitySet::native());
    let mut s = Session::new(cfg, Hosts::noop());
    let (out, _) = s.eval(text, "spec.vact", 1, 0, None);
    let diags = out
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Hint)
        .map(|d| code_at(text, d))
        .collect();
    let mut fails: Vec<String> = out
        .forms
        .iter()
        .filter_map(|f| {
            let line = line_of(text, f.span.start);
            f.failure.as_ref().map(|e| format!("{}@{line}", e.code))
        })
        .collect();
    fails.extend(out.faults.iter().map(|e| {
        let line = e.origin.span.map_or(0, |sp| line_of(text, sp.start));
        format!("{}@{line}", e.code)
    }));
    SessionRun {
        diags: multiset(diags),
        fails: multiset(fails),
    }
}
