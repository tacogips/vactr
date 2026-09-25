//! Kernel shape validation and `is_kernel` (design 6.5.5 "Kernel validation").

use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::types::diag::{DiagCode, Diagnostic};

/// The kernel words and literal atoms that cannot be bound (6.5.5 "Hygiene").
const RESERVED: [&str; 14] = [
    "match", "fn", "let", "var", "upd", "enum", "if", "elif", "else", "for", "import", "nil",
    "true", "false",
];

/// True when `name` is a reserved word.
#[must_use]
pub(crate) fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// The head name of a call, when it is a `Sym`.
pub(crate) fn head_name(n: &Node) -> Option<&str> {
    match n.kind {
        NodeKind::Call => n.children.first()?.sym_name(),
        _ => None,
    }
}

/// The first lhs item's name of an arrow, when it is a `Sym`.
pub(crate) fn arrow_lead(n: &Node) -> Option<&str> {
    match n.kind {
        NodeKind::Arrow if n.children.len() >= 2 => n.children.first()?.sym_name(),
        _ => None,
    }
}

/// True for sugar that no kernel tree holds: `-x`, `?`, interpolation, an
/// `if` chain, a call headed `if`/`elif`/`else`/`for`, or a binding `if`.
pub(crate) fn is_sugar(n: &Node) -> bool {
    matches!(
        n.kind,
        NodeKind::Neg | NodeKind::Fallback | NodeKind::Interp | NodeKind::IfChain
    ) || matches!(head_name(n), Some("if" | "elif" | "else" | "for"))
        || matches!(arrow_lead(n), Some("if" | "elif" | "else"))
}

fn err(code: DiagCode, n: &Node, msg: &str) -> Diagnostic {
    Diagnostic::error(code, n.span, msg)
}

fn reserved(n: &Node) -> Diagnostic {
    err(DiagCode::ReservedWord, n, "a reserved word cannot be bound")
}

/// A binder at the top of a target or fn header: a name, a `name: type`
/// pair, or a list pattern. Rejects reserved words; other shapes pass.
pub(crate) fn check_binder(n: &Node) -> Result<(), Diagnostic> {
    match &n.kind {
        NodeKind::Atom(Atom::Sym(name)) if is_reserved(name) => Err(reserved(n)),
        NodeKind::Atom(Atom::Nil | Atom::Bool(_)) => Err(reserved(n)),
        NodeKind::Pair => match n.children.first() {
            Some(key) => match &key.kind {
                NodeKind::Atom(Atom::Keyword(name)) if is_reserved(name) => Err(reserved(key)),
                _ => Ok(()),
            },
            None => Ok(()),
        },
        NodeKind::List => n.children.iter().try_for_each(check_list_binder),
        _ => Ok(()),
    }
}

/// A name inside a list pattern. In a dict pattern `[amp: a]` the pair
/// value is the binder, and the key is only a dict key.
fn check_list_binder(n: &Node) -> Result<(), Diagnostic> {
    match &n.kind {
        NodeKind::Atom(Atom::Sym(name)) if is_reserved(name) => Err(reserved(n)),
        NodeKind::List | NodeKind::Splat => n.children.iter().try_for_each(check_list_binder),
        NodeKind::Pair => n.children.get(1).map_or(Ok(()), check_list_binder),
        _ => Ok(()),
    }
}

/// Validates the shape of a kernel-headed call (`match fn let var upd
/// enum`). Any other node passes.
pub(crate) fn check_shape(n: &Node) -> Result<(), Diagnostic> {
    match head_name(n) {
        Some("match") => match_shape(n),
        Some("fn") => fn_shape(n),
        Some("let" | "var") => binding_shape(n),
        Some("upd") => upd_shape(n),
        Some("enum") => enum_shape(n),
        _ => Ok(()),
    }
}

/// `(match SUBJ {Arrow+})`, every clause lhs non-empty. In a clause lhs a
/// top-level `if` splits the pattern from exactly one guard item.
fn match_shape(n: &Node) -> Result<(), Diagnostic> {
    let bad = |at: &Node| {
        err(
            DiagCode::MalformedMatch,
            at,
            "expected `match SUBJ:` with `pattern -> body` clauses",
        )
    };
    let ch = &n.children;
    if ch.len() != 3 || !matches!(ch[2].kind, NodeKind::Block) || ch[2].children.is_empty() {
        return Err(bad(n));
    }
    for clause in ch[2].children.iter() {
        if !matches!(clause.kind, NodeKind::Arrow) {
            return Err(bad(clause));
        }
        let lhs = clause_lhs(clause);
        if lhs.is_empty() {
            return Err(bad(clause));
        }
        if let Some(k) = guard_index(lhs) {
            let more = lhs[k + 1..].iter().any(|i| i.sym_name() == Some("if"));
            if k == 0 || lhs.len() != k + 2 || more {
                return Err(bad(clause));
            }
        }
    }
    Ok(())
}

