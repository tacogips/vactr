//! The `#@` directive machinery (design 13.5, 14.5.8).
//!
//! A pure, wasm-safe library over reader output: `build_table` parses every
//! directive trivia item, attaches it, registers labels, resolves selectors
//! and merges the result into one `DirectiveTable`. The language never reads
//! a directive: nothing here changes reading, checking or evaluation, and
//! every diagnostic is a warning.

pub mod attach;
pub mod key;
pub mod labels;
pub mod parse;
pub mod persist;
pub mod resolve;
pub mod writeback;

use std::rc::Rc;

use serde::Serialize;

use crate::directives::attach::{attach, Doc, Placed};
use crate::directives::key::{BindingIdent, BindingKey};
use crate::directives::labels::{LabelKind, LabelLookup, LabelRegistry};
use crate::directives::resolve::{resolve, Hit};
use crate::reader::node::{Node, Trivia};
use crate::reader::span::{FileId, Span};
use crate::types::diag::Diagnostic;
use crate::types::HostManifest;
use crate::value::{intern_kw, KwId};

/// File-wide settings (`#@ midi ch: n`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FileDefaults {
    pub midi_ch: Option<u8>,
}

/// One parameter the directives put on the panel, merged over every
/// directive that names it (a later `cc:` mapping wins).
#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedBinding {
    /// The labeled identity; `None` is positional provenance.
    pub key: Option<BindingKey>,
    pub ident: BindingIdent,
    /// The call-site head or the definition parameter.
    pub target_span: Span,
    pub param: KwId,
    pub param_name: Rc<str>,
    pub param_index: u16,
    pub cc: Option<u8>,
    pub ch: Option<u8>,
    /// The directive that governs this binding (the last one that mapped a
    /// CC, else the last one that named it).
    pub directive_span: Span,
    /// That directive's index in `entries`.
    pub directive: usize,
    /// The position in that directive's `cc:` list.
    pub cc_slot: u16,
}

/// Every directive of one document, resolved.
#[derive(Clone, Debug, Default)]
pub struct DirectiveTable {
    pub file_level: FileDefaults,
    pub entries: Vec<Placed>,
    pub labels: LabelRegistry,
    pub resolved: Vec<ResolvedBinding>,
    /// The unmerged per-directive hits, in directive order.
    pub hits: Vec<Hit>,
    pub doc: Doc,
}

/// Builds the directive table of one read document.
#[must_use]
pub fn build_table(
    src: &str,
    file: FileId,
    nodes: &[Node],
    trivia: &Trivia,
    manifest: &HostManifest,
) -> (DirectiveTable, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let doc = Doc::new(src, file, nodes, manifest);
    let (entries, midi_ch) = attach(src, &doc, trivia, &mut diags);
    let labels = LabelRegistry::build(&doc, &entries, &mut diags);
    let hits = resolve(&doc, &entries, &labels, midi_ch, &mut diags);
    let resolved = merge(&entries, &hits);
    let table = DirectiveTable {
        file_level: FileDefaults { midi_ch },
        entries,
        labels,
        resolved,
        hits,
        doc,
    };
    (table, diags)
}

/// Merges hits per identity, in order of first appearance.
fn merge(entries: &[Placed], hits: &[Hit]) -> Vec<ResolvedBinding> {
    let mut out: Vec<ResolvedBinding> = Vec::new();
    for h in hits {
        let span = entries[h.directive].directive.span;
        let fresh = ResolvedBinding {
            key: h.key.clone(),
            ident: h.ident.clone(),
            target_span: h.target_span,
            param: intern_kw(&h.param),
            param_name: Rc::clone(&h.param),
            param_index: h.param_index,
            cc: h.cc,
            ch: h.ch,
            directive_span: span,
            directive: h.directive,
            cc_slot: h.cc_slot,
        };
        match out.iter_mut().find(|r| r.ident == h.ident) {
            None => out.push(fresh),
            Some(r) if h.cc.is_some() || r.cc.is_none() => *r = fresh,
            Some(_) => {}
        }
    }
    out
}

