//! Directive tests (design 13.5, 14.5.8): attachment, labels, selector
//! resolution, binding keys, both persistence modes and write-back.

mod attach;
mod key;
mod labels;
mod parse;
mod persist;
mod resolve;
mod writeback;

use crate::directives::key::BindingIdent;
use crate::directives::{build_table, DirectiveTable, ResolvedBinding};
use crate::reader::span::{FileId, Span};
use crate::reader::{read, AliasEnv};
use crate::types::diag::{Diagnostic, Severity};
use crate::types::HostManifest;

/// The file every test document is read as.
pub(super) const FILE: FileId = FileId::new(7);

/// The spec examples (architecture.md Editor Requirements), in one document.
pub(super) const SPEC: &str = "#@ midi ch: 1
s [:bd :sd] > lpf 800 res: 0.4 > hpf 120 > d1
#@ lpf cc: 74 71
#@ hpf cc: 30
inst analog cutoff: float = 1200 res: float = 0.3:
\tvco :saw freq > ladder cutoff res > * amp
#@ cutoff res
s [:bd :sd] > lpf 800 res: 0.4 > d1        #@ bass-filter: lpf cc: 74 71
s [:hh] > hpf 2000 > d2                    #@ hats:
#@ hats.hpf cc: 30                          # later, by label
#@ analog.cutoff cc: 1                      # an inst's name is a label
";

/// Reads `src` and builds its directive table; asserts every diagnostic is
/// a warning.
pub(super) fn table(src: &str) -> (DirectiveTable, Vec<Diagnostic>) {
    let r = read(src, FILE, &AliasEnv::new());
    let (t, diags) = build_table(
        src,
        FILE,
        &r.nodes,
        &r.trivia,
        &HostManifest::spec_default(),
    );
    assert!(
        diags.iter().all(|d| d.severity == Severity::Warning),
        "directive diagnostics are warnings: {diags:?}"
    );
    (t, diags)
}

/// The diagnostic codes, in order.
pub(super) fn codes(diags: &[Diagnostic]) -> Vec<&'static str> {
    diags.iter().map(|d| d.code.as_str()).collect()
}

/// The span of the `nth` (0-based) occurrence of `needle` in `src`.
pub(super) fn find(src: &str, needle: &str, nth: usize) -> Span {
    let at = src
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("`{needle}` #{nth} not in source"))
        .0;
    let at = u32::try_from(at).expect("small source");
    Span::new(
        FILE,
        at,
        at + u32::try_from(needle.len()).expect("small needle"),
    )
}

/// `key cc ch`, or `@line:col.param cc ch` for positional provenance.
pub(super) fn show(src: &str, r: &ResolvedBinding) -> String {
    let id = match &r.ident {
        BindingIdent::Key(k) => k.to_string(),
        BindingIdent::Positional { span, param } => {
            let before = &src[..span.0 as usize];
            let line = before.matches('\n').count() + 1;
            let col = span.0 as usize - before.rfind('\n').map_or(0, |k| k + 1) + 1;
            format!("@{line}:{col}.{param}")
        }
    };
    let opt = |v: Option<u8>| v.map_or_else(|| "-".to_string(), |n| n.to_string());
    format!("{id} cc:{} ch:{}", opt(r.cc), opt(r.ch))
}

/// Every merged binding, shown.
pub(super) fn shown(src: &str, t: &DirectiveTable) -> Vec<String> {
    t.resolved.iter().map(|r| show(src, r)).collect()
}
