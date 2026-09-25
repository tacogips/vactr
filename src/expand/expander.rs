//! Traversal: expression positions are expanded; pattern and header
//! positions are only checked (design 6.5.5 "Positions").

use crate::expand::kernel::{self, clause_lhs, guard_index};
use crate::expand::sugar;
use crate::expand::ExpandCx;
use crate::reader::node::{Node, NodeKind};
use crate::types::diag::{DiagCode, Diagnostic};

/// The expansion depth cap. The reader caps bracket nesting at 128; pipes
/// and `?` folds nest calls without brackets, so the expander bounds its
/// own recursion and reports `nesting-too-deep` instead of overflowing.
pub(super) const MAX_DEPTH: u32 = 512;

/// Where an expression sits, for the `Splat` rule.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    /// A call argument.
    Arg,
    /// A list item.
    Item,
    /// Anywhere else.
    Other,
}

pub(super) fn diag(code: DiagCode, n: &Node, msg: &str) -> Diagnostic {
    Diagnostic::error(code, n.span, msg)
}

/// Fails with `nesting-too-deep` past the depth cap.
pub(super) fn guard(n: &Node, depth: u32) -> Result<(), Diagnostic> {
    if depth > MAX_DEPTH {
        Err(diag(
            DiagCode::NestingTooDeep,
            n,
            "the form nests too deeply to expand",
        ))
    } else {
        Ok(())
    }
}

/// The same node with new children; it keeps its id, kind and span.
pub(super) fn rebuild(n: &Node, children: Vec<Node>) -> Node {
    Node {
        id: n.id,
        kind: n.kind.clone(),
        span: n.span,
        children: children.into_boxed_slice(),
    }
}

/// One top-level form. `Import` is allowed only here.
pub(super) fn form(n: &Node, cx: &mut ExpandCx) -> Result<Node, Diagnostic> {
    match n.kind {
        NodeKind::Import(_) => Ok(n.clone()),
        _ => expr(n, Slot::Other, cx, 0),
    }
}

/// Expands an expression position.
pub(super) fn expr(
    n: &Node,
    slot: Slot,
    cx: &mut ExpandCx,
    depth: u32,
) -> Result<Node, Diagnostic> {
    guard(n, depth)?;
    let d = depth + 1;
    match &n.kind {
        NodeKind::Atom(_) => Ok(n.clone()),
        NodeKind::Call => call(n, cx, d),
        NodeKind::List => each(n, Slot::Item, cx, d),
        NodeKind::Block => each(n, Slot::Other, cx, d),
        NodeKind::Pair => {
            let mut out = Vec::with_capacity(n.children.len());
            for (k, c) in n.children.iter().enumerate() {
                out.push(if k == 0 {
                    c.clone()
                } else {
                    expr(c, Slot::Other, cx, d)?
                });
            }
            Ok(rebuild(n, out))
        }
        NodeKind::Arrow => arrow(n, cx, d),
        NodeKind::Splat => {
            if slot == Slot::Other {
                return Err(diag(
                    DiagCode::MisplacedSplat,
                    n,
                    "`&` belongs only in a call argument or a list item",
                ));
            }
            each(n, Slot::Other, cx, d)
        }
        NodeKind::Neg => sugar::neg(n, cx, d),
        NodeKind::Fallback => sugar::fallback(n, cx, d),
        NodeKind::Interp => sugar::interp(n, cx, d),
        NodeKind::IfChain => sugar::if_chain(n, cx, d),
        NodeKind::Import(_) => Err(diag(
            DiagCode::ImportNotTopLevel,
            n,
            "`import` is allowed only at the top level",
        )),
        NodeKind::Error => Err(diag(
            DiagCode::ReadErrorPresent,
            n,
            "the form has a reader error",
        )),
    }
}

/// Expands every child in the same slot.
fn each(n: &Node, slot: Slot, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    let out = n
        .children
        .iter()
        .map(|c| expr(c, slot, cx, depth))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rebuild(n, out))
}

fn call(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    match kernel::head_name(n) {
        Some("if") => sugar::if_call(n, cx, depth),
        Some("elif" | "else") => Err(else_without_if(n)),
        Some("for") => sugar::for_call(n, cx, depth),
        Some("match") => match_form(n, cx, depth),
        Some("fn") => fn_form(n, cx, depth),
        Some("let" | "var" | "upd") => binding_form(n, cx, depth),
        Some("enum") => enum_form(n, depth),
        _ => {
            let mut out = Vec::with_capacity(n.children.len());
            for (k, c) in n.children.iter().enumerate() {
                let slot = if k == 0 { Slot::Other } else { Slot::Arg };
                out.push(expr(c, slot, cx, depth)?);
            }
            Ok(rebuild(n, out))
        }
    }
}