/// The protocol form of a table (`eval-result`, 14.4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct TableSummary {
    pub midi_ch: Option<u8>,
    pub labels: Vec<LabelSummary>,
    pub bindings: Vec<BindingSummary>,
}

/// One label of the summary.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct LabelSummary {
    pub name: String,
    /// Where each declaration spells it.
    pub spans: Vec<[u32; 2]>,
    pub ambiguous: bool,
}

/// One binding of the summary.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct BindingSummary {
    /// `label.site.n.param` or `label.param`; absent for positional.
    pub key: Option<String>,
    pub span: [u32; 2],
    pub param: String,
    pub param_index: u16,
    pub cc: Option<u8>,
    pub ch: Option<u8>,
    pub directive: [u32; 2],
}

impl DirectiveTable {
    /// The serializable summary.
    #[must_use]
    pub fn summary(&self) -> TableSummary {
        TableSummary {
            midi_ch: self.file_level.midi_ch,
            labels: self
                .labels
                .iter()
                .map(|(name, decls)| LabelSummary {
                    name: name.to_string(),
                    spans: decls.iter().map(|d| [d.span.start, d.span.end]).collect(),
                    ambiguous: decls.len() > 1,
                })
                .collect(),
            bindings: self
                .resolved
                .iter()
                .map(|r| BindingSummary {
                    key: r.key.as_ref().map(ToString::to_string),
                    span: [r.target_span.start, r.target_span.end],
                    param: r.param_name.to_string(),
                    param_index: r.param_index,
                    cc: r.cc,
                    ch: r.ch,
                    directive: [r.directive_span.start, r.directive_span.end],
                })
                .collect(),
        }
    }

    /// The merged binding of `ident`.
    #[must_use]
    pub fn binding(&self, ident: &BindingIdent) -> Option<&ResolvedBinding> {
        self.resolved.iter().find(|r| &r.ident == ident)
    }

    /// The span of parameter `param` of the definition labeled `label`.
    #[must_use]
    pub fn definition_param(&self, label: &str, param: &str) -> Option<Span> {
        let LabelLookup::Target(t) = self.labels.resolve(label) else {
            return None;
        };
        let def = self.doc.targets[t.target].def.as_ref()?;
        def.params
            .iter()
            .find(|(p, _)| &**p == param)
            .map(|(_, span)| *span)
    }

    /// The 1-based ordinal of the `site` call whose head is exactly `head`,
    /// among same-named call sites under `label`.
    #[must_use]
    pub fn site_ordinal(&self, label: &str, site: &str, head: (u32, u32)) -> Option<u16> {
        let LabelLookup::Target(t) = self.labels.resolve(label) else {
            return None;
        };
        let k = self
            .doc
            .sites_in(t.span)
            .filter(|(_, s)| &*s.name == site)
            .position(|(_, s)| (s.head.start, s.head.end) == head)?;
        u16::try_from(k + 1).ok()
    }

    /// The declared parameters a key's site or definition has, with the
    /// span that identifies it (call-site head or parameter).
    #[must_use]
    pub fn key_target(&self, key: &BindingKey) -> Option<(Vec<Rc<str>>, Span)> {
        let LabelLookup::Target(t) = self.labels.resolve(&key.label) else {
            return None;
        };
        match &key.site {
            None => {
                let LabelKind::Definition { params } = &t.kind else {
                    return None;
                };
                let span = self.definition_param(&key.label, &key.param)?;
                Some((params.clone(), span))
            }
            Some((site, n)) => {
                let (_, s) = self
                    .doc
                    .sites_in(t.span)
                    .filter(|(_, s)| s.name == *site)
                    .nth(usize::from(*n).checked_sub(1)?)?;
                Some((s.params.clone(), s.head))
            }
        }
    }

    /// The text of directive `k`.
    #[must_use]
    pub fn directive_text<'a>(&self, src: &'a str, k: usize) -> Option<&'a str> {
        let span = self.entries.get(k)?.directive.span;
        src.get(span.start as usize..span.end as usize)
    }
}

#[cfg(test)]
mod tests;
