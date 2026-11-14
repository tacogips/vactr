//! Checker unit tests: ME-MASKS (`masks`, `natives`, `ty`) and ME-CHECK
//! (the rest), with the helpers the ME-CHECK modules share.

use crate::expand::{expand, ExpandCx};
use crate::reader::node::Node;
use crate::reader::span::FileId;
use crate::reader::{read, AliasEnv};
use crate::types::check::{check, CheckResult};
use crate::types::diag::{DiagCode, Severity};
use crate::types::manifest::HostManifest;
use crate::types::ty::CheckEnv;

mod analysis;
mod check_basic;
mod chords;
mod deps;
mod diags;
mod inst;
mod masks;
mod natives;
mod no_abort;
mod scope;
mod scope_cost;
mod sound;
mod ty;

/// An ordinary file (not the console).
pub(super) const FILE: FileId = FileId::new(1);

/// Reads and expands `src`, which must read and expand clean.
pub(super) fn forms(src: &str) -> Vec<Node> {
    let r = read(src, FILE, &AliasEnv::new());
    let codes: Vec<&str> = r.diags.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.is_empty(), "{src:?}: reader diagnostics {codes:?}");
    let mut cx = ExpandCx::new(r.next_node_id());
    r.nodes
        .iter()
        .map(|n| expand(n, &mut cx).unwrap_or_else(|d| panic!("{src:?}: {d}")))
        .collect()
}

/// Checks `src` as one document against an empty session and the spec
/// manifest.
pub(super) fn check_src(src: &str) -> CheckResult {
    check_env(src, &CheckEnv::empty(), &HostManifest::spec_default())
}

/// Checks `src` as one document against `env` and `manifest`.
pub(super) fn check_env(src: &str, env: &CheckEnv, manifest: &HostManifest) -> CheckResult {
    check(&forms(src), env, manifest)
}

/// Every diagnostic as `code@line` (1-based), in report order.
pub(super) fn diag_lines(src: &str, r: &CheckResult) -> Vec<String> {
    r.diags
        .iter()
        .map(|d| format!("{}@{}", d.code, line_of(src, d.span.start)))
        .collect()
}

/// The 1-based line of a byte offset.
pub(super) fn line_of(src: &str, offset: u32) -> usize {
    let end = usize::try_from(offset).unwrap_or(usize::MAX).min(src.len());
    src[..end].matches('\n').count() + 1
}

/// Asserts that `src` checks with exactly the diagnostics `expected`
/// (`code@line`, in report order).
pub(super) fn assert_diags(src: &str, expected: &[&str]) {
    let r = check_src(src);
    assert_eq!(diag_lines(src, &r), expected, "{src:?}: {:?}", r.diags);
}

/// Asserts that `src` checks clean.
pub(super) fn assert_clean(src: &str) {
    assert_diags(src, &[]);
}

/// Asserts one diagnostic with `code` on `line`, with the code's default
/// severity.
pub(super) fn assert_has(src: &str, code: DiagCode, line: usize, severity: Severity) {
    let r = check_src(src);
    let hit = r
        .diags
        .iter()
        .find(|d| d.code == code && line_of(src, d.span.start) == line);
    let d = hit.unwrap_or_else(|| panic!("{src:?}: no {code}@{line} in {:?}", r.diags));
    assert_eq!(d.severity, severity, "{src:?}: {d}");
    assert_eq!(d.severity, code.default_severity());
}

/// The type of the last top-level form.
pub(super) fn last_type(src: &str) -> String {
    let fs = forms(src);
    let r = check(&fs, &CheckEnv::empty(), &HostManifest::spec_default());
    let last = fs.last().expect("a form");
    r.types
        .get(&last.id)
        .map(ToString::to_string)
        .unwrap_or_default()
}
