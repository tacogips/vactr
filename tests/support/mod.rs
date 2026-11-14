//! Shared helpers for the spec fixture tests.

pub mod eval;
pub mod toml_subset;

use std::path::PathBuf;

/// The repository root.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A spec document under `design-docs/specs/`.
pub fn spec_doc(name: &str) -> String {
    let path = root().join("design-docs/specs").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The fixture manifest, parsed.
pub fn manifest() -> toml_subset::Manifest {
    let path = root().join("tests/fixtures/spec/manifest.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml_subset::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

/// The text of every fence whose info string is `vactr` or `vact`, in
/// document order (ordinal 1 is the first).
pub fn spec_blocks(doc: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in doc.lines() {
        match cur.as_mut() {
            Some(text) if line.starts_with("```") => {
                out.push(std::mem::take(text));
                cur = None;
            }
            Some(text) => {
                text.push_str(line);
                text.push('\n');
            }
            None if line == "```vactr" || line == "```vact" => cur = Some(String::new()),
            None => {}
        }
    }
    out
}

/// A sorted multiset of strings, for exact multiset comparison.
pub fn multiset(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items
}
