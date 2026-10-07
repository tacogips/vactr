//! The Decided attachment rule (design 13.5, 14.5.8) and the document
//! model it runs over.
//!
//! A TARGET is a statement that begins a line: every top-level form, and
//! every statement of an indented block body. A trailing `#@` binds to the
//! innermost target covering its line. Consecutive own-line `#@` lines at
//! one indentation form one block, which binds to the nearest PRECEDING
//! target whose indentation is no deeper than the block's (a line at another
//! indentation starts a new block). Addressed directives are position-free.

use std::rc::Rc;

use crate::directives::parse::{parse_directive, Directive, DirectiveBody, DirectiveKind};
use crate::reader::node::{Atom, Node, NodeKind, Op, Trivia, TriviaKind};
use crate::reader::span::{FileId, Span};
use crate::types::diag::{DiagCode, Diagnostic};
use crate::types::HostManifest;

/// Line starts of one document, for line and indentation queries.
#[derive(Clone, Debug, Default)]
pub struct Lines {
    starts: Vec<u32>,
    indents: Vec<u32>,
    firsts: Vec<u32>,
}

impl Lines {
    /// Indexes `src`: line starts, indentation widths and first non-blank
    /// offsets.
    #[must_use]
    pub fn new(src: &str) -> Lines {
        let mut lines = Lines::default();
        let mut start = 0usize;
        for line in src.split('\n') {
            let ws = line
                .bytes()
                .take_while(|b| *b == b'\t' || *b == b' ')
                .count();
            let at = |k: usize| u32::try_from(k).unwrap_or(u32::MAX);
            lines.starts.push(at(start));
            lines.indents.push(at(ws));
            lines.firsts.push(at(start + ws));
            start += line.len() + 1;
        }
        lines
    }

    /// The 0-based line of byte `at`.
    #[must_use]
    pub fn line_of(&self, at: u32) -> usize {
        self.starts.partition_point(|s| *s <= at).saturating_sub(1)
    }

    /// The indentation width of line `line`.
    #[must_use]
    pub fn indent(&self, line: usize) -> u32 {
        self.indents.get(line).copied().unwrap_or(0)
    }

    /// The first non-blank byte of line `line`.
    #[must_use]
    pub fn first(&self, line: usize) -> u32 {
        self.firsts.get(line).copied().unwrap_or(0)
    }

    /// The start of line `line`.
    #[must_use]
    pub fn start(&self, line: usize) -> Option<u32> {
        self.starts.get(line).copied()
    }

    /// The number of lines.
    #[must_use]
    pub fn count(&self) -> usize {
        self.starts.len()
    }
}

/// A definition head with declared parameters (`inst`, `fn`, `bus`, `look`).
#[derive(Clone, PartialEq, Debug)]
pub struct DefInfo {
    pub head: &'static str,
    pub name: Rc<str>,
    pub name_span: Span,
    /// Declared parameters in header order, with their spans.
    pub params: Vec<(Rc<str>, Span)>,
}

/// A statement directives can attach to.
#[derive(Clone, PartialEq, Debug)]
pub struct Target {
    /// From the first to the last byte of the statement's subtree.
    pub extent: Span,
    pub indent: u32,
    pub first_line: usize,
    pub last_line: usize,
    /// The top-level form this target belongs to.
    pub top: usize,
    pub def: Option<DefInfo>,
    /// The implicit label of a top-level `let` or a named slot.
    pub implicit: Option<(Rc<str>, Span)>,
}

/// One call site: a symbol-headed call.
#[derive(Clone, PartialEq, Debug)]
pub struct CallSite {
    pub name: Rc<str>,
    /// The head symbol's span: the site's identity for key migration.
    pub head: Span,
    /// Declared parameters, in order (builtin `EditorDecl` or a document
    /// definition's header).
    pub params: Vec<Rc<str>>,
    /// Every argument, in order: the named-argument keyword (a `Pair` whose
    /// key is a `Keyword`), else `None`, and the argument's full extent
    /// (design 15.1.2 G3).
    pub args: Vec<(Option<Rc<str>>, Span)>,
}

