//! Learned-CC write-back into directive comments (design 13.5 "Write-back
//! of learned mappings", 14.5.8).
//!
//! `learn_edit` makes the minimal change that puts the learned CC number at
//! the binding's position in its governing directive, or appends a new
//! directive block after the target statement when no directive names the
//! binding. The edit's `expected` text is the directive's current text, so
//! the editor verifies it (with the `edit_epoch` check of 14.5.6) before
//! applying. A stale binding is refused.

use std::fmt;

use crate::directives::key::{BindingIdent, KeyState, KeyTable};
use crate::directives::parse::{cc_of, ch_of, DirectiveBody, Pair};
use crate::directives::persist::TextEdit;
use crate::directives::DirectiveTable;
use crate::reader::span::Span;

/// Why a learn event produces no edit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LearnError {
    /// The binding's key is stale: re-confirm it first.
    Stale,
    /// The identity is not a call-site or definition parameter directives
    /// can name (a literal tweak site keeps positional `TweakId` provenance).
    NotDirectiveBacked,
    /// The key does not resolve in this revision.
    Unknown,
}

impl fmt::Display for LearnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LearnError::Stale => "the binding is stale; re-confirm it before learning",
            LearnError::NotDirectiveBacked => "the site cannot be named by a directive",
            LearnError::Unknown => "the binding does not resolve in this revision",
        })
    }
}

impl std::error::Error for LearnError {}

/// `cc: a _ b` and `ch: n` pair text (leading space), or empty.
#[must_use]
pub fn render_pairs(ccs: &[Option<u8>], ch: Option<u8>) -> String {
    let mut out = String::new();
    if !ccs.is_empty() {
        out.push_str(" cc:");
        for c in ccs {
            match c {
                Some(n) => out.push_str(&format!(" {n}")),
                None => out.push_str(" _"),
            }
        }
    }
    if let Some(ch) = ch {
        out.push_str(&format!(" ch: {ch}"));
    }
    out
}

/// A `cc:` list with `cc` at `slot` and `_` before it.
#[must_use]
pub fn cc_list_at(slot: usize, cc: u8) -> Vec<Option<u8>> {
    let mut v = vec![None; slot];
    v.push(Some(cc));
    v
}

/// The innermost target containing `span`.
#[must_use]
pub fn owner_target(table: &DirectiveTable, span: Span) -> Option<usize> {
    table
        .doc
        .targets
        .iter()
        .enumerate()
        .filter(|(_, t)| t.extent.start <= span.start && span.end <= t.extent.end)
        .max_by_key(|(_, t)| t.extent.start)
        .map(|(k, _)| k)
}

/// Where a new own-line directive for target `t` goes: the end of its last
/// line, past any own-line `#@` lines right after it, and the indentation
/// to write.
#[must_use]
pub fn append_point(src: &str, table: &DirectiveTable, t: usize) -> (u32, String) {
    let target = &table.doc.targets[t];
    let lines = &table.doc.lines;
    let line_end = |line: usize| -> u32 {
        lines.start(line + 1).map_or_else(
            || u32::try_from(src.len()).unwrap_or(u32::MAX),
            |next| next.saturating_sub(1),
        )
    };
    let mut last = target.last_line;
    while last + 1 < lines.count() {
        let first = lines.first(last + 1) as usize;
        if src.get(first..).is_some_and(|s| s.starts_with("#@")) {
            last += 1;
        } else {
            break;
        }
    }
    let start = lines.start(target.first_line).unwrap_or(0) as usize;
    let indent = src
        .get(start..start + target.indent as usize)
        .unwrap_or("")
        .to_string();
    let mut at = line_end(last);
    if at > 0 && src.as_bytes().get(at as usize - 1) == Some(&b'\r') {
        at -= 1;
    }
    (at, indent)
}

/// An insertion of own-line directive `lines` after target `t`.
#[must_use]
pub fn append_after(src: &str, table: &DirectiveTable, t: usize, lines: &[String]) -> TextEdit {
    let (at, indent) = append_point(src, table, t);
    let mut text = String::new();
    for l in lines {
        text.push('\n');
        text.push_str(&indent);
        text.push_str(l);
    }
    TextEdit {
        span: (at, at),
        expected: String::new(),
        text,
    }
}