pub(super) fn else_without_if(n: &Node) -> Diagnostic {
    diag(
        DiagCode::ElseWithoutIf,
        n,
        "`elif` or `else` must follow an `if` at the same level",
    )
}

/// A lambda or statement arrow: the lhs is a pattern, the body an
/// expression. An lhs led by `if` is a binding `if`.
fn arrow(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    match kernel::arrow_lead(n) {
        Some("if") => return sugar::binding_if(n, None, cx, depth),
        Some("elif" | "else") => return Err(else_without_if(n)),
        _ => {}
    }
    let Some((body, lhs)) = n.children.split_last() else {
        return Ok(n.clone());
    };
    let mut out = Vec::with_capacity(n.children.len());
    for item in lhs {
        pattern(item, Slot::Other, depth)?;
        out.push(item.clone());
    }
    out.push(expr(body, Slot::Other, cx, depth)?);
    Ok(rebuild(n, out))
}

/// Checks a pattern or header position, which is never expanded: sugar there
/// is `sugar-in-pattern`, a nested kernel head must be in shape, and a
/// `Splat` must be a call argument or list item.
pub(super) fn pattern(n: &Node, slot: Slot, depth: u32) -> Result<(), Diagnostic> {
    guard(n, depth)?;
    if kernel::is_sugar(n) {
        return Err(diag(
            DiagCode::SugarInPattern,
            n,
            "sugar is not allowed in a pattern or header position",
        ));
    }
    kernel::check_shape(n)?;
    if matches!(n.kind, NodeKind::Splat) && slot == Slot::Other {
        return Err(diag(
            DiagCode::MisplacedSplat,
            n,
            "`&` belongs only in a call argument or a list item",
        ));
    }
    for (k, c) in n.children.iter().enumerate() {
        let child_slot = match n.kind {
            NodeKind::Call if k > 0 => Slot::Arg,
            NodeKind::List => Slot::Item,
            _ => Slot::Other,
        };
        pattern(c, child_slot, depth + 1)?;
    }
    Ok(())
}

/// `(match SUBJ {clauses})`: the subject and bodies are expressions, each
/// clause pattern is checked, and a guard after a top-level `if` is expanded.
fn match_form(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    kernel::check_shape(n)?;
    let subject = expr(&n.children[1], Slot::Other, cx, depth)?;
    let block = &n.children[2];
    let mut clauses = Vec::with_capacity(block.children.len());
    for clause in block.children.iter() {
        let lhs = clause_lhs(clause);
        let split = guard_index(lhs).unwrap_or(lhs.len());
        let mut out = Vec::with_capacity(clause.children.len());
        for item in &lhs[..split] {
            pattern(item, Slot::Other, depth)?;
            out.push(item.clone());
        }
        if let Some((marker, rest)) = lhs[split..].split_first() {
            out.push(marker.clone());
            for g in rest {
                out.push(expr(g, Slot::Other, cx, depth)?);
            }
        }
        if let Some(body) = clause.children.last() {
            out.push(expr(body, Slot::Other, cx, depth)?);
        }
        clauses.push(rebuild(clause, out));
    }
    Ok(rebuild(
        n,
        vec![n.children[0].clone(), subject, rebuild(block, clauses)],
    ))
}

/// `(fn NAME HEADER* Block)`: the header is checked, the body expanded.
fn fn_form(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    kernel::check_shape(n)?;
    let ch = &n.children;
    let last = ch.len() - 1;
    let mut out = Vec::with_capacity(ch.len());
    for (k, c) in ch.iter().enumerate() {
        if k == last {
            out.push(expr(c, Slot::Other, cx, depth)?);
        } else {
            if k > 0 {
                pattern(c, Slot::Arg, depth)?;
            }
            out.push(c.clone());
        }
    }
    Ok(rebuild(n, out))
}

/// `(let TARGET EXPR)`, `(var TARGET EXPR)` and `(upd NAME EXPR)`.
fn binding_form(n: &Node, cx: &mut ExpandCx, depth: u32) -> Result<Node, Diagnostic> {
    kernel::check_shape(n)?;
    let ch = &n.children;
    pattern(&ch[1], Slot::Other, depth)?;
    let value = expr(&ch[2], Slot::Other, cx, depth)?;
    Ok(rebuild(n, vec![ch[0].clone(), ch[1].clone(), value]))
}

/// `(enum NAME {variants})`: the body is a header position.
fn enum_form(n: &Node, depth: u32) -> Result<Node, Diagnostic> {
    kernel::check_shape(n)?;
    for line in n.children[2].children.iter() {
        pattern(line, Slot::Other, depth)?;
    }
    Ok(n.clone())
}
