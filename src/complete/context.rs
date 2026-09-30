use super::ContextKind;
use crate::dsp;
use crate::reader::layout;
use crate::reader::span::FileId;
use crate::reader::{
    lexer::{lex_line, Tok},
    node::{Op, Trivia},
};

pub(crate) struct Scan {
    pub context: ContextKind,
    pub from: usize,
}

pub(crate) fn scan(text: &str, cursor: usize) -> Scan {
    let bytes = text.as_bytes();
    let line_start = bytes
        .get(..cursor)
        .and_then(|s| s.iter().rposition(|b| *b == b'\n'))
        .map_or(0, |n| n + 1);
    let mut quoted = false;
    let mut escaped = false;
    let mut interpolation = 0usize;
    let mut comment = false;
    for b in bytes.get(line_start..cursor).unwrap_or(&[]) {
        if comment {
            break;
        }
        if quoted {
            if escaped {
                escaped = false;
                continue;
            }
            match *b {
                b'\\' => escaped = true,
                b'"' if interpolation == 0 => quoted = false,
                b'{' => interpolation = interpolation.saturating_add(1),
                b'}' => interpolation = interpolation.saturating_sub(1),
                _ => {}
            }
        } else {
            match *b {
                b'"' => quoted = true,
                b'#' => comment = true,
                _ => {}
            }
        }
    }
    let mut from = cursor;
    while from > line_start
        && bytes
            .get(from - 1)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-')
    {
        from -= 1;
    }
    if from > line_start && bytes.get(from - 1) == Some(&b'.') {
        from -= 1;
        while from > line_start
            && bytes
                .get(from - 1)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-')
        {
            from -= 1;
        }
    }
    if from > line_start && bytes.get(from - 1) == Some(&b':') {
        from -= 1;
    }
    if quoted && interpolation == 0 || comment {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    let line_end = bytes
        .get(cursor..)
        .and_then(|s| s.iter().position(|b| *b == b'\n'))
        .map_or(text.len(), |n| cursor + n);
    let lex = lex_line(text, line_start, line_end, FileId::new(1));
    let tokens: Vec<_> = lex
        .tokens
        .into_iter()
        .filter(|t| (t.span.end as usize) <= from)
        .collect();
    let prev = tokens.last();
    let before = bytes.get(..from).and_then(|s| s.last()).copied();
    let word = text.get(from..cursor).unwrap_or("");
    if word.as_bytes().first().is_some_and(u8::is_ascii_digit)
        || word.starts_with("-") && word.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
    {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    if matches!(prev.map(|t| &t.kind), Some(Tok::PairColon)) {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    if matches!(prev.map(|t| &t.kind), Some(Tok::Ident(s)) if matches!(&**s, "let"|"var"|"fn"|"inst"|"struct"|"enum"|"bus"|"look"))
    {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    if matches!(tokens.first().map(|t| &t.kind), Some(Tok::Ident(s)) if matches!(&**s, "fn" | "inst"))
        && tokens
            .iter()
            .skip(2)
            .all(|t| !matches!(t.kind, Tok::BlockColon | Tok::PairColon))
        && !word.starts_with(':')
    {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    if word == ":"
        && from > line_start
        && bytes
            .get(from - 1)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-')
    {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    if word.starts_with(':')
        && lex_line(text, line_start, cursor, FileId::new(1))
            .tokens
            .last()
            .is_some_and(|t| matches!(t.kind, Tok::PairColon))
    {
        return Scan {
            context: ContextKind::None,
            from,
        };
    }
    if word.starts_with(':') {
        return Scan {
            context: ContextKind::Keyword,
            from,
        };
    }
    if word.contains('.') {
        return Scan {
            context: ContextKind::Qualified,
            from,
        };
    }
    if prev.is_some_and(|t| t.is_op(Op::Gt))
        && !matches!(
            tokens.get(tokens.len().saturating_sub(2)).map(|t| &t.kind),
            Some(Tok::LBrace)
        )
    {
        return Scan {
            context: ContextKind::PipeTarget,
            from,
        };
    }
    if prev.is_none()
        || matches!(
            prev.map(|t| &t.kind),
            Some(Tok::LBrace) | Some(Tok::Op(Op::Arrow))
        )
    {
        return Scan {
            context: ContextKind::Head,
            from,
        };
    }
    let head = tokens.iter().find_map(|t| match &t.kind {
        Tok::Ident(n) => Some(n.as_ref()),
        _ => None,
    });
    if let Some(head) = head {
        let sig = crate::types::natives::NativeTable::global()
            .get(head)
            .map(|(_, s)| s);
        let (doc_keys, doc_arity) = document_header(text, head);
        let has_keys = sig.is_some_and(|s| !s.keywords.is_empty())
            || dsp::meta::decl_for(head).is_some_and(|d| !d.params.is_empty())
            || !doc_keys.is_empty();
        let prior_pair = tokens.iter().any(|t| matches!(t.kind, Tok::PairColon));
        let positional = tokens
            .iter()
            .filter(|t| {
                matches!(
                    t.kind,
                    Tok::Int(_)
                        | Tok::Float { .. }
                        | Tok::Ratio(_)
                        | Tok::Str(_)
                        | Tok::Keyword(_)
                        | Tok::LBrace
                        | Tok::LBracket
                        | Tok::Ident(_)
                )
            })
            .count()
            .saturating_sub(1);
        let max_args = sig.and_then(|s| s.max_args).map(usize::from).or(doc_arity);
        let at_arity = max_args.is_some_and(|n| positional >= n);
        if has_keys && (prior_pair || at_arity) {
            return Scan {
                context: ContextKind::PairKey,
                from,
            };
        }
    }
    let mut trivia = Trivia::default();
    let _ = crate::reader::layout::split_lines(text, FileId::new(1), &mut trivia);
    let _ = before;
    Scan {
        context: ContextKind::Argument,
        from,
    }
}

fn document_header(text: &str, name: &str) -> (Vec<String>, Option<usize>) {
    let mut trivia = Trivia::default();
    let mut lines = layout::split_lines(text, FileId::new(1), &mut trivia);
    let statements = layout::statements(&mut lines);
    for stmt in statements {
        let is_decl = matches!(stmt.tokens.first().map(|t| &t.kind), Some(Tok::Ident(k)) if matches!(&**k, "fn" | "inst"));
        let named =
            matches!(stmt.tokens.get(1).map(|t| &t.kind), Some(Tok::Ident(n)) if &**n == name);
        if !is_decl || !named {
            continue;
        }
        let mut keys = Vec::new();
        let mut positional = 0usize;
        let mut in_pair_value = false;
        for (i, token) in stmt.tokens.iter().enumerate().skip(2) {
            if matches!(token.kind, Tok::PairColon) {
                in_pair_value = true;
                continue;
            }
            if let Tok::Ident(key) = &token.kind {
                if stmt
                    .tokens
                    .get(i + 1)
                    .is_some_and(|next| matches!(next.kind, Tok::PairColon))
                {
                    keys.push(key.to_string());
                    in_pair_value = false;
                } else if !in_pair_value {
                    positional = positional.saturating_add(1);
                }
            }
        }
        return (keys, (positional > 0).then_some(positional));
    }
    (Vec::new(), None)
}
