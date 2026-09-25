//! The `#@` directive grammar (design 13.5 PROPOSED vocabulary, 14.5.8).
//!
//! One directive per line. The first token classifies it: `midi` and a
//! `label.sel[.n]` head are Addressed (position-free); everything else is
//! Positional and resolves against its attach target. A token starting with
//! `#` ends the directive (the rest is an ordinary comment).

use std::rc::Rc;

use crate::reader::span::Span;
use crate::types::diag::{DiagCode, Diagnostic};

/// Positional (attached) or Addressed (position-free), by first token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirectiveKind {
    Positional,
    Addressed,
}

/// One parsed directive line.
#[derive(Clone, PartialEq, Debug)]
pub struct Directive {
    /// From `#` to the end of the line text.
    pub span: Span,
    pub kind: DirectiveKind,
    pub body: DirectiveBody,
    /// Where the pairs start (the end of the head), for write-back.
    pub head_end: u32,
}

/// What a directive says.
#[derive(Clone, PartialEq, Debug)]
pub enum DirectiveBody {
    /// `#@ midi ch: 1`.
    FileDefault { pairs: Vec<PairAt> },
    /// `#@ name X [sites..]` or the trailing short form `#@ X: [sites..]`.
    LabelDef {
        name: Rc<str>,
        name_span: Span,
        rest: Option<Positional>,
    },
    /// `#@ lpf hpf cc: 74 71`, `#@ cutoff res`, `#@ lpf.2 cc: 30`.
    Positional(Positional),
    /// `#@ hats.hpf cc: 30`, `#@ analog.cutoff cc: 1`, `#@ hats.lpf.2`.
    Addressed {
        label: Rc<str>,
        label_span: Span,
        sel: Rc<str>,
        sel_span: Span,
        ordinal: Option<u16>,
        pairs: Vec<PairAt>,
    },
    /// Nothing usable after `#@` (already diagnosed, or blank).
    Empty,
}

/// The sites and pairs of a positional directive.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Positional {
    pub sites: Vec<SiteSel>,
    pub pairs: Vec<PairAt>,
}

/// A call-site name or parameter keyword, with an optional ordinal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SiteSel {
    pub name: Rc<str>,
    pub ordinal: Option<u16>,
    pub span: Span,
}

/// A `key: value+` pair. `_` in a `cc:` list is `None` (skip).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Pair {
    Cc(Vec<Option<u8>>),
    Ch(u8),
}

/// A pair with the spans write-back edits need.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PairAt {
    pub pair: Pair,
    pub key_span: Span,
    pub value_spans: Vec<Span>,
}

impl PairAt {
    /// The span from the key to the last value.
    #[must_use]
    pub fn span(&self) -> Span {
        self.value_spans
            .last()
            .map_or(self.key_span, |v| self.key_span.join(*v))
    }
}

/// The pairs of a directive body, if it has any.
#[must_use]
pub fn pairs_of(body: &DirectiveBody) -> &[PairAt] {
    match body {
        DirectiveBody::FileDefault { pairs } | DirectiveBody::Addressed { pairs, .. } => pairs,
        DirectiveBody::Positional(p) => &p.pairs,
        DirectiveBody::LabelDef { rest: Some(p), .. } => &p.pairs,
        DirectiveBody::LabelDef { rest: None, .. } | DirectiveBody::Empty => &[],
    }
}

/// The `cc:` list of a directive, if any (the last one when repeated).
#[must_use]
pub fn cc_of(pairs: &[PairAt]) -> Option<&PairAt> {
    pairs.iter().rev().find(|p| matches!(p.pair, Pair::Cc(_)))
}

/// The `ch:` value of a directive, if any.
#[must_use]
pub fn ch_of(pairs: &[PairAt]) -> Option<u8> {
    pairs.iter().rev().find_map(|p| match p.pair {
        Pair::Ch(c) => Some(c),
        Pair::Cc(_) => None,
    })
}

/// One whitespace-separated token with its absolute span.
#[derive(Clone, Copy, Debug)]
struct Tok<'a> {
    text: &'a str,
    span: Span,
}

fn warn(code: DiagCode, span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        span,
        severity: code.default_severity(),
        code,
        message: message.into(),
        origin: None,
    }
}

/// Splits the text after `#@` into tokens, stopping at a `#` token.
fn tokens(text: &str, span: Span) -> Vec<Tok<'_>> {
    let mut out = Vec::new();
    let body_start = text.find("#@").map_or(0, |k| k + 2);
    let mut i = body_start;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let word = &text[start..i];
        if word.starts_with('#') {
            break;
        }
        let at = |k: usize| {
            span.start
                .saturating_add(u32::try_from(k).unwrap_or(u32::MAX))
        };
        out.push(Tok {
            text: word,
            span: Span::new(span.file, at(start), at(i)),
        });
    }
    out
}

