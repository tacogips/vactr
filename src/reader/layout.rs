//! Physical lines to statements (6.3, 6.5.4 "Layout").
//!
//! Layout only groups lines: a statement is its first line, the `>` lines
//! that continue it, and the block its trailing colon opens. `line.rs` reads
//! the tokens; `group_if_chains` then joins `if`/`elif`/`else` statements.

use crate::reader::lexer::{lex_line, Tok, Token};
use crate::reader::node::{Node, NodeKind, Op, Trivia};
use crate::reader::span::{FileId, Span};
use crate::reader::MAX_NESTING;
use crate::types::diag::{DiagCode, Diagnostic};

/// One non-blank, non-comment-only line.
#[derive(Debug)]
pub(crate) struct Line {
    /// The number of leading tabs.
    pub level: u32,
    /// A space appears in the indentation.
    pub indent_space: bool,
    /// Tokens, without a trailing block colon.
    pub tokens: Vec<Token>,
    pub diags: Vec<Diagnostic>,
    /// From the first code byte to the last one before any comment.
    pub span: Span,
    pub block_colon: Option<Span>,
    pub starts_with_gt: bool,
    pub is_import: bool,
}

/// A statement: tokens of its lines, its diagnostics, and its block.
#[derive(Debug)]
pub(crate) struct Stmt {
    pub tokens: Vec<Token>,
    pub diags: Vec<Diagnostic>,
    pub span: Span,
    pub block: Option<BlockSkel>,
    /// The code span of the first line when the statement is headed `import`.
    pub import: Option<Span>,
    /// Continuation lines were appended.
    pub continued: bool,
}

/// The body a trailing block colon opens.
#[derive(Debug)]
pub(crate) struct BlockSkel {
    pub stmts: Vec<Stmt>,
    pub colon: Span,
    pub span: Span,
}

impl Stmt {
    fn empty(span: Span) -> Stmt {
        Stmt {
            tokens: Vec::new(),
            diags: Vec::new(),
            span,
            block: None,
            import: None,
            continued: false,
        }
    }

    fn error(first: Span, span: Span, code: DiagCode, msg: &str) -> Stmt {
        let mut stmt = Stmt::empty(span);
        stmt.diags.push(Diagnostic::error(code, first, msg));
        stmt
    }
}

fn to_u32(x: usize) -> u32 {
    u32::try_from(x).unwrap_or(u32::MAX)
}

/// Splits `src` into lines, lexes each one, and records every comment in
/// `trivia`. Blank and comment-only lines are dropped.
pub(crate) fn split_lines(src: &str, file: FileId, trivia: &mut Trivia) -> Vec<Line> {
    let bytes = src.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start <= bytes.len() {
        let nl = bytes[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |k| start + k);
        let end = if nl > start && bytes.get(nl - 1) == Some(&b'\r') {
            nl - 1
        } else {
            nl
        };
        if let Some(line) = one_line(src, start, end, file, trivia) {
            lines.push(line);
        }
        start = nl + 1;
    }
    lines
}

fn one_line(
    src: &str,
    start: usize,
    end: usize,
    file: FileId,
    trivia: &mut Trivia,
) -> Option<Line> {
    let bytes = src.as_bytes();
    let mut content = start;
    let mut tabs = 0u32;
    let mut indent_space = false;
    while content < end {
        match bytes.get(content) {
            Some(b'\t') => tabs = tabs.saturating_add(1),
            Some(b' ') => indent_space = true,
            _ => break,
        }
        content += 1;
    }
    let mut lexed = lex_line(src, content, end, file);
    if let Some(c) = lexed.comment {
        trivia.items.push(c);
    }
    if lexed.tokens.is_empty() && lexed.diags.is_empty() {
        return None;
    }
    let mut code_end = lexed.code_end;
    while code_end > content && matches!(bytes.get(code_end - 1), Some(b' ' | b'\t')) {
        code_end -= 1;
    }
    let block_colon = match lexed.tokens.last() {
        Some(t) if t.kind == Tok::BlockColon => Some(t.span),
        _ => None,
    };
    if block_colon.is_some() {
        lexed.tokens.pop();
    }
    let first = lexed.tokens.first();
    let starts_with_gt = first.is_some_and(|t| t.is_op(Op::Gt));
    let is_import = first.is_some_and(|t| matches!(&t.kind, Tok::Ident(n) if &**n == "import"));
    Some(Line {
        level: tabs,
        indent_space,
        tokens: lexed.tokens,
        diags: lexed.diags,
        span: Span::new(file, to_u32(content), to_u32(code_end)),
        block_colon,
        starts_with_gt,
        is_import,
    })
}

/// The top-level statements of a document.
pub(crate) fn statements(lines: &mut [Line]) -> Vec<Stmt> {
    let mut i = 0;
    body(lines, &mut i, 0, 0)
}

/// Statements at exactly `level`, until a shallower line.
fn body(lines: &mut [Line], i: &mut usize, level: u32, nest: u32) -> Vec<Stmt> {
    let mut out = Vec::new();
    while let Some(line) = lines.get(*i) {
        if line.level < level {
            break;
        }
        if line.indent_space {
            let span = line.span;
            out.push(Stmt::error(
                span,
                span,
                DiagCode::IndentSpace,
                "indent with tabs only",
            ));
            *i += 1;
            continue;
        }
        if line.level > level {
            let first = line.span;
            let mut span = first;
            *i += 1;
            while let Some(next) = lines.get(*i) {
                if next.level <= level {
                    break;
                }
                span = span.join(next.span);
                *i += 1;
            }
            out.push(Stmt::error(
                first,
                span,
                DiagCode::UnexpectedIndent,
                "unexpected indentation",
            ));
            continue;
        }
        out.push(statement(lines, i, level, nest));
    }
    out
}

