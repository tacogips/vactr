//! Rewrites (design 6.5.5 "Desugarings"). Every synthesized node takes a
//! fresh id and the span of the sugar node it replaces.

use std::rc::Rc;

use crate::expand::expander::{diag, else_without_if, expr, guard, pattern, Slot};
use crate::expand::kernel::{self, clause_lhs};
use crate::expand::ExpandCx;
use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::reader::span::Span;
use crate::types::diag::{DiagCode, Diagnostic};

/// A synthesized node.
fn syn(cx: &mut ExpandCx, kind: NodeKind, span: Span, children: Vec<Node>) -> Node {
    Node {
        id: cx.fresh(),
        kind,
        span,
        children: children.into_boxed_slice(),
    }
}

fn syn_atom(cx: &mut ExpandCx, atom: Atom, span: Span) -> Node {
    syn(cx, NodeKind::Atom(atom), span, Vec::new())
}

/// `(HEAD args..)` with a `Builtin` head that resolves only in the prelude.
fn builtin_call(cx: &mut ExpandCx, name: &str, span: Span, args: Vec<Node>) -> Node {
    let mut children = Vec::with_capacity(args.len() + 1);
    children.push(syn_atom(cx, Atom::Builtin(Rc::from(name)), span));
    children.extend(args);
    syn(cx, NodeKind::Call, span, children)
}

/// `(-> (lhs..) body)`.
fn clause(cx: &mut ExpandCx, span: Span, mut lhs: Vec<Node>, body: Node) -> Node {
    lhs.push(body);
    syn(cx, NodeKind::Arrow, span, lhs)
}

/// The falsy pattern `false | nil`.
fn falsy(cx: &mut ExpandCx, span: Span) -> Vec<Node> {
    vec![
        syn_atom(cx, Atom::Bool(false), span),
        syn_atom(cx, Atom::Op(Op::Bar), span),
        syn_atom(cx, Atom::Nil, span),
    ]
}

/// `(match subject {clauses..})`.
fn match_of(cx: &mut ExpandCx, span: Span, subject: Node, clauses: Vec<Node>) -> Node {
    let head = syn_atom(cx, Atom::Sym(Rc::from("match")), span);
    let block = syn(cx, NodeKind::Block, span, clauses);
    syn(cx, NodeKind::Call, span, vec![head, subject, block])
}

/// `(match C {(-> (false | nil) E) (-> (_) T)})`.
fn truthy_match(cx: &mut ExpandCx, span: Span, cond: Node, then: Node, els: Node) -> Node {
    let fl = falsy(cx, span);
    let no = clause(cx, span, fl, els);
    let wild = syn_atom(cx, Atom::Wildcard, span);
    let yes = clause(cx, span, vec![wild], then);
    match_of(cx, span, cond, vec![no, yes])
}

fn malformed_if(n: &Node) -> Diagnostic {
    diag(
        DiagCode::MalformedIf,
        n,
        "expected `if COND THEN [ELSE]` or `if SUBJ PATTERN -> BODY`",
    )
}

/// `(if C T E)` or `(if C T)`.
pub(super) fn if_call(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    let ch = &n.children;
    if !(3..=4).contains(&ch.len()) {
        return Err(malformed_if(n));
    }
    let cond = expr(&ch[1], Slot::Other, cx, depth)?;
    let then = expr(&ch[2], Slot::Other, cx, depth)?;
    let els = match ch.get(3) {
        Some(e) => expr(e, Slot::Other, cx, depth)?,
        None => syn_atom(cx, Atom::Nil, n.span),
    };
    Ok(truthy_match(cx, n.span, cond, then, els))
}

/// The expanded parts of one `if` or `elif` link, before its else is known.
enum Link {
    Plain {
        span: Span,
        cond: Node,
        then: Node,
    },
    Binding {
        span: Span,
        subject: Node,
        pats: Vec<Node>,
        then: Node,
    },
}

impl Link {
    fn build(self, cx: &mut ExpandCx, els: Node) -> Node {
        match self {
            Link::Plain { span, cond, then } => truthy_match(cx, span, cond, then, els),
            Link::Binding {
                span,
                subject,
                pats,
                then,
            } => {
                let simple = pats.len() == 1
                    && matches!(pats[0].kind, NodeKind::Atom(Atom::Sym(_) | Atom::Wildcard));
                if simple {
                    let fl = falsy(cx, span);
                    let no = clause(cx, span, fl, els);
                    let yes = clause(cx, span, pats, then);
                    match_of(cx, span, subject, vec![no, yes])
                } else {
                    let yes = clause(cx, span, pats, then);
                    let wild = syn_atom(cx, Atom::Wildcard, span);
                    let no = clause(cx, span, vec![wild], els);
                    match_of(cx, span, subject, vec![yes, no])
                }
            }
        }
    }
}