/// A pair key token (`cc:`), returning the key without the colon.
fn key_of<'a>(tok: &Tok<'a>) -> Option<&'a str> {
    let k = tok.text.strip_suffix(':')?;
    is_ident(k).then_some(k)
}

/// An identifier: letters, digits, `-`, `_`, starting with a letter.
fn is_ident(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '?')
}

/// The 1-based ordinal segment of `name.n`.
fn ordinal(s: &str) -> Option<u16> {
    s.parse::<u16>().ok().filter(|n| *n >= 1)
}

/// Parses one directive comment. `text` is the trivia text (`#@ ...`) and
/// `span` its span; diagnostics are warnings.
#[must_use]
pub fn parse_directive(text: &str, span: Span, diags: &mut Vec<Diagnostic>) -> Directive {
    let toks = tokens(text, span);
    let mut out = Directive {
        span,
        kind: DirectiveKind::Positional,
        body: DirectiveBody::Empty,
        head_end: span.end,
    };
    let Some(first) = toks.first() else {
        return out;
    };
    let pair_start = toks
        .iter()
        .skip(1)
        .position(|t| key_of(t).is_some())
        .map_or(toks.len(), |k| k + 1);
    out.head_end = toks
        .get(pair_start.saturating_sub(1))
        .map_or(span.end, |t| t.span.end);
    if first.text == "midi" {
        out.kind = DirectiveKind::Addressed;
        let pairs = parse_pairs(&toks[1..], diags);
        for p in &pairs {
            if matches!(p.pair, Pair::Cc(_)) {
                diags.push(warn(
                    DiagCode::ReservedKey,
                    p.key_span,
                    "`midi` takes only `ch:` in v1",
                ));
            }
        }
        out.body = DirectiveBody::FileDefault {
            pairs: pairs
                .into_iter()
                .filter(|p| matches!(p.pair, Pair::Ch(_)))
                .collect(),
        };
        out.head_end = first.span.end;
        return out;
    }
    if first.text == "name" {
        let Some(name) = toks.get(1).filter(|t| is_ident(t.text)) else {
            diags.push(warn(
                DiagCode::UnknownDirectiveSite,
                first.span,
                "`#@ name` needs a label name",
            ));
            return out;
        };
        let rest = positional(&toks[2..], diags);
        out.body = DirectiveBody::LabelDef {
            name: Rc::from(name.text),
            name_span: name.span,
            rest,
        };
        return out;
    }
    if let Some(key) = key_of(first) {
        if matches!(key, "cc" | "ch" | "range") || !is_label_head(key) {
            // A pair with no site: parsed for its own diagnostics.
            let _ = parse_pairs(&toks, diags);
            diags.push(warn(
                DiagCode::UnknownDirectiveSite,
                first.span,
                format!("`{}` needs a call site or parameter before it", first.text),
            ));
            return out;
        }
        let name_span = Span::new(span.file, first.span.start, first.span.end - 1);
        out.body = DirectiveBody::LabelDef {
            name: Rc::from(key),
            name_span,
            rest: positional(&toks[1..], diags),
        };
        return out;
    }
    let mut parts = first.text.split('.');
    let head = parts.next().unwrap_or("");
    let second = parts.next();
    let third = parts.next();
    if let Some(sel) = second.filter(|s| ordinal(s).is_none()) {
        out.kind = DirectiveKind::Addressed;
        let ord = match third {
            None => None,
            Some(n) => match ordinal(n) {
                Some(n) if parts.next().is_none() => Some(n),
                _ => {
                    diags.push(warn(
                        DiagCode::UnknownDirectiveSite,
                        first.span,
                        format!("`{}` is not `label.selector[.n]`", first.text),
                    ));
                    return out;
                }
            },
        };
        if !is_ident(head) || !is_ident(sel) {
            diags.push(warn(
                DiagCode::UnknownDirectiveSite,
                first.span,
                format!("`{}` is not `label.selector[.n]`", first.text),
            ));
            return out;
        }
        let label_start = first.span.start;
        let label_end = label_start + u32::try_from(head.len()).unwrap_or(0);
        let sel_start = label_end + 1;
        let sel_end = sel_start + u32::try_from(sel.len()).unwrap_or(0);
        out.body = DirectiveBody::Addressed {
            label: Rc::from(head),
            label_span: Span::new(span.file, label_start, label_end),
            sel: Rc::from(sel),
            sel_span: Span::new(span.file, sel_start, sel_end),
            ordinal: ord,
            pairs: parse_pairs(&toks[1..], diags),
        };
        return out;
    }
    if let Some(p) = positional(&toks, diags) {
        out.body = DirectiveBody::Positional(p);
    }
    out
}

