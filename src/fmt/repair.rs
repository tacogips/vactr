//! Conservative repairs for source rejected only because of space indentation.

use crate::reader::span::FileId;
use crate::types::diag::{DiagCode, Diagnostic, Severity};

use super::lines::{self, LineKind};

/// Converts unambiguous, pure-space indentation to tabs when diagnostics allow it.
pub(super) fn space_indent(src: &str, diags: &[Diagnostic]) -> Option<String> {
    let mut has_indent_space = false;
    for diagnostic in diags {
        if diagnostic.severity != Severity::Error {
            continue;
        }
        match diagnostic.code {
            DiagCode::IndentSpace => has_indent_space = true,
            DiagCode::EmptyBlock => {}
            _ => return None,
        }
    }
    if !has_indent_space {
        return None;
    }

    let lines = lines::split(src, FileId::new(1));
    let mut indents = Vec::new();
    for line in &lines {
        if !matches!(line.kind, LineKind::Code { .. } | LineKind::Comment { .. })
            || line.indent_width == 0
        {
            continue;
        }
        if line.original_level != 0 {
            return None;
        }
        indents.push((line.start, line.indent_end, line.indent_width));
    }

    let unit = indents.iter().map(|(_, _, width)| *width).min()?;
    if !(2..=8).contains(&unit) || indents.iter().any(|(_, _, width)| width % unit != 0) {
        return None;
    }

    let mut converted = String::with_capacity(src.len());
    let mut copied_to = 0;
    for (start, indent_end, width) in indents {
        converted.push_str(&src[copied_to..start]);
        for _ in 0..(width / unit) {
            converted.push('\t');
        }
        copied_to = indent_end;
    }
    converted.push_str(&src[copied_to..]);
    Some(converted)
}
