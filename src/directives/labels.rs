//! The per-document label namespace (design 13.5 LABEL RESOLUTION AND
//! UNIQUENESS, 14.5.8).
//!
//! Explicit labels (the trailing `label:` short form, `#@ name X`, and the
//! block-following form for bodies) and implicit labels (every top-level
//! `let`/`fn`/`inst`/`bus`/`look` name and every named slot) share ONE
//! namespace with exact match. Any collision involving an explicit label is
//! `duplicate-label`, and the name is ambiguous: references to it are
//! rejected, never guessed.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::directives::attach::{Doc, Placed};
use crate::directives::parse::DirectiveBody;
use crate::reader::span::Span;
use crate::types::diag::{DiagCode, Diagnostic};

/// What a label names.
#[derive(Clone, PartialEq, Debug)]
pub enum LabelKind {
    /// A statement: its call sites are selectable by name.
    Line,
    /// A definition with declared parameters.
    Definition { params: Vec<Rc<str>> },
}

/// A resolved label target.
#[derive(Clone, PartialEq, Debug)]
pub struct LabelTarget {
    /// The target's extent.
    pub span: Span,
    pub kind: LabelKind,
    /// The index of the target in the document model.
    pub target: usize,
}

/// The result of looking a label up.
#[derive(Clone, PartialEq, Debug)]
pub enum LabelLookup {
    Target(LabelTarget),
    Ambiguous,
    Unknown,
}

/// One definition of a label.
#[derive(Clone, PartialEq, Debug)]
pub struct LabelDecl {
    pub target: usize,
    /// Where the label is spelled.
    pub span: Span,
    pub explicit: bool,
}

/// Every label of one document.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct LabelRegistry {
    labels: BTreeMap<Rc<str>, Vec<LabelDecl>>,
    kinds: Vec<LabelKind>,
    extents: Vec<Span>,
}

impl LabelRegistry {
    /// Registers the implicit labels of `doc` and the explicit labels of
    /// `placed`, reporting `duplicate-label` for every collision that
    /// involves an explicit label.
    #[must_use]
    pub fn build(doc: &Doc, placed: &[Placed], diags: &mut Vec<Diagnostic>) -> LabelRegistry {
        let mut reg = LabelRegistry {
            kinds: doc
                .targets
                .iter()
                .map(|t| match &t.def {
                    Some(d) => LabelKind::Definition {
                        params: d.params.iter().map(|(p, _)| Rc::clone(p)).collect(),
                    },
                    None => LabelKind::Line,
                })
                .collect(),
            extents: doc.targets.iter().map(|t| t.extent).collect(),
            ..LabelRegistry::default()
        };
        for (k, t) in doc.targets.iter().enumerate() {
            let implicit = match &t.def {
                Some(d) => Some((Rc::clone(&d.name), d.name_span)),
                None => t.implicit.clone(),
            };
            // Only top-level forms carry implicit labels.
            if doc.top_of(k) != k {
                continue;
            }
            if let Some((name, span)) = implicit {
                reg.add(name, k, span, false, diags);
            }
        }
        for p in placed {
            if let (
                DirectiveBody::LabelDef {
                    name, name_span, ..
                },
                Some(target),
            ) = (&p.directive.body, p.attach)
            {
                reg.add(Rc::clone(name), target, *name_span, true, diags);
            }
        }
        reg
    }

    fn add(
        &mut self,
        name: Rc<str>,
        target: usize,
        span: Span,
        explicit: bool,
        diags: &mut Vec<Diagnostic>,
    ) {
        let decls = self.labels.entry(Rc::clone(&name)).or_default();
        if decls.iter().any(|d| d.target == target) {
            // The same name on the same target twice is one label.
            return;
        }
        if let Some(prev) = decls.first() {
            if explicit || prev.explicit {
                let (what, at) = if explicit && prev.explicit {
                    ("another explicit label", prev.span)
                } else if explicit {
                    ("a named definition", prev.span)
                } else {
                    ("an explicit label", prev.span)
                };
                diags.push(Diagnostic {
                    span,
                    severity: DiagCode::DuplicateLabel.default_severity(),
                    code: DiagCode::DuplicateLabel,
                    message: format!(
                        "label `{name}` is also {what} at {}..{}; references to it are rejected until one side is renamed",
                        at.start, at.end
                    ),
                    origin: None,
                });
            }
        }
        decls.push(LabelDecl {
            target,
            span,
            explicit,
        });
    }

    /// Looks `name` up: exact match, no precedence.
    #[must_use]
    pub fn resolve(&self, name: &str) -> LabelLookup {
        match self.labels.get(name).map(Vec::as_slice) {
            None | Some([]) => LabelLookup::Unknown,
            Some([one]) => LabelLookup::Target(LabelTarget {
                span: self.extents[one.target],
                kind: self.kinds[one.target].clone(),
                target: one.target,
            }),
            Some(_) => LabelLookup::Ambiguous,
        }
    }

    /// True when `name` names more than one thing.
    #[must_use]
    pub fn is_ambiguous(&self, name: &str) -> bool {
        self.labels.get(name).is_some_and(|d| d.len() > 1)
    }

    /// The labels declared on target `t` (explicit first), ambiguous or not.
    #[must_use]
    pub fn labels_of(&self, t: usize) -> Vec<(Rc<str>, bool)> {
        let mut out: Vec<(Rc<str>, bool)> = self
            .labels
            .iter()
            .flat_map(|(name, decls)| {
                decls
                    .iter()
                    .filter(move |d| d.target == t)
                    .map(move |d| (Rc::clone(name), d.explicit))
            })
            .collect();
        out.sort_by_key(|(_, explicit)| !explicit);
        out
    }

    /// Every label with its declarations, in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&Rc<str>, &[LabelDecl])> {
        self.labels.iter().map(|(n, d)| (n, d.as_slice()))
    }
}