/// The positional head naming site `head` (`lpf` or `lpf.2`) within its
/// innermost target, or the parameter name for a definition parameter.
fn positional_head(table: &DirectiveTable, head: Span, param: &str) -> Option<(usize, String)> {
    let t = owner_target(table, head)?;
    if let Some(def) = &table.doc.targets[t].def {
        if def.params.iter().any(|(_, s)| *s == head) {
            return Some((t, param.to_string()));
        }
    }
    let site = table.doc.sites.iter().find(|s| s.head == head)?;
    let same: Vec<Span> = table
        .doc
        .sites_in(table.doc.targets[t].extent)
        .filter(|(_, s)| s.name == site.name)
        .map(|(_, s)| s.head)
        .collect();
    let name = if same.len() > 1 {
        let n = same.iter().position(|s| *s == head)? + 1;
        format!("{}.{n}", site.name)
    } else {
        site.name.to_string()
    };
    Some((t, name))
}

/// The call-site or parameter span and declared index of `ident` in this
/// revision.
fn locate(table: &DirectiveTable, ident: &BindingIdent) -> Result<(Span, usize), LearnError> {
    match ident {
        BindingIdent::Key(key) => {
            let (params, span) = table.key_target(key).ok_or(LearnError::Unknown)?;
            let index = params
                .iter()
                .position(|p| *p == key.param)
                .ok_or(LearnError::Unknown)?;
            Ok((span, index))
        }
        BindingIdent::Positional { span, param } => {
            let file = table.doc.file.ok_or(LearnError::NotDirectiveBacked)?;
            let span = Span::new(file, span.0, span.1);
            if let Some(site) = table.doc.sites.iter().find(|s| s.head == span) {
                let index = site
                    .params
                    .iter()
                    .position(|p| p == param)
                    .ok_or(LearnError::NotDirectiveBacked)?;
                return Ok((span, index));
            }
            for t in &table.doc.targets {
                if let Some(def) = &t.def {
                    if let Some(k) = def.params.iter().position(|(_, s)| *s == span) {
                        return Ok((span, k));
                    }
                }
            }
            Err(LearnError::NotDirectiveBacked)
        }
    }
}

