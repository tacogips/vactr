//! Expander unit tests (FE-EXPAND required tests).

use std::collections::BTreeSet;

use crate::expand::{expand, is_kernel, ExpandCx};
use crate::reader::node::Node;
use crate::reader::span::{FileId, Span};
use crate::reader::{print_all, read, AliasEnv, ReadResult};
use crate::types::diag::Diagnostic;

mod diagnostics;
mod for_map;
mod if_match;
mod kernel;
mod small_sugar;

/// An ordinary file (not the console).
pub(super) const FILE: FileId = FileId::new(1);

pub(super) fn rd(src: &str) -> ReadResult {
    read(src, FILE, &AliasEnv::new())
}

/// Reads `src` (which must read clean) and expands every form.
pub(super) fn expand_src(src: &str) -> (ReadResult, Vec<Result<Node, Diagnostic>>) {
    let r = rd(src);
    let codes: Vec<&str> = r.diags.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.is_empty(), "{src:?}: reader diagnostics {codes:?}");
    let mut cx = ExpandCx::new(r.next_node_id());
    let out = r.nodes.iter().map(|n| expand(n, &mut cx)).collect();
    (r, out)
}

/// Asserts that `src` expands clean to `expected`, that every output is
/// kernel-only, and that synthesized ids are fresh and unique.
pub(super) fn expands_as(src: &str, expected: &str) {
    let (r, out) = expand_src(src);
    let nodes: Vec<Node> = out
        .into_iter()
        .map(|o| o.unwrap_or_else(|d| panic!("{src:?}: {d}")))
        .collect();
    assert_eq!(print_all(&nodes), expected, "{src:?}");
    for n in &nodes {
        assert!(is_kernel(n), "{src:?}: not kernel-only");
    }
    assert_fresh_ids(&r, &nodes);
}

/// Every id is unique in the output; ids the reader did not assign are at
/// least `first_free`.
pub(super) fn assert_fresh_ids(r: &ReadResult, nodes: &[Node]) {
    let mut read_ids = BTreeSet::new();
    for n in &r.nodes {
        n.walk(&mut |c: &Node| {
            read_ids.insert(c.id);
        });
    }
    let first_free = r.next_node_id();
    let mut seen = BTreeSet::new();
    for n in nodes {
        n.walk(&mut |c: &Node| {
            assert!(seen.insert(c.id), "duplicate node id {:?}", c.id);
            assert!(
                read_ids.contains(&c.id) || c.id >= first_free,
                "synthesized id {:?} is below first_free {first_free:?}",
                c.id
            );
        });
    }
}

/// The first expander diagnostic of `src` (the reader must be clean).
pub(super) fn expand_err(src: &str) -> Diagnostic {
    let (_, out) = expand_src(src);
    out.into_iter()
        .find_map(Result::err)
        .unwrap_or_else(|| panic!("{src:?}: expected an expander diagnostic"))
}

/// The byte span of the first occurrence of `needle` in `src`.
pub(super) fn span_of(src: &str, needle: &str) -> Span {
    let start = src
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {src:?}"));
    let start = u32::try_from(start).expect("small test source");
    let len = u32::try_from(needle.len()).expect("small test source");
    Span::new(FILE, start, start + len)
}