/// The lhs items of an arrow (every child but the body).
pub(crate) fn clause_lhs(arrow: &Node) -> &[Node] {
    match arrow.children.split_last() {
        Some((_, lhs)) => lhs,
        None => &[],
    }
}

/// The index of the first top-level `if` in a clause lhs.
pub(crate) fn guard_index(lhs: &[Node]) -> Option<usize> {
    lhs.iter().position(|i| i.sym_name() == Some("if"))
}

/// `(fn NAME HEADER* Block)` with `NAME` a `Sym`. Parameters before a `->`
/// return-type marker cannot be reserved words.
fn fn_shape(n: &Node) -> Result<(), Diagnostic> {
    let ch = &n.children;
    let well_formed = ch.len() >= 3
        && ch[1].sym_name().is_some()
        && ch.last().is_some_and(|b| matches!(b.kind, NodeKind::Block));
    if !well_formed {
        return Err(err(
            DiagCode::MalformedFn,
            n,
            "expected `fn NAME PARAMS:` with a block body",
        ));
    }
    check_binder(&ch[1])?;
    for item in &ch[2..ch.len() - 1] {
        if matches!(item.kind, NodeKind::Atom(Atom::Op(Op::Arrow))) {
            break;
        }
        check_binder(item)?;
    }
    Ok(())
}

/// `(let TARGET EXPR)` or `(var TARGET EXPR)`: a name, a `name: type` pair
/// or a list pattern.
fn binding_shape(n: &Node) -> Result<(), Diagnostic> {
    let bad = || err(DiagCode::MalformedBinding, n, "expected `let TARGET EXPR`");
    let ch = &n.children;
    if ch.len() != 3 {
        return Err(bad());
    }
    let target = &ch[1];
    check_binder(target)?;
    let ok = match &target.kind {
        NodeKind::Atom(Atom::Sym(_)) | NodeKind::List => true,
        NodeKind::Pair => target
            .children
            .first()
            .is_some_and(|k| matches!(k.kind, NodeKind::Atom(Atom::Keyword(_)))),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(bad())
    }
}

/// `(upd NAME EXPR)`.
fn upd_shape(n: &Node) -> Result<(), Diagnostic> {
    let ch = &n.children;
    if ch.len() != 3 || ch[1].sym_name().is_none() {
        return Err(err(
            DiagCode::MalformedBinding,
            n,
            "expected `upd NAME EXPR`",
        ));
    }
    check_binder(&ch[1])
}

/// `(enum NAME Block)`, each line a `Sym` or a `Sym`-headed call.
fn enum_shape(n: &Node) -> Result<(), Diagnostic> {
    let bad = |at: &Node| {
        err(
            DiagCode::MalformedEnum,
            at,
            "expected `enum NAME:` with one variant per line",
        )
    };
    let ch = &n.children;
    if ch.len() != 3 || ch[1].sym_name().is_none() || !matches!(ch[2].kind, NodeKind::Block) {
        return Err(bad(n));
    }
    check_binder(&ch[1])?;
    for line in ch[2].children.iter() {
        let name = match line.kind {
            NodeKind::Call => line.children.first().filter(|h| h.sym_name().is_some()),
            _ => Some(line).filter(|l| l.sym_name().is_some()),
        };
        match name {
            Some(name) => check_binder(name)?,
            None => return Err(bad(line)),
        }
    }
    Ok(())
}

/// Where a node sits, for the `Builtin` and `Splat` position rules.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pos {
    Head,
    Arg,
    Item,
    Other,
}

/// True when the tree is kernel-only: no `Neg`, `Fallback`, `Interp`,
/// `IfChain` or `Error` node, no call headed `if`/`elif`/`else`/`for`,
/// `Builtin` atoms only as call heads, `Splat` only as a call argument or
/// list item, `Import` only at the root, and every kernel head in its shape.
#[must_use]
pub fn is_kernel(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Import(_)) || kernel_at(n, Pos::Other)
}

fn kernel_at(n: &Node, pos: Pos) -> bool {
    if is_sugar(n) {
        return false;
    }
    let all = |p: Pos| n.children.iter().all(|c| kernel_at(c, p));
    match &n.kind {
        NodeKind::Atom(Atom::Builtin(_)) => pos == Pos::Head,
        NodeKind::Atom(_) => true,
        NodeKind::Splat => matches!(pos, Pos::Arg | Pos::Item) && all(Pos::Other),
        NodeKind::Call => {
            check_shape(n).is_ok()
                && n.children
                    .iter()
                    .enumerate()
                    .all(|(k, c)| kernel_at(c, if k == 0 { Pos::Head } else { Pos::Arg }))
        }
        NodeKind::List => all(Pos::Item),
        NodeKind::Block | NodeKind::Pair | NodeKind::Arrow => all(Pos::Other),
        NodeKind::Neg
        | NodeKind::Fallback
        | NodeKind::Interp
        | NodeKind::IfChain
        | NodeKind::Import(_)
        | NodeKind::Error => false,
    }
}
