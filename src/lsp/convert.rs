//! Conversions between Vactr source positions and LSP types (design
//! 14.5.11): byte offsets to LSP `Position`s (UTF-16 columns) and back,
//! and checker or runtime diagnostics to `lsp_types::Diagnostic`.

use tower_lsp::lsp_types::{self as lsp, NumberOrString, Position, Range};

use crate::session::protocol::WireDiag;
use crate::types::diag::{Diagnostic, Severity};

/// The `source` of every published diagnostic.
pub const SOURCE: &str = "vactr";

/// The LSP position of byte `offset` in `text`. An offset past the end
/// clamps to the end; an offset inside a UTF-8 sequence rounds down to
/// its character.
#[must_use]
pub fn position(text: &str, offset: usize) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |k| k + 1);
    let line = before.matches('\n').count();
    let character: usize = before[line_start..].chars().map(char::len_utf16).sum();
    Position::new(
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(character).unwrap_or(u32::MAX),
    )
}

/// The byte offset of `pos` in `text`. A line past the end clamps to the
/// end of the text; a column past the end of its line clamps to the line
/// end; a column inside a surrogate pair rounds down to its character.
#[must_use]
pub fn offset(text: &str, pos: Position) -> usize {
    let mut start = 0;
    for _ in 0..pos.line {
        match text[start..].find('\n') {
            Some(k) => start += k + 1,
            None => return text.len(),
        }
    }
    let line_end = text[start..].find('\n').map_or(text.len(), |k| start + k);
    let mut units = 0usize;
    let want = usize::try_from(pos.character).unwrap_or(usize::MAX);
    for (k, c) in text[start..line_end].char_indices() {
        if units + c.len_utf16() > want {
            return start + k;
        }
        units += c.len_utf16();
    }
    line_end
}

/// The LSP range of the byte range `start..end` in `text`.
#[must_use]
pub fn range(text: &str, start: usize, end: usize) -> Range {
    Range::new(position(text, start), position(text, end.max(start)))
}

fn severity(s: Severity) -> lsp::DiagnosticSeverity {
    match s {
        Severity::Error => lsp::DiagnosticSeverity::ERROR,
        Severity::Warning => lsp::DiagnosticSeverity::WARNING,
        Severity::Hint => lsp::DiagnosticSeverity::HINT,
    }
}

fn diagnostic(
    range: Range,
    sev: lsp::DiagnosticSeverity,
    code: &str,
    msg: &str,
) -> lsp::Diagnostic {
    lsp::Diagnostic {
        range,
        severity: Some(sev),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some(SOURCE.to_string()),
        message: msg.to_string(),
        ..lsp::Diagnostic::default()
    }
}

/// A checker, reader, expander, package or directive diagnostic of
/// `text` as an LSP diagnostic.
#[must_use]
pub fn to_lsp(text: &str, d: &Diagnostic) -> lsp::Diagnostic {
    let r = range(text, d.span.start as usize, d.span.end as usize);
    diagnostic(r, severity(d.severity), d.code.as_str(), &d.message)
}

/// A runtime `diag` from a session socket as an LSP diagnostic of `text`.
/// An unknown severity string maps to an error.
#[must_use]
pub fn wire_to_lsp(text: &str, d: &WireDiag) -> lsp::Diagnostic {
    let sev = match d.severity.as_str() {
        "warning" => lsp::DiagnosticSeverity::WARNING,
        "hint" => lsp::DiagnosticSeverity::HINT,
        _ => lsp::DiagnosticSeverity::ERROR,
    };
    let r = range(text, d.span.start as usize, d.span.end as usize);
    diagnostic(r, sev, &d.code, &d.message)
}