/// The edit that writes learned `cc` (and `ch`, when given and different
/// from the binding's channel) for `target` into the directives of
/// `doc_text`.
///
/// # Errors
/// `Stale` for a stale key, `Unknown` when the key does not resolve, and
/// `NotDirectiveBacked` for an identity no directive can name.
pub fn learn_edit(
    doc_text: &str,
    table: &DirectiveTable,
    keys: &KeyTable,
    target: &BindingIdent,
    cc: u8,
    ch: Option<u8>,
) -> Result<TextEdit, LearnError> {
    if let BindingIdent::Key(k) = target {
        if keys.state(k) == Some(KeyState::Stale) {
            return Err(LearnError::Stale);
        }
    }
    let (span, index) = locate(table, target)?;
    let file_ch = table.file_level.midi_ch;
    let Some(r) = table.binding(target) else {
        // No directive names it: a new block after the target statement.
        let (t, head) =
            positional_head(table, span, param_of(target)).ok_or(LearnError::NotDirectiveBacked)?;
        let slot = if is_def_param(table, span) { 0 } else { index };
        let ch = ch.filter(|c| Some(*c) != file_ch);
        let line = format!("#@ {head}{}", render_pairs(&cc_list_at(slot, cc), ch));
        return Ok(append_after(doc_text, table, t, &[line]));
    };
    let placed = &table.entries[r.directive];
    let d = &placed.directive;
    let text = doc_text
        .get(d.span.start as usize..d.span.end as usize)
        .ok_or(LearnError::Unknown)?;
    let base = d.span.start;
    let rel = |x: u32| (x - base) as usize;
    let pairs = crate::directives::parse::pairs_of(&d.body);
    let slot = usize::from(r.cc_slot);
    let mut new = text.to_string();
    // Edits from the back so earlier offsets stay valid.
    let dir_ch = ch_of(pairs).or(file_ch);
    let others_mapped = table
        .hits
        .iter()
        .any(|h| h.directive == r.directive && h.ident != *target && h.cc.is_some());
    let want_ch = ch.filter(|c| Some(*c) != dir_ch);
    if want_ch.is_some() && others_mapped {
        // The directive's channel is shared: add an override line after it.
        let head = match (&d.body, target) {
            (DirectiveBody::Addressed { .. }, BindingIdent::Key(k)) => override_head(table, k),
            _ => positional_head(table, span, param_of(target)).map(|(_, h)| h),
        }
        .ok_or(LearnError::NotDirectiveBacked)?;
        let own_slot = if is_def_param(table, span) { 0 } else { index };
        let indent = own_line_indent(doc_text, table, d.span, placed.trailing);
        new.push('\n');
        new.push_str(&indent);
        new.push_str(&format!(
            "#@ {head}{}",
            render_pairs(&cc_list_at(own_slot, cc), want_ch)
        ));
        return Ok(TextEdit {
            span: (d.span.start, d.span.end),
            expected: text.to_string(),
            text: new,
        });
    }
    // The channel: replace or append `ch:` on this directive.
    let cc_pair = cc_of(pairs);
    let ch_pair = pairs.iter().rev().find(|p| matches!(p.pair, Pair::Ch(_)));
    let mut ops: Vec<(usize, usize, String)> = Vec::new();
    if let Some(c) = want_ch {
        match ch_pair.and_then(|p| p.value_spans.first()) {
            Some(v) => ops.push((rel(v.start), rel(v.end), c.to_string())),
            None => {
                let end = pairs
                    .iter()
                    .map(|p| p.span().end)
                    .max()
                    .unwrap_or(d.head_end)
                    .max(d.head_end);
                ops.push((rel(end), rel(end), format!(" ch: {c}")));
            }
        }
    }
    match cc_pair {
        Some(p) => match p.value_spans.get(slot) {
            Some(v) => ops.push((rel(v.start), rel(v.end), cc.to_string())),
            None => {
                let end = p.span().end;
                let have = p.value_spans.len();
                let mut add = " _".repeat(slot - have);
                add.push_str(&format!(" {cc}"));
                ops.push((rel(end), rel(end), add));
            }
        },
        None => {
            let at = rel(d.head_end);
            let list = render_pairs(&cc_list_at(slot, cc), None);
            ops.push((at, at, list));
        }
    }
    ops.sort_by(|a, b| b.0.cmp(&a.0));
    for (from, to, with) in ops {
        new.replace_range(from..to, &with);
    }
    Ok(TextEdit {
        span: (d.span.start, d.span.end),
        expected: text.to_string(),
        text: new,
    })
}

/// True when `span` is a declared definition parameter (selected alone, so
/// its `cc:` list has one slot).
#[must_use]
pub fn is_def_param(table: &DirectiveTable, span: Span) -> bool {
    table
        .doc
        .targets
        .iter()
        .filter_map(|t| t.def.as_ref())
        .any(|d| d.params.iter().any(|(_, s)| *s == span))
}

fn param_of(ident: &BindingIdent) -> &str {
    match ident {
        BindingIdent::Key(k) => &k.param,
        BindingIdent::Positional { param, .. } => param,
    }
}

/// The addressed head of a key (`hats.lpf`, `hats.lpf.2`, `analog.cutoff`).
#[must_use]
pub fn override_head(
    table: &DirectiveTable,
    key: &crate::directives::key::BindingKey,
) -> Option<String> {
    match &key.site {
        None => Some(format!("{}.{}", key.label, key.param)),
        Some((site, n)) => {
            let crate::directives::labels::LabelLookup::Target(t) =
                table.labels.resolve(&key.label)
            else {
                return None;
            };
            let count = table
                .doc
                .sites_in(t.span)
                .filter(|(_, s)| s.name == *site)
                .count();
            Some(if count > 1 {
                format!("{}.{site}.{n}", key.label)
            } else {
                format!("{}.{site}", key.label)
            })
        }
    }
}

/// The indentation for an own-line directive following directive `span`.
fn own_line_indent(src: &str, table: &DirectiveTable, span: Span, trailing: bool) -> String {
    let line = table.doc.lines.line_of(span.start);
    let start = table.doc.lines.start(line).unwrap_or(0) as usize;
    let width = if trailing {
        table.doc.lines.indent(line)
    } else {
        span.start - start as u32
    };
    src.get(start..start + width as usize)
        .unwrap_or("")
        .to_string()
}
