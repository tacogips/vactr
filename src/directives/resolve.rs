//! ADDRESSED SELECTOR RESOLUTION, exactly as design 13.5 states it.
//!
//! Within a target: (1) a definition's declared PARAMETER keyword selects
//! that parameter; (2) otherwise a CALL-SITE name inside the target selects
//! that call site, and `cc:` maps over ITS declared parameter order; (3) a
//! selector matching both is `ambiguous-selector`; (4) a repeated same-named
//! call site used bare is `ambiguous-selector`, and the ordinal `.n` selects
//! the n-th in source order. Positional first tokens resolve against the
//! attach target the same way.

use std::rc::Rc;

use crate::directives::attach::{Doc, Placed};
use crate::directives::key::{BindingIdent, BindingKey};
use crate::directives::labels::{LabelLookup, LabelRegistry};
use crate::directives::parse::{cc_of, ch_of, DirectiveBody, Pair, PairAt, Positional};
use crate::reader::span::Span;
use crate::types::diag::{DiagCode, Diagnostic};

/// One parameter a directive named, before merging.
#[derive(Clone, PartialEq, Debug)]
pub struct Hit {
    /// The index of the directive in the table's `entries`.
    pub directive: usize,
    pub ident: BindingIdent,
    pub key: Option<BindingKey>,
    /// The call-site head or the definition parameter span.
    pub target_span: Span,
    pub param: Rc<str>,
    /// The index in the site's (or definition's) declared order.
    pub param_index: u16,
    /// The position in the directive's `cc:` list this parameter maps from.
    pub cc_slot: u16,
    pub cc: Option<u8>,
    /// The directive's `ch:`, or the file default, when mapped.
    pub ch: Option<u8>,
}

/// What one selector picked.
#[derive(Clone, Debug)]
enum Sel {
    /// A declared parameter of the definition target `target`.
    Param { target: usize, index: usize },
    /// Call site `site` of the document model.
    Site(usize),
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

/// How a selector that matches nothing is reported.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Positional,
    Addressed,
}

/// Resolves `name[.ordinal]` within target `t` (steps 1-4).
fn select(
    doc: &Doc,
    t: usize,
    name: &str,
    ordinal: Option<u16>,
    span: Span,
    mode: Mode,
    diags: &mut Vec<Diagnostic>,
) -> Option<Sel> {
    let target = &doc.targets[t];
    let param = target
        .def
        .as_ref()
        .and_then(|d| d.params.iter().position(|(p, _)| &**p == name));
    let sites: Vec<usize> = doc
        .sites_in(target.extent)
        .filter(|(_, s)| &*s.name == name)
        .map(|(k, _)| k)
        .collect();
    if param.is_some() && !sites.is_empty() {
        diags.push(warn(
            DiagCode::AmbiguousSelector,
            span,
            format!("`{name}` is both a parameter and a call site here; rename one"),
        ));
        return None;
    }
    if let Some(index) = param {
        if ordinal.is_some() {
            diags.push(warn(
                DiagCode::UnknownParameter,
                span,
                format!("parameter `{name}` takes no ordinal"),
            ));
            return None;
        }
        return Some(Sel::Param { target: t, index });
    }
    match (sites.as_slice(), ordinal) {
        ([], _) => {
            let (code, what) = match (mode, &target.def) {
                (Mode::Addressed, Some(_)) => (
                    DiagCode::UnknownParameter,
                    "neither a declared parameter nor a call site",
                ),
                _ => (
                    DiagCode::UnknownDirectiveSite,
                    "not a call site or parameter of the attached statement",
                ),
            };
            diags.push(warn(code, span, format!("`{name}` is {what}")));
            None
        }
        ([one], None) => Some(Sel::Site(*one)),
        (_, None) => {
            diags.push(warn(
                DiagCode::AmbiguousSelector,
                span,
                format!(
                    "`{name}` occurs {} times here; select one with `{name}.n`",
                    sites.len()
                ),
            ));
            None
        }
        (all, Some(n)) => match all.get(usize::from(n) - 1) {
            Some(k) => Some(Sel::Site(*k)),
            None => {
                diags.push(warn(
                    DiagCode::UnknownDirectiveSite,
                    span,
                    format!("there is no `{name}` number {n} here ({} found)", all.len()),
                ));
                None
            }
        },
    }
}

/// The label a binding under target `t` is keyed by, and that label's
/// target: `t`'s own first unambiguous label, else its top-level form's.
/// `None` means positional provenance.
fn key_label(doc: &Doc, labels: &LabelRegistry, t: usize) -> Option<(Rc<str>, usize)> {
    let own = |t: usize| {
        labels
            .labels_of(t)
            .into_iter()
            .find(|(name, _)| !labels.is_ambiguous(name))
            .map(|(name, _)| (name, t))
    };
    let mine = labels.labels_of(t);
    if !mine.is_empty() {
        return own(t);
    }
    let top = doc.top_of(t);
    if top == t {
        return None;
    }
    if labels.labels_of(top).is_empty() {
        return None;
    }
    own(top)
}

/// The 1-based ordinal of site `site` among same-named sites in `target`.
fn ordinal_in(doc: &Doc, target: usize, site: usize) -> Option<u16> {
    let name = &doc.sites[site].name;
    let k = doc
        .sites_in(doc.targets[target].extent)
        .filter(|(_, s)| &s.name == name)
        .position(|(k, _)| k == site)?;
    u16::try_from(k + 1).ok()
}

/// One selected parameter, before cc mapping.
struct Slot {
    ident: BindingIdent,
    key: Option<BindingKey>,
    target_span: Span,
    param: Rc<str>,
    param_index: u16,
}

