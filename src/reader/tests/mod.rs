//! Reader unit tests (FE-READER required tests).

use crate::reader::span::FileId;
use crate::reader::{print_all, read, AliasEnv, ReadResult};

mod import;
mod layout;
mod lexer;
mod line;
mod no_panic;
mod pathlit;
mod spans;

/// An ordinary file (not the console).
pub(super) const FILE: FileId = FileId::new(1);

pub(super) fn rd(src: &str) -> ReadResult {
    read(src, FILE, &AliasEnv::new())
}

pub(super) fn printed(src: &str) -> String {
    print_all(&rd(src).nodes)
}

pub(super) fn codes(src: &str) -> Vec<&'static str> {
    rd(src).diags.iter().map(|d| d.code.as_str()).collect()
}

/// Asserts the canonical print and that no diagnostic was produced.
pub(super) fn reads_as(src: &str, expected: &str) {
    let r = rd(src);
    let got: Vec<&str> = r.diags.iter().map(|d| d.code.as_str()).collect();
    assert!(got.is_empty(), "{src:?}: unexpected diagnostics {got:?}");
    assert_eq!(print_all(&r.nodes), expected, "{src:?}");
}

/// Asserts that `bad` gives exactly `code`, and that the next line
/// `print 1` still reads.
pub(super) fn recovers(bad: &str, code: &str) {
    let src = format!("{bad}\nprint 1\n");
    let r = rd(&src);
    let got: Vec<&str> = r.diags.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(got, [code], "{bad:?}");
    for d in &r.diags {
        assert_eq!(d.span.file, FILE, "{bad:?}: diagnostic in the wrong file");
        assert!(
            d.span.start <= d.span.end && d.span.end as usize <= bad.len(),
            "{bad:?}: {d}"
        );
    }
    assert_eq!(print_all(&r.nodes), "(#error)\n(print 1)", "{bad:?}");
}

/// The text of every `vactr`/`vact` fence in a spec document.
pub(super) fn spec_blocks(doc: &str) -> Vec<String> {
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

pub(super) const LANG_REFERENCE: &str =
    include_str!("../../../design-docs/specs/lang-reference.md");
pub(super) const DESIGN_MUSIC: &str = include_str!("../../../design-docs/specs/design-music.md");