/// One statement at `level`: its line, its continuation lines, its block.
fn statement(lines: &mut [Line], i: &mut usize, level: u32, nest: u32) -> Stmt {
    let Some(line) = lines.get_mut(*i) else {
        return Stmt::empty(Span::new(FileId::CONSOLE, 0, 0));
    };
    *i += 1;
    let mut stmt = Stmt::empty(line.span);
    stmt.tokens = std::mem::take(&mut line.tokens);
    if line.is_import {
        stmt.import = Some(line.span);
    } else {
        stmt.diags = std::mem::take(&mut line.diags);
    }
    let (line_level, colon) = (line.level, line.block_colon);
    if let Some(colon) = colon {
        stmt.block = Some(block(lines, i, line_level, nest, colon, &mut stmt.diags));
    }
    while let Some(next) = lines.get_mut(*i) {
        if next.level <= level || next.indent_space {
            break;
        }
        *i += 1;
        stmt.span = stmt.span.join(next.span);
        let (next_level, next_colon, next_span) = (next.level, next.block_colon, next.span);
        let misplaced = if !next.starts_with_gt {
            Some((DiagCode::UnexpectedIndent, "unexpected indentation"))
        } else if stmt.block.is_some() {
            Some((
                DiagCode::ContinuationAfterBlock,
                "a `>` line cannot follow a block",
            ))
        } else {
            None
        };
        if let Some((code, msg)) = misplaced {
            // The line and the lines under it belong to this statement, which
            // becomes the `Error` node; a later `>` line still continues it.
            stmt.diags.push(Diagnostic::error(code, next_span, msg));
            while let Some(inner) = lines.get(*i) {
                if inner.level <= next_level {
                    break;
                }
                stmt.span = stmt.span.join(inner.span);
                *i += 1;
            }
            continue;
        }
        stmt.continued = true;
        stmt.tokens.append(&mut next.tokens);
        stmt.diags.append(&mut next.diags);
        if let Some(colon) = next_colon {
            stmt.block = Some(block(lines, i, next_level, nest, colon, &mut stmt.diags));
        }
    }
    if let Some(b) = &stmt.block {
        stmt.span = stmt.span.join(b.span);
    }
    stmt
}

/// The body opened by a colon on a line at `opener_level`.
fn block(
    lines: &mut [Line],
    i: &mut usize,
    opener_level: u32,
    nest: u32,
    colon: Span,
    diags: &mut Vec<Diagnostic>,
) -> BlockSkel {
    let inner = opener_level.saturating_add(1);
    let mut span = colon;
    if nest + 1 > MAX_NESTING {
        while let Some(line) = lines.get(*i) {
            if line.level < inner {
                break;
            }
            span = span.join(line.span);
            *i += 1;
        }
        diags.push(Diagnostic::error(
            DiagCode::NestingTooDeep,
            colon,
            "nesting deeper than 128",
        ));
        return BlockSkel {
            stmts: Vec::new(),
            colon,
            span,
        };
    }
    let stmts = body(lines, i, inner, nest + 1);
    match (stmts.first(), stmts.last()) {
        (Some(first), Some(last)) => span = first.span.join(last.span),
        _ => diags.push(Diagnostic::error(
            DiagCode::EmptyBlock,
            colon,
            "a `:` block needs a body",
        )),
    }
    BlockSkel { stmts, colon, span }
}

/// The leading word of a statement: a `Sym` atom, a call head, or the first
/// lhs item of an arrow.
fn head_word(n: &Node) -> Option<&str> {
    match n.kind {
        NodeKind::Atom(_) => n.sym_name(),
        NodeKind::Call => n.children.first()?.sym_name(),
        NodeKind::Arrow if n.children.len() >= 2 => n.children.first()?.sym_name(),
        _ => None,
    }
}

/// True when an `elif` or `else` statement may join `n`.
fn chain_open(n: &Node) -> bool {
    match n.kind {
        NodeKind::IfChain => n.children.last().and_then(head_word) != Some("else"),
        _ => matches!(head_word(n), Some("if" | "elif")),
    }
}

/// Groups `if`/`elif`/`else` statements of one body into `IfChain` nodes.
pub(crate) fn group_if_chains(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    for n in nodes {
        if matches!(head_word(&n), Some("elif" | "else")) {
            if let Some(last) = out.last_mut() {
                if chain_open(last) {
                    let span = last.span.join(n.span);
                    let placeholder = Node::new(NodeKind::Error, span, Vec::new());
                    let prev = std::mem::replace(last, placeholder);
                    let mut children = if matches!(prev.kind, NodeKind::IfChain) {
                        prev.children.into_vec()
                    } else {
                        vec![prev]
                    };
                    children.push(n);
                    *last = Node::new(NodeKind::IfChain, span, children);
                    continue;
                }
            }
        }
        out.push(n);
    }
    out
}