/// Expands selections into parameter slots in written order.
fn slots(doc: &Doc, labels: &LabelRegistry, sels: &[Sel]) -> Vec<Slot> {
    let mut out = Vec::new();
    for sel in sels {
        match sel {
            Sel::Param { target, index } => {
                let Some(def) = &doc.targets[*target].def else {
                    continue;
                };
                let (param, span) = &def.params[*index];
                let key = key_label(doc, labels, *target)
                    .map(|(label, _)| BindingKey::param(&label, param));
                out.push(Slot {
                    ident: key
                        .clone()
                        .map_or_else(|| BindingIdent::positional(*span, param), BindingIdent::Key),
                    key,
                    target_span: *span,
                    param: Rc::clone(param),
                    param_index: u16::try_from(*index).unwrap_or(u16::MAX),
                });
            }
            Sel::Site(k) => {
                let site = &doc.sites[*k];
                let owner = doc
                    .targets
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        t.extent.start <= site.head.start && site.head.end <= t.extent.end
                    })
                    .max_by_key(|(_, t)| t.extent.start)
                    .map(|(k, _)| k);
                let label = owner.and_then(|t| key_label(doc, labels, t));
                for (i, param) in site.params.iter().enumerate() {
                    let key = label.as_ref().and_then(|(label, lt)| {
                        let n = ordinal_in(doc, *lt, *k)?;
                        Some(BindingKey::site(label, &site.name, n, param))
                    });
                    out.push(Slot {
                        ident: key.clone().map_or_else(
                            || BindingIdent::positional(site.head, param),
                            BindingIdent::Key,
                        ),
                        key,
                        target_span: site.head,
                        param: Rc::clone(param),
                        param_index: u16::try_from(i).unwrap_or(u16::MAX),
                    });
                }
            }
        }
    }
    out
}

/// Maps `cc:`/`ch:` over the slots of directive `directive`.
fn map_pairs(
    directive: usize,
    slots: Vec<Slot>,
    pairs: &[PairAt],
    file_ch: Option<u8>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Hit> {
    let (ccs, spans): (&[Option<u8>], &[Span]) = match cc_of(pairs) {
        Some(PairAt {
            pair: Pair::Cc(ccs),
            value_spans,
            ..
        }) => (ccs, value_spans),
        _ => (&[], &[]),
    };
    for (k, span) in spans.iter().enumerate().skip(slots.len()) {
        if ccs.get(k).copied().flatten().is_some() {
            diags.push(warn(
                DiagCode::UnknownParameter,
                *span,
                format!("CC number {} has no parameter to map onto", k + 1),
            ));
        }
    }
    let ch = ch_of(pairs).or(file_ch);
    slots
        .into_iter()
        .enumerate()
        .map(|(k, s)| {
            let cc = ccs.get(k).copied().flatten();
            Hit {
                directive,
                ident: s.ident,
                key: s.key,
                target_span: s.target_span,
                param: s.param,
                param_index: s.param_index,
                cc_slot: u16::try_from(k).unwrap_or(u16::MAX),
                cc,
                ch: cc.and(ch),
            }
        })
        .collect()
}

/// Resolves every placed directive into parameter hits.
#[must_use]
pub fn resolve(
    doc: &Doc,
    placed: &[Placed],
    labels: &LabelRegistry,
    file_ch: Option<u8>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (k, p) in placed.iter().enumerate() {
        let d = &p.directive;
        match &d.body {
            DirectiveBody::FileDefault { .. } | DirectiveBody::Empty => {}
            DirectiveBody::LabelDef { rest: None, .. } => {}
            DirectiveBody::LabelDef {
                rest: Some(pos), ..
            }
            | DirectiveBody::Positional(pos) => {
                let Some(t) = p.attach else {
                    continue;
                };
                hits.extend(positional(doc, labels, k, t, pos, file_ch, diags));
            }
            DirectiveBody::Addressed {
                label,
                label_span,
                sel,
                sel_span,
                ordinal,
                pairs,
            } => {
                let t = match labels.resolve(label) {
                    LabelLookup::Target(t) => t.target,
                    LabelLookup::Ambiguous => {
                        diags.push(warn(
                            DiagCode::DuplicateLabel,
                            *label_span,
                            format!("label `{label}` is ambiguous; this reference is rejected"),
                        ));
                        continue;
                    }
                    LabelLookup::Unknown => {
                        diags.push(warn(
                            DiagCode::UnknownLabel,
                            *label_span,
                            format!("no label `{label}` in this document"),
                        ));
                        continue;
                    }
                };
                let span = Span::new(
                    sel_span.file,
                    label_span.start,
                    d.head_end.max(sel_span.end),
                );
                let Some(sel) = select(doc, t, sel, *ordinal, span, Mode::Addressed, diags) else {
                    continue;
                };
                let slots = slots(doc, labels, &[sel]);
                hits.extend(map_pairs(k, slots, pairs, file_ch, diags));
            }
        }
    }
    hits
}

fn positional(
    doc: &Doc,
    labels: &LabelRegistry,
    k: usize,
    t: usize,
    pos: &Positional,
    file_ch: Option<u8>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Hit> {
    let mut sels = Vec::new();
    for s in &pos.sites {
        if let Some(sel) = select(doc, t, &s.name, s.ordinal, s.span, Mode::Positional, diags) {
            sels.push(sel);
        }
    }
    if sels.len() != pos.sites.len() {
        // A partly resolved directive maps nothing: its cc positions would
        // shift onto the wrong parameters.
        return Vec::new();
    }
    let slots = slots(doc, labels, &sels);
    map_pairs(k, slots, &pos.pairs, file_ch, diags)
}
