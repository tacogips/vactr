//! Physical-line parsing and output assembly (design sections 3.2–3.3).

use crate::reader::lexer::{lex_line, Tok};
use crate::reader::node::TriviaKind;
use crate::reader::span::FileId;

/// The syntax class of a physical line after its indentation run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineKind {
    Blank,
    Comment { kind: TriviaKind, start: usize },
    Code { opens_block: bool, directive: bool },
}

/// A source line with byte ranges and formatter-selected indentation.
#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub(crate) start: usize,
    pub(crate) indent_end: usize,
    pub(crate) body_end: usize,
    pub(crate) terminator_end: usize,
    pub(crate) trimmed_end: usize,
    pub(crate) indent_width: u32,
    pub(crate) original_level: u32,
    pub(crate) formatted_level: Option<u32>,
    pub(crate) kind: LineKind,
}

impl Line {
    /// True for code lines; used to look up the next statement's body level.
    pub(crate) const fn is_code(&self) -> bool {
        matches!(self.kind, LineKind::Code { .. })
    }

    /// True when this line contributes content rather than only layout.
    pub(crate) const fn has_content(&self) -> bool {
        !matches!(self.kind, LineKind::Blank)
    }
}

/// Splits at LF, excluding a CR immediately before LF from the line body.
pub(crate) fn split(src: &str, file: FileId) -> Vec<Line> {
    let bytes = src.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    loop {
        let newline = bytes[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| start + offset);
        let line_end = newline.unwrap_or(bytes.len());
        let body_end = if line_end > start && bytes.get(line_end - 1) == Some(&b'\r') {
            line_end - 1
        } else {
            line_end
        };
        let terminator_end = newline.map_or(bytes.len(), |at| at.saturating_add(1));

        let mut indent_end = start;
        let mut indent_width = 0u32;
        let mut original_level = 0u32;
        while indent_end < body_end {
            match bytes.get(indent_end) {
                Some(b'\t') => {
                    original_level = original_level.saturating_add(1);
                    indent_width = indent_width.saturating_add(1);
                }
                Some(b' ') => indent_width = indent_width.saturating_add(1),
                _ => break,
            }
            indent_end = indent_end.saturating_add(1);
        }

        let lexed = lex_line(src, indent_end, body_end, file);
        let kind = if lexed.tokens.is_empty() {
            match lexed.comment {
                Some(comment) => LineKind::Comment {
                    kind: comment.kind,
                    start: usize::try_from(comment.span.start).unwrap_or(indent_end),
                },
                None => LineKind::Blank,
            }
        } else {
            LineKind::Code {
                opens_block: lexed
                    .tokens
                    .last()
                    .is_some_and(|token| token.kind == Tok::BlockColon),
                directive: lexed
                    .comment
                    .is_some_and(|comment| comment.kind == TriviaKind::Directive),
            }
        };
        let preserve_trailing = matches!(
            kind,
            LineKind::Comment {
                kind: TriviaKind::Directive,
                ..
            } | LineKind::Code {
                directive: true,
                ..
            }
        );
        let mut trimmed_end = body_end;
        if !preserve_trailing {
            while trimmed_end > indent_end
                && matches!(bytes.get(trimmed_end - 1), Some(b' ' | b'\t'))
            {
                trimmed_end -= 1;
            }
        }

        lines.push(Line {
            start,
            indent_end,
            body_end,
            terminator_end,
            trimmed_end,
            indent_width,
            original_level,
            formatted_level: None,
            kind,
        });
        let Some(newline) = newline else {
            break;
        };
        start = newline.saturating_add(1);
        if start > bytes.len() {
            break;
        }
    }
    lines
}

/// Assembles lines with selected indentation and their original terminators.
pub(crate) fn assemble(src: &str, lines: &[Line]) -> String {
    let Some(last_content) = lines.iter().rposition(Line::has_content) else {
        return String::new();
    };
    let mut output = String::with_capacity(src.len());
    for (index, line) in lines
        .iter()
        .take(last_content.saturating_add(1))
        .enumerate()
    {
        match line.kind {
            LineKind::Blank => {}
            LineKind::Comment { start, .. } => {
                let level = line.formatted_level.unwrap_or(0);
                append_tabs(&mut output, level);
                output.push_str(&src[start..line.trimmed_end]);
            }
            LineKind::Code { .. } => {
                let level = line.formatted_level.unwrap_or(line.original_level);
                if level == line.original_level {
                    output.push_str(&src[line.start..line.indent_end]);
                } else {
                    append_tabs(&mut output, level);
                }
                output.push_str(&src[line.indent_end..line.trimmed_end]);
            }
        }
        if line.terminator_end > line.body_end {
            output.push_str(&src[line.body_end..line.terminator_end]);
        } else if index == last_content {
            output.push('\n');
        }
    }
    output
}

fn append_tabs(output: &mut String, level: u32) {
    for _ in 0..level {
        output.push('\t');
    }
}