/// A label name usable in the trailing short form.
fn is_label_head(key: &str) -> bool {
    is_ident(key)
}

/// `site+ pair*`. `None` when there are no tokens at all.
fn positional(toks: &[Tok<'_>], diags: &mut Vec<Diagnostic>) -> Option<Positional> {
    if toks.is_empty() {
        return None;
    }
    let split = toks
        .iter()
        .position(|t| key_of(t).is_some())
        .unwrap_or(toks.len());
    let mut sites = Vec::new();
    for t in &toks[..split] {
        let (name, ord) = match t.text.split_once('.') {
            Some((name, n)) => match ordinal(n) {
                Some(n) => (name, Some(n)),
                None => {
                    diags.push(warn(
                        DiagCode::UnknownDirectiveSite,
                        t.span,
                        format!("`{}` is not a site name or `name.n`", t.text),
                    ));
                    continue;
                }
            },
            None => (t.text, None),
        };
        if !is_ident(name) {
            diags.push(warn(
                DiagCode::UnknownDirectiveSite,
                t.span,
                format!("`{}` is not a site name", t.text),
            ));
            continue;
        }
        sites.push(SiteSel {
            name: Rc::from(name),
            ordinal: ord,
            span: t.span,
        });
    }
    if sites.is_empty() && split > 0 {
        return None;
    }
    if sites.is_empty() {
        diags.push(warn(
            DiagCode::UnknownDirectiveSite,
            toks[0].span,
            "a positional directive needs a call site or parameter",
        ));
    }
    Some(Positional {
        sites,
        pairs: parse_pairs(&toks[split..], diags),
    })
}

/// `(key ":" value+)*`: `cc:` ints 0..127 and `_`, `ch:` one int 1..16.
/// Any other key is `reserved-key`; a repeated key is `duplicate-key`.
fn parse_pairs(toks: &[Tok<'_>], diags: &mut Vec<Diagnostic>) -> Vec<PairAt> {
    let mut out: Vec<PairAt> = Vec::new();
    let mut k = 0;
    while k < toks.len() {
        let tok = toks[k];
        k += 1;
        let Some(key) = key_of(&tok) else {
            diags.push(warn(
                DiagCode::UnknownDirectiveSite,
                tok.span,
                format!("unexpected `{}` among the pairs", tok.text),
            ));
            continue;
        };
        let start = k;
        while k < toks.len() && key_of(&toks[k]).is_none() {
            k += 1;
        }
        let values = &toks[start..k];
        let value_spans: Vec<Span> = values.iter().map(|t| t.span).collect();
        let pair = match key {
            "cc" => {
                let mut ccs = Vec::new();
                let mut ok = true;
                for v in values {
                    if v.text == "_" {
                        ccs.push(None);
                        continue;
                    }
                    match v.text.parse::<u8>() {
                        Ok(n) if n <= 127 => ccs.push(Some(n)),
                        _ => {
                            ok = false;
                            diags.push(warn(
                                DiagCode::CcOutOfRange,
                                v.span,
                                format!("`{}` is not a CC number 0..127 or `_`", v.text),
                            ));
                        }
                    }
                }
                ok.then_some(Pair::Cc(ccs))
            }
            "ch" => match values {
                [v] => match v.text.parse::<u8>() {
                    Ok(n) if (1..=16).contains(&n) => Some(Pair::Ch(n)),
                    _ => {
                        diags.push(warn(
                            DiagCode::CcOutOfRange,
                            v.span,
                            format!("`{}` is not a MIDI channel 1..16", v.text),
                        ));
                        None
                    }
                },
                _ => {
                    diags.push(warn(
                        DiagCode::CcOutOfRange,
                        tok.span,
                        "`ch:` takes exactly one channel 1..16",
                    ));
                    None
                }
            },
            other => {
                diags.push(warn(
                    DiagCode::ReservedKey,
                    tok.span,
                    format!("`{other}:` is reserved; ranges and editor kinds come from ParamMeta"),
                ));
                None
            }
        };
        let Some(pair) = pair else {
            continue;
        };
        let same = |p: &PairAt| std::mem::discriminant(&p.pair) == std::mem::discriminant(&pair);
        if out.iter().any(same) {
            diags.push(warn(
                DiagCode::DuplicateKey,
                tok.span,
                format!("`{key}:` is given twice; the later one wins"),
            ));
            out.retain(|p| !same(p));
        }
        out.push(PairAt {
            pair,
            key_span: tok.span,
            value_spans,
        });
    }
    out
}
