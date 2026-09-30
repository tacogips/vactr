//! Whitespace-only source formatting (design sections 3.1–3.6).

mod levels;
mod lines;
mod repair;

use crate::directives::attach::extent;
use crate::reader::import::AliasEnv;
use crate::reader::node::{Node, NodeKind};
use crate::reader::span::FileId;
use crate::types::diag::{Diagnostic, Severity};

/// The result category of formatting one source document.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The formatter accepted the source and did not change it.
    Unchanged,
    /// The formatter accepted the source and changed its layout whitespace.
    Changed,
    /// The reader rejected the source, so formatting was skipped.
    Refused,
}

/// Formatted source, its outcome, and all diagnostics emitted by the reader.
#[derive(Clone, Debug)]
pub struct Formatted {
    /// The resulting source. Refused inputs are returned byte for byte.
    pub text: String,
    /// Whether formatting changed, preserved, or refused the source.
    pub outcome: Outcome,
    /// Every reader diagnostic, including non-error severities.
    pub diags: Vec<Diagnostic>,
}

/// Formats source layout while preserving the reader's S-expression meaning.
///
/// Uses the file reader gate and layout rules in design sections 3.1–3.6.
#[must_use]
pub fn format(src: &str) -> Formatted {
    let read = crate::reader::read(src, FileId::new(1), &AliasEnv::new());
    if read
        .diags
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        let Some(converted) = repair::space_indent(src, &read.diags) else {
            return refused(src, read.diags);
        };
        let converted_read = crate::reader::read(&converted, FileId::new(1), &AliasEnv::new());
        if converted_read
            .diags
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            return refused(src, read.diags);
        }
        let mut formatted = format_clean(&converted, converted_read.diags, &converted_read.nodes);
        formatted.outcome = Outcome::Changed;
        return formatted;
    }

    format_clean(src, read.diags, &read.nodes)
}

fn refused(src: &str, diags: Vec<Diagnostic>) -> Formatted {
    Formatted {
        text: src.to_owned(),
        outcome: Outcome::Refused,
        diags,
    }
}

fn format_clean(src: &str, diags: Vec<Diagnostic>, nodes: &[Node]) -> Formatted {
    let mut lines = lines::split(src, FileId::new(1));
    levels::assign_code_levels(&mut lines, &attach_target_starts(src, nodes));
    let text = lines::assemble(src, &lines);
    let outcome = if text == src {
        Outcome::Unchanged
    } else {
        Outcome::Changed
    };
    Formatted {
        text,
        outcome,
        diags,
    }
}

/// Sorted start offsets of every `#@` attach target, mirroring
/// `directives::attach::Doc::new`: each non-error top-level node, plus every
/// non-error block statement that begins its own line.
fn attach_target_starts(src: &str, nodes: &[Node]) -> Vec<usize> {
    fn block_statements(node: &Node, src: &str, starts: &mut Vec<usize>) {
        for child in &node.children {
            if child.kind == NodeKind::Block {
                for stmt in &child.children {
                    if stmt.kind == NodeKind::Error {
                        continue;
                    }
                    let start = usize::try_from(extent(stmt).start).unwrap_or(usize::MAX);
                    if first_non_blank(src, start) == Some(start) {
                        starts.push(start);
                    }
                }
            }
            block_statements(child, src, starts);
        }
    }

    let mut starts = Vec::new();
    for node in nodes {
        if node.kind != NodeKind::Error {
            starts.push(usize::try_from(extent(node).start).unwrap_or(usize::MAX));
        }
        block_statements(node, src, &mut starts);
    }
    starts.sort_unstable();
    starts.dedup();
    starts
}

/// The offset of the first non-tab, non-space byte of the line holding `at`.
fn first_non_blank(src: &str, at: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let head = bytes.get(..at)?;
    let line_start = head
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline.saturating_add(1));
    let blanks = bytes
        .get(line_start..)?
        .iter()
        .take_while(|byte| matches!(**byte, b'\t' | b' '))
        .count();
    Some(line_start.saturating_add(blanks))
}

/// The byte status returned when source text was accepted and formatted.
pub const STATUS_FORMATTED: u32 = 0;
/// The byte status returned when reader errors caused formatting to be refused.
pub const STATUS_REFUSED: u32 = 1;
/// The byte status returned when the input bytes are not UTF-8.
pub const STATUS_NOT_UTF8: u32 = 2;

/// Formats UTF-8 bytes, preserving the input bytes on refusal or invalid UTF-8.
///
/// Status values are defined in design section 3.1 for the wasm-facing seam.
#[must_use]
pub fn format_bytes(input: &[u8]) -> (u32, Vec<u8>) {
    let Ok(src) = std::str::from_utf8(input) else {
        return (STATUS_NOT_UTF8, input.to_vec());
    };
    let formatted = format(src);
    if formatted.outcome == Outcome::Refused {
        (STATUS_REFUSED, input.to_vec())
    } else {
        (STATUS_FORMATTED, formatted.text.into_bytes())
    }
}

#[cfg(test)]
mod tests;