/// The document model: targets and call sites in source order.
#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub file: Option<FileId>,
    pub lines: Lines,
    pub targets: Vec<Target>,
    pub sites: Vec<CallSite>,
    top_first: Vec<usize>,
}

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static TOP_OF_STEPS: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn top_of_steps() -> u64 {
    TOP_OF_STEPS.with(Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_top_of_steps() {
    TOP_OF_STEPS.with(|steps| steps.set(0));
}

#[cfg(test)]
#[inline]
fn note_top_of_step() {
    TOP_OF_STEPS.with(|steps| steps.set(steps.get() + 1));
}

#[cfg(not(test))]
#[inline(always)]
fn note_top_of_step() {}

/// Heads that are syntax, never call sites.
const NOT_SITES: [&str; 13] = [
    "let", "var", "upd", "fn", "inst", "bus", "look", "master", "struct", "enum", "import", "slot",
    "if",
];

/// The smallest span covering `n` and every descendant.
#[must_use]
pub fn extent(n: &Node) -> Span {
    let mut s = n.span;
    n.walk(&mut |c: &Node| s = s.join(c.span));
    s
}

/// The named-argument keyword of a call argument: `n` of a `key: value`
/// `Pair` (design 15.1.2 G3), else `None` for a positional argument.
fn arg_keyword(item: &Node) -> Option<Rc<str>> {
    if item.kind != NodeKind::Pair {
        return None;
    }
    match &item.children.first()?.kind {
        NodeKind::Atom(Atom::Keyword(k)) => Some(Rc::clone(k)),
        _ => None,
    }
}

fn def_info(n: &Node) -> Option<DefInfo> {
    let ch = &n.children;
    if !matches!(n.kind, NodeKind::Call) {
        return None;
    }
    let head = match ch.first()?.sym_name()? {
        "inst" => "inst",
        "fn" => "fn",
        "bus" => "bus",
        "look" => "look",
        _ => return None,
    };
    let (name, name_span) = match &ch.get(1)?.kind {
        NodeKind::Atom(Atom::Sym(s) | Atom::Keyword(s)) => (Rc::clone(s), ch[1].span),
        NodeKind::Pair => match &ch[1].children.first()?.kind {
            NodeKind::Atom(Atom::Keyword(k)) => (Rc::clone(k), ch[1].children[0].span),
            _ => return None,
        },
        _ => return None,
    };
    let mut params = Vec::new();
    let mut after_eq = false;
    for item in ch.iter().skip(2) {
        if after_eq {
            after_eq = false;
            continue;
        }
        match &item.kind {
            NodeKind::Atom(Atom::Op(Op::Eq)) => after_eq = true,
            NodeKind::Atom(Atom::Sym(s)) if head != "bus" => params.push((Rc::clone(s), item.span)),
            NodeKind::Pair => {
                if let Some(NodeKind::Atom(Atom::Keyword(k))) =
                    item.children.first().map(|c| &c.kind)
                {
                    params.push((Rc::clone(k), item.children[0].span));
                }
            }
            _ => {}
        }
    }
    Some(DefInfo {
        head,
        name,
        name_span,
        params,
    })
}

/// A top-level `let a ...` name, or the first `slot :name` inside `n`.
fn implicit_of(n: &Node) -> Option<(Rc<str>, Span)> {
    if let [head, name, ..] = &*n.children {
        if head.sym_name() == Some("let") {
            return name.sym_name().map(|s| (Rc::from(s), name.span));
        }
    }
    let mut found = None;
    n.walk(&mut |c: &Node| {
        if found.is_some() || !matches!(c.kind, NodeKind::Call) {
            return;
        }
        if c.children.first().and_then(Node::sym_name) == Some("slot") {
            found = c.children.iter().skip(1).find_map(|a| match &a.kind {
                NodeKind::Atom(Atom::Keyword(k)) => Some((Rc::clone(k), a.span)),
                _ => None,
            });
        }
    });
    found
}

impl Doc {
    /// Builds the model of one read document.
    #[must_use]
    pub fn new(src: &str, file: FileId, nodes: &[Node], manifest: &HostManifest) -> Doc {
        let mut doc = Doc {
            file: Some(file),
            lines: Lines::new(src),
            ..Doc::default()
        };
        for (top, n) in nodes.iter().enumerate() {
            let def = def_info(n);
            let implicit = if def.is_none() { implicit_of(n) } else { None };
            doc.push_target(n, top, def, implicit);
            doc.body_targets(n, top);
        }
        let defs: Vec<DefInfo> = doc.targets.iter().filter_map(|t| t.def.clone()).collect();
        for n in nodes {
            n.walk(&mut |c: &Node| {
                if !matches!(c.kind, NodeKind::Call) {
                    return;
                }
                let Some(head) = c.children.first() else {
                    return;
                };
                let Some(name) = head.sym_name() else {
                    return;
                };
                if NOT_SITES.contains(&name) {
                    return;
                }
                let params = match manifest.editor_decl(name) {
                    Some(decl) => decl.params.iter().map(|p| Rc::from(p.name)).collect(),
                    None => defs
                        .iter()
                        .find(|d| &*d.name == name && d.head != "bus")
                        .map(|d| d.params.iter().map(|(p, _)| Rc::clone(p)).collect())
                        .unwrap_or_default(),
                };
                // A piped call (`x > f a b`) reads as `f x a b`: the
                // reader inserts the piped value right after the head
                // (`reader/line.rs` `fold_operand`), so it is always the
                // FIRST child and always textually BEFORE the head (the
                // typed arguments always follow the head). Filtering by
                // position excludes it, so `args` lines up with the
                // declared `params` the same way for a piped and a plain
                // call.
                let args = c
                    .children
                    .iter()
                    .skip(1)
                    .map(|item| (item, extent(item)))
                    .filter(|(_, ext)| ext.start >= head.span.end)
                    .map(|(item, ext)| (arg_keyword(item), ext))
                    .collect();
                doc.sites.push(CallSite {
                    name: Rc::from(name),
                    head: head.span,
                    params,
                    args,
                });
            });
        }
        doc.sites.sort_by_key(|s| s.head.start);
        doc.targets.sort_by_key(|t| (t.extent.start, t.first_line));
        let top_count = nodes.len();
        doc.top_first = vec![usize::MAX; top_count];
        for (target_index, target) in doc.targets.iter().enumerate() {
            let first = &mut doc.top_first[target.top];
            if *first == usize::MAX {
                *first = target_index;
            }
        }
        doc
    }

    fn push_target(
        &mut self,
        n: &Node,
        top: usize,
        def: Option<DefInfo>,
        implicit: Option<(Rc<str>, Span)>,
    ) {
        if matches!(n.kind, NodeKind::Error) {
            return;
        }
        let ext = extent(n);
        let first_line = self.lines.line_of(ext.start);
        self.targets.push(Target {
            extent: ext,
            indent: self.lines.indent(first_line),
            first_line,
            last_line: self.lines.line_of(ext.end.saturating_sub(1).max(ext.start)),
            top,
            def,
            implicit,
        });
    }

    /// Every block statement under `n` that begins its own line.
    fn body_targets(&mut self, n: &Node, top: usize) {
        for c in n.children.iter() {
            if matches!(c.kind, NodeKind::Block) {
                for stmt in c.children.iter() {
                    let ext = extent(stmt);
                    let line = self.lines.line_of(ext.start);
                    if self.lines.first(line) == ext.start {
                        self.push_target(stmt, top, None, None);
                    }
                }
            }
            self.body_targets(c, top);
        }
    }

    /// The call sites whose head lies inside `span`, in source order.
    pub fn sites_in(&self, span: Span) -> impl Iterator<Item = (usize, &CallSite)> {
        self.sites
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.head.start >= span.start && s.head.end <= span.end)
    }

    /// The innermost target covering line `line`.
    #[must_use]
    pub fn target_on_line(&self, line: usize) -> Option<usize> {
        self.targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.first_line <= line && line <= t.last_line)
            .max_by_key(|(_, t)| (t.first_line, t.extent.start))
            .map(|(k, _)| k)
    }

    /// The nearest target starting before `at` whose indentation is no
    /// deeper than `indent`.
    #[must_use]
    pub fn preceding(&self, at: u32, indent: u32) -> Option<usize> {
        self.targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.extent.start < at && t.indent <= indent)
            .max_by_key(|(_, t)| t.extent.start)
            .map(|(k, _)| k)
    }

    /// The top-level target of `t` (itself when it is top-level).
    #[must_use]
    pub fn top_of(&self, t: usize) -> usize {
        let top = self.targets[t].top;
        if let Some(first) = self.top_first.get(top).copied() {
            if first != usize::MAX {
                note_top_of_step();
                return first;
            }
        }
        let mut steps = 0;
        let first = self.targets.iter().position(|target| {
            steps += 1;
            target.top == top
        });
        for _ in 0..steps {
            note_top_of_step();
        }
        first.unwrap_or(t)
    }
}