/// Expands the parts of a binding `if` (or `elif`): an arrow whose lhs is
/// `[if, S, P..]`. A top-level `if` inside `P..` is `if-guard`.
fn binding_link(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Link, Diagnostic> {
    let lhs = clause_lhs(n);
    if lhs.len() < 3 {
        return Err(malformed_if(n));
    }
    let pats = &lhs[2..];
    if let Some(g) = pats.iter().find(|p| p.sym_name() == Some("if")) {
        return Err(diag(
            DiagCode::IfGuard,
            g,
            "guards exist only in `match`; write a `match` clause",
        ));
    }
    let subject = expr(&lhs[1], Slot::Other, cx, depth)?;
    for p in pats {
        pattern(p, Slot::Other, depth)?;
    }
    let body = match n.children.last() {
        Some(b) => b,
        None => return Err(malformed_if(n)),
    };
    let then = expr(body, Slot::Other, cx, depth)?;
    Ok(Link::Binding {
        span: n.span,
        subject,
        pats: pats.to_vec(),
        then,
    })
}

/// A binding `if` outside a chain (its else is `els`, or nil).
pub(super) fn binding_if(
    n: &Node,
    els: Option<Node>,
    cx: &mut ExpandCx,
    depth: u32,
) -> Result<Node, Diagnostic> {
    let link = binding_link(n, cx, depth)?;
    let els = match els {
        Some(e) => e,
        None => syn_atom(cx, Atom::Nil, n.span),
    };
    Ok(link.build(cx, els))
}

/// One `if`/`elif` link of a chain: `(if C T)`, `(elif C T)`, or a binding
/// arrow. A plain link in a chain takes no inline else.
fn chain_link(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Link, Diagnostic> {
    if matches!(n.kind, NodeKind::Arrow) {
        return binding_link(n, cx, depth);
    }
    let ch = &n.children;
    if ch.len() != 3 {
        return Err(malformed_if(n));
    }
    let cond = expr(&ch[1], Slot::Other, cx, depth)?;
    let then = expr(&ch[2], Slot::Other, cx, depth)?;
    Ok(Link::Plain {
        span: n.span,
        cond,
        then,
    })
}

/// `IfChain[if, elif.., else?]`: each link's else is the expansion of the
/// rest of the chain; the final `else` supplies its single argument, and a
/// chain without one ends in nil. Links expand in source order, so the
/// first diagnostic is the earliest.
pub(super) fn if_chain(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    guard(
        n,
        depth.saturating_add(u32::try_from(n.children.len()).unwrap_or(u32::MAX)),
    )?;
    let mut links = Vec::with_capacity(n.children.len());
    let mut els = None;
    for (k, link) in n.children.iter().enumerate() {
        let lead = match link.kind {
            NodeKind::Arrow => kernel::arrow_lead(link),
            _ => kernel::head_name(link),
        };
        match (k, lead) {
            (0, Some("if")) | (1.., Some("elif")) => links.push(chain_link(link, cx, depth)?),
            (1.., Some("else")) if k + 1 == n.children.len() => {
                if !matches!(link.kind, NodeKind::Call) || link.children.len() != 2 {
                    return Err(malformed_if(link));
                }
                els = Some(expr(&link.children[1], Slot::Other, cx, depth)?);
            }
            (_, Some("elif" | "else")) => return Err(else_without_if(link)),
            _ => return Err(malformed_if(link)),
        }
    }
    if links.is_empty() {
        return Err(malformed_if(n));
    }
    let mut acc = els;
    for link in links.into_iter().rev() {
        let e = match acc.take() {
            Some(e) => e,
            None => syn_atom(cx, Atom::Nil, link_span(&link)),
        };
        acc = Some(link.build(cx, e));
    }
    match acc {
        Some(out) => Ok(with_span(out, n.span)),
        None => Err(malformed_if(n)),
    }
}

fn link_span(link: &Link) -> Span {
    match link {
        Link::Plain { span, .. } | Link::Binding { span, .. } => *span,
    }
}

/// The outermost `match` of a chain replaces the whole `IfChain`.
fn with_span(mut n: Node, span: Span) -> Node {
    n.span = span;
    n
}

/// `(for P S B)` becomes `(match (map S (-> (P) B)) {(-> (_) nil)})`.
pub(super) fn for_call(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    let ch = &n.children;
    if ch.len() != 4 {
        return Err(diag(
            DiagCode::MalformedFor,
            n,
            "expected `for PATTERN SOURCE: BODY`",
        ));
    }
    kernel::check_binder(&ch[1])?;
    pattern(&ch[1], Slot::Other, depth)?;
    let source = expr(&ch[2], Slot::Other, cx, depth)?;
    let body = expr(&ch[3], Slot::Other, cx, depth)?;
    let span = n.span;
    let lambda = clause(cx, span, vec![ch[1].clone()], body);
    let mapped = builtin_call(cx, "map", span, vec![source, lambda]);
    let wild = syn_atom(cx, Atom::Wildcard, span);
    let nil = syn_atom(cx, Atom::Nil, span);
    let discard = clause(cx, span, vec![wild], nil);
    Ok(match_of(cx, span, mapped, vec![discard]))
}

/// `(#neg X)` becomes `(neg X)`.
pub(super) fn neg(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    let args = expand_all(n, cx, depth)?;
    Ok(builtin_call(cx, "neg", n.span, args))
}

/// `(#? X D)` becomes `(or X D)`.
pub(super) fn fallback(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    let args = expand_all(n, cx, depth)?;
    Ok(builtin_call(cx, "or", n.span, args))
}

/// `(#interp parts..)` becomes `(concat parts..)`, empty text pieces dropped.
pub(super) fn interp(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    let mut args = Vec::with_capacity(n.children.len());
    for part in n.children.iter() {
        match &part.kind {
            NodeKind::Atom(Atom::Str(s)) if s.is_empty() => {}
            _ => args.push(expr(part, Slot::Other, cx, depth)?),
        }
    }
    Ok(builtin_call(cx, "concat", n.span, args))
}

fn expand_all(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Vec<Node>, Diagnostic> {
    n.children
        .iter()
        .map(|c| expr(c, Slot::Other, cx, depth))
        .collect()
}
