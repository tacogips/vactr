use super::{Candidate, CandidateKind};
use crate::reader::span::FileId;
use crate::reader::{
    layout,
    lexer::{Tok, Token},
    node::Trivia,
};

pub(crate) fn locals(text: &str, cursor: usize) -> Vec<Candidate> {
    let mut trivia = Trivia::default();
    let mut lines = layout::split_lines(text, FileId::new(1), &mut trivia);
    let statements = layout::statements(&mut lines);
    let mut found = Vec::new();
    let cursor_level = line_indent(text, cursor);
    collect(text, &statements, cursor, cursor_level, &mut found, 0);
    let mut seen = std::collections::BTreeSet::new();
    found.retain(|c| seen.insert(c.label.clone()));
    found
}
fn collect(
    text: &str,
    stmts: &[layout::Stmt],
    cursor: usize,
    cursor_level: usize,
    out: &mut Vec<Candidate>,
    depth: usize,
) {
    if depth > 128 {
        return;
    }
    let mut frame = None;
    let mut frame_bindings = Vec::new();
    let mut owner: Option<&layout::Stmt> = None;
    for stmt in stmts {
        let start = stmt.span.start as usize;
        if start >= cursor {
            break;
        }
        if let Some(Tok::Ident(name)) = stmt.tokens.first().map(|t| &t.kind) {
            if matches!(name.as_ref(), "let" | "var") {
                if let Some(first) = identifiers(&stmt.tokens).first() {
                    push(&mut frame_bindings, first, name.as_ref());
                }
            }
        }
        owner = Some(stmt);
    }
    // Only the last statement starting before the cursor owns a frame or
    // holds the cursor; header, loop and lambda binders never leak from
    // earlier siblings.
    if let Some(stmt) = owner {
        let start = stmt.span.start as usize;
        if cursor_level > line_indent(text, start) && stmt.block.is_some() {
            frame = stmt.block.as_ref().map(|block| block.stmts.as_slice());
            owner_binders(stmt, &mut frame_bindings);
        } else if on_statement_line(text, stmt.span.end as usize, cursor) {
            let before = stmt
                .tokens
                .iter()
                .take_while(|t| t.span.end as usize <= cursor)
                .count();
            arrow_binders(
                stmt.tokens.get(..before).unwrap_or(&[]),
                &mut frame_bindings,
            );
        }
    }
    if let Some(inner) = frame {
        collect(text, inner, cursor, cursor_level, out, depth + 1);
    }
    out.extend(frame_bindings);
}
/// True when the cursor is inside the statement or on its last line (for
/// example after a trailing space typed past the final token).
fn on_statement_line(text: &str, span_end: usize, cursor: usize) -> bool {
    cursor <= span_end
        || text
            .get(span_end..cursor)
            .is_some_and(|gap| !gap.contains('\n'))
}
fn owner_binders(stmt: &layout::Stmt, out: &mut Vec<Candidate>) {
    let keyword = match stmt.tokens.first().map(|t| &t.kind) {
        Some(Tok::Ident(name)) => name.as_ref(),
        _ => "",
    };
    match keyword {
        "fn" | "inst" => {
            let detail = match stmt.tokens.get(1).map(|token| &token.kind) {
                Some(Tok::Ident(name)) => name.as_ref(),
                _ => keyword,
            };
            header_binders(&stmt.tokens, out, detail);
        }
        "for" => for_binders(&stmt.tokens, out),
        "struct" | "enum" | "let" | "var" => {}
        _ => arrow_binders(&stmt.tokens, out),
    }
}
fn line_indent(text: &str, offset: usize) -> usize {
    let line_start = text
        .get(..offset)
        .and_then(|prefix| prefix.rfind('\n'))
        .map_or(0, |newline| newline.saturating_add(1));
    text.as_bytes()
        .get(line_start..offset)
        .unwrap_or(&[])
        .iter()
        .take_while(|byte| **byte == b'\t')
        .count()
}
fn identifiers(tokens: &[Token]) -> Vec<String> {
    tokens
        .iter()
        .filter_map(|t| match &t.kind {
            Tok::Ident(name)
                if !matches!(
                    &**name,
                    "if" | "elif" | "else" | "match" | "for" | "fn" | "let" | "var" | "upd"
                ) =>
            {
                Some(name.to_string())
            }
            _ => None,
        })
        .collect()
}
fn header_binders(tokens: &[Token], out: &mut Vec<Candidate>, detail: &str) {
    let mut in_default = false;
    for (index, token) in tokens.iter().enumerate().skip(2) {
        if matches!(token.kind, Tok::PairColon) {
            in_default = true;
            continue;
        }
        if matches!(token.kind, Tok::BlockColon) {
            break;
        }
        if let Tok::Ident(name) = &token.kind {
            let followed_by_pair = tokens
                .get(index + 1)
                .is_some_and(|next| matches!(next.kind, Tok::PairColon));
            if followed_by_pair {
                push(out, name, detail);
                in_default = false;
            } else if !in_default {
                push(out, name, detail);
            }
        }
    }
}
fn for_binders(tokens: &[Token], out: &mut Vec<Candidate>) {
    let Some(for_at) = tokens
        .iter()
        .position(|t| matches!(&t.kind, Tok::Ident(name) if &**name == "for"))
    else {
        return;
    };
    let pattern = tokens.get(for_at.saturating_add(1));
    if matches!(pattern.map(|t| &t.kind), Some(Tok::LBracket)) {
        for token in tokens.iter().skip(for_at.saturating_add(2)) {
            if matches!(token.kind, Tok::RBracket) {
                break;
            }
            if let Tok::Ident(name) = &token.kind {
                push(out, name, "for");
            }
        }
    } else if let Some(Token {
        kind: Tok::Ident(name),
        ..
    }) = pattern
    {
        push(out, name, "for");
    }
}
fn arrow_binders(tokens: &[Token], out: &mut Vec<Candidate>) {
    let Some(arrow) = tokens
        .iter()
        .rposition(|t| matches!(t.kind, Tok::Op(crate::reader::node::Op::Arrow)))
    else {
        return;
    };
    let left = tokens.get(..arrow).unwrap_or(&[]);
    let has_pattern_pairs = left.iter().any(|t| matches!(t.kind, Tok::PairColon));
    let start = left
        .iter()
        .rposition(|t| matches!(t.kind, Tok::LBrace))
        .map_or(0, |i| i.saturating_add(1));
    let segment = left.get(start..).unwrap_or(&[]);
    let skip_constructor =
        has_pattern_pairs || segment.iter().any(|t| matches!(t.kind, Tok::LBracket));
    let mut saw_constructor = false;
    for (i, token) in segment.iter().enumerate() {
        let Tok::Ident(name) = &token.kind else {
            continue;
        };
        if matches!(
            &**name,
            "if" | "elif" | "else" | "match" | "for" | "fn" | "let" | "var" | "upd"
        ) {
            continue;
        }
        if segment
            .get(i + 1)
            .is_some_and(|next| matches!(next.kind, Tok::PairColon))
        {
            continue;
        }
        if skip_constructor && !saw_constructor {
            saw_constructor = true;
            continue;
        }
        push(out, name, "lambda");
    }
}
fn push(out: &mut Vec<Candidate>, name: &str, detail: &str) {
    out.push(Candidate {
        label: name.to_owned(),
        kind: CandidateKind::Local,
        detail: detail.to_owned(),
        insert: name.to_owned(),
    });
}