/// A parsed directive with its placement.
#[derive(Clone, PartialEq, Debug)]
pub struct Placed {
    pub directive: Directive,
    /// The attach target (`None` for Addressed directives and unattached
    /// positional ones).
    pub attach: Option<usize>,
    /// True when the directive trails code on its line.
    pub trailing: bool,
}

/// Parses every directive trivia item and attaches it. Returns the placed
/// directives in source order and the file-level `#@ midi ch:` default.
#[must_use]
pub fn attach(
    src: &str,
    doc: &Doc,
    trivia: &Trivia,
    diags: &mut Vec<Diagnostic>,
) -> (Vec<Placed>, Option<u8>) {
    let mut placed: Vec<Placed> = Vec::new();
    let mut midi_ch: Option<u8> = None;
    // The current own-line block: its last line, indentation and target.
    // A line at another indentation starts a new block: each block binds
    // no deeper than its own comment's indentation.
    let mut block: Option<(usize, u32, Option<usize>)> = None;
    for item in trivia
        .items
        .iter()
        .filter(|t| t.kind == TriviaKind::Directive)
    {
        let text = src
            .get(item.span.start as usize..item.span.end as usize)
            .unwrap_or("");
        let d = parse_directive(text, item.span, diags);
        let line = doc.lines.line_of(item.span.start);
        let trailing = doc.lines.first(line) < item.span.start;
        let attach = if trailing {
            block = None;
            doc.target_on_line(line)
        } else {
            let indent = doc.lines.indent(line);
            match block {
                Some((last, at, target)) if last + 1 == line && at == indent => {
                    block = Some((line, at, target));
                    target
                }
                _ => {
                    let target = doc.preceding(item.span.start, indent);
                    block = Some((line, indent, target));
                    target
                }
            }
        };
        if let DirectiveBody::FileDefault { pairs } = &d.body {
            if let Some(ch) = crate::directives::parse::ch_of(pairs) {
                if midi_ch.is_some() {
                    diags.push(Diagnostic {
                        span: d.span,
                        severity: DiagCode::DuplicateKey.default_severity(),
                        code: DiagCode::DuplicateKey,
                        message: "a later `#@ midi ch:` replaces the file default".into(),
                        origin: None,
                    });
                }
                midi_ch = Some(ch);
            }
        }
        let attach = match d.kind {
            DirectiveKind::Addressed => None,
            DirectiveKind::Positional => attach,
        };
        let needs_target =
            d.kind == DirectiveKind::Positional && !matches!(d.body, DirectiveBody::Empty);
        if needs_target && attach.is_none() {
            diags.push(Diagnostic {
                span: d.span,
                severity: DiagCode::UnknownDirectiveSite.default_severity(),
                code: DiagCode::UnknownDirectiveSite,
                message: "no preceding statement at this indentation to attach to".into(),
                origin: None,
            });
        }
        placed.push(Placed {
            directive: d,
            attach,
            trailing,
        });
    }
    (placed, midi_ch)
}
