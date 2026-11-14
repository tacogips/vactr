//! One statement's tokens to a `Node` (6.5.4 "Statement grammar").

use std::rc::Rc;

use crate::reader::import::{parse_import, AliasEnv};
use crate::reader::layout::{group_if_chains, BlockSkel, Stmt};
use crate::reader::lexer::{StrPart, Tok, Token};
use crate::reader::node::{Atom, Node, NodeKind, Op};
use crate::reader::span::{FileId, Span};
use crate::reader::MAX_NESTING;
use crate::types::diag::{DiagCode, Diagnostic};

/// A backstop on total tree depth (pipes, fallbacks and arrows nest nodes
/// too), so later recursive passes and `Drop` stay within the stack.
const MAX_TREE_DEPTH: u32 = 1024;

/// Errors are boxed to keep the recursive parse frames small.
type PResult<T> = Result<T, Box<Diagnostic>>;

fn err(code: DiagCode, span: Span, msg: &str) -> Box<Diagnostic> {
    Box::new(Diagnostic::error(code, span, msg))
}

/// Nesting so far: `nest` counts groups, lists, interpolation and blocks
/// (capped at 128); `tree` counts every node level.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Depth {
    nest: u32,
    tree: u32,
}

impl Depth {
    pub(crate) const TOP: Depth = Depth { nest: 0, tree: 0 };

    fn check(self, span: Span) -> PResult<Depth> {
        if self.nest > MAX_NESTING || self.tree > MAX_TREE_DEPTH {
            Err(err(
                DiagCode::NestingTooDeep,
                span,
                "nesting deeper than 128",
            ))
        } else {
            Ok(self)
        }
    }

    /// One more group, list, interpolation or block.
    fn nest(self, span: Span) -> PResult<Depth> {
        Depth {
            nest: self.nest.saturating_add(1),
            tree: self.tree.saturating_add(1),
        }
        .check(span)
    }

    /// `k` more node levels.
    fn tree(self, k: usize, span: Span) -> PResult<Depth> {
        let k = u32::try_from(k).unwrap_or(u32::MAX);
        Depth {
            nest: self.nest,
            tree: self.tree.saturating_add(k),
        }
        .check(span)
    }
}

/// Where an item list is read. Operands never hold a top-level `->`, `>` or
/// `?`; a list rejects `->` and `?`; a flat list keeps them as `Op` atoms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Flat,
    List,
    Operand,
}

/// An atom token read as a node.
struct Leaf {
    node: Node,
    rangeable: bool,
}

/// A prefix collected before an item's base: `&`, `-`, or a pair key (with
/// the span of its colon).
enum Wrap {
    Splat(Span),
    Neg(Span),
    Key(Node, Span),
}

/// One operand of a pipeline.
struct Operand<'t> {
    ts: &'t [Token],
    /// The first operand of a later segment: the running value is piped in.
    piped: bool,
    /// Not the first operand of its segment: it is a `?` fallback.
    fallback: bool,
    /// The segment has more than one operand.
    in_fallback: bool,
    /// The last operand of the statement, which takes the trailing block.
    last: bool,
    /// The `?` next to it, or the `>` before its segment.
    sep: Option<Span>,
}

/// Splits at each `>` that is not a segment's first token, then each segment
/// at `?`.
#[inline(never)]
fn split_operands(ts: &[Token]) -> Vec<Operand<'_>> {
    let mut segs: Vec<(&[Token], Option<Span>)> = Vec::new();
    let mut seg_start = 0;
    let mut pipe_before = None;
    for p in top_level(ts, |t| t.is_op(Op::Gt)) {
        if p != seg_start {
            segs.push((ts.get(seg_start..p).unwrap_or(&[]), pipe_before));
            pipe_before = ts.get(p).map(|t| t.span);
            seg_start = p + 1;
        }
    }
    segs.push((ts.get(seg_start..).unwrap_or(&[]), pipe_before));
    let mut ops = Vec::new();
    let n_segs = segs.len();
    for (si, (seg, pipe)) in segs.into_iter().enumerate() {
        let qs = top_level(seg, |t| t.is_op(Op::Question));
        let n_ops = qs.len() + 1;
        let mut start = 0;
        for oi in 0..n_ops {
            let end = qs.get(oi).copied().unwrap_or(seg.len());
            let q_near = qs
                .get(oi)
                .or(qs.last())
                .and_then(|&q| seg.get(q))
                .map(|t| t.span);
            ops.push(Operand {
                ts: seg.get(start..end).unwrap_or(&[]),
                piped: oi == 0 && si > 0,
                fallback: oi > 0,
                in_fallback: n_ops > 1,
                last: si + 1 == n_segs && oi + 1 == n_ops,
                sep: if n_ops > 1 { q_near } else { pipe },
            });
            start = end + 1;
        }
    }
    ops
}

/// Folds one operand into the running value: a piped operand gets the value
/// as its first argument; a fallback operand wraps it in `Fallback`.
#[inline(never)]
fn fold_operand(
    value: Option<Node>,
    items: Vec<Node>,
    op: &Operand<'_>,
    at: Span,
) -> PResult<Node> {
    if items.is_empty() {
        let span = op.sep.unwrap_or(at);
        if op.in_fallback {
            return Err(err(
                DiagCode::MisplacedFallback,
                span,
                "`?` needs a value on both sides",
            ));
        }
        return Err(err(DiagCode::EmptyPipe, span, "`>` needs a call after it"));
    }
    if op.piped {
        let Some(prev) = value else {
            return Err(err(DiagCode::EmptyPipe, at, "`>` needs a value before it"));
        };
        let mut it = items.into_iter();
        let mut children = Vec::new();
        children.extend(it.next());
        children.push(prev);
        children.extend(it);
        return Ok(call(children));
    }
    let node = operand(items);
    match value {
        Some(lhs) if op.fallback => {
            let span = lhs.span.join(node.span);
            Ok(Node::new(NodeKind::Fallback, span, vec![lhs, node]))
        }
        _ => Ok(node),
    }
}

/// The diagnostic for a prefix with nothing after it.
fn missing_operand(last: Option<&Wrap>, next: Option<&Token>, here: Span) -> Box<Diagnostic> {
    let at = next.map_or(here, |t| t.span);
    match last {
        Some(Wrap::Key(_, colon)) => err(DiagCode::BadPair, *colon, "a pair needs one value"),
        Some(Wrap::Splat(span)) => err(DiagCode::MisplacedSplat, *span, "`&` needs an operand"),
        Some(Wrap::Neg(span)) => err(DiagCode::StrayChar, *span, "`-` needs an operand"),
        None if next.is_some() => err(DiagCode::StrayChar, at, "unmatched closing bracket"),
        None => err(DiagCode::EmptyGroup, at, "missing item"),
    }
}

/// Reads statements against a document-local alias environment.
pub(crate) struct Parser<'a> {
    src: &'a str,
    file: FileId,
    pub env: AliasEnv,
    pub diags: Vec<Diagnostic>,
}

fn is_open(t: &Token) -> bool {
    matches!(t.kind, Tok::LBrace | Tok::LBracket)
}

fn is_close(t: &Token) -> bool {
    matches!(t.kind, Tok::RBrace | Tok::RBracket)
}

/// Indexes of tokens outside brackets that satisfy `f`.
fn top_level(ts: &[Token], f: impl Fn(&Token) -> bool) -> Vec<usize> {
    let mut depth = 0u32;
    let mut out = Vec::new();
    for (k, t) in ts.iter().enumerate() {
        if is_open(t) {
            depth += 1;
        } else if is_close(t) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && f(t) {
            out.push(k);
        }
    }
    out
}

/// The index of the closer matching an opener just before `start`.
fn matching(ts: &[Token], start: usize) -> Option<usize> {
    let mut depth = 0u32;
    for (k, t) in ts.iter().enumerate().skip(start) {
        if is_open(t) {
            depth += 1;
        } else if is_close(t) {
            if depth == 0 {
                return Some(k);
            }
            depth -= 1;
        }
    }
    None
}

fn missing_value(span: Span) -> Box<Diagnostic> {
    err(
        DiagCode::BindingWithoutValue,
        span,
        "a binding needs a value",
    )
}

fn call(children: Vec<Node>) -> Node {
    let span = span_of(&children);
    Node::new(NodeKind::Call, span, children)
}

fn span_of(children: &[Node]) -> Span {
    match (children.first(), children.last()) {
        (Some(a), Some(b)) => a.span.join(b.span),
        _ => Span::new(FileId::CONSOLE, 0, 0),
    }
}

/// One item reads as itself; two or more read as a `Call`.
fn operand(mut items: Vec<Node>) -> Node {
    if items.len() == 1 {
        if let Some(only) = items.pop() {
            return only;
        }
    }
    call(items)
}

impl<'a> Parser<'a> {
    pub(crate) fn new(src: &'a str, file: FileId, env: AliasEnv) -> Self {
        Parser {
            src,
            file,
            env,
            diags: Vec::new(),
        }
    }

    /// An empty span at the start of the file, for errors with no token.
    fn nowhere(&self) -> Span {
        Span::new(self.file, 0, 0)
    }

    /// Reads one statement. A bad statement becomes one `Error` node; its
    /// diagnostics are recorded and the next statement still reads.
    pub(crate) fn statement(&mut self, stmt: &Stmt, d: Depth, top: bool) -> Node {
        let error = Node::new(NodeKind::Error, stmt.span, Vec::new());
        if !stmt.diags.is_empty() {
            self.diags.extend(stmt.diags.iter().cloned());
            if let Some(b) = &stmt.block {
                self.block_for_diags(b, d);
            }
            return error;
        }
        let mut block = stmt.block.as_ref();
        match self.statement_inner(stmt, &mut block, d, top) {
            Ok(node) => node,
            Err(diag) => {
                self.diags.push(*diag);
                if let Some(b) = block {
                    self.block_for_diags(b, d);
                }
                error
            }
        }
    }

    fn block_for_diags(&mut self, b: &BlockSkel, d: Depth) {
        if let Err(diag) = self.block_node(b, d) {
            self.diags.push(*diag);
        }
    }

    fn statement_inner(
        &mut self,
        stmt: &Stmt,
        block: &mut Option<&BlockSkel>,
        d: Depth,
        top: bool,
    ) -> PResult<Node> {
        if let Some(code) = stmt.import {
            return self.import(stmt, code, top);
        }
        let ts = stmt.tokens.as_slice();
        match ts.first().map(|t| &t.kind) {
            Some(Tok::Ident(h)) if matches!(&**h, "let" | "var" | "upd") => {
                self.binding(ts, block, d)
            }
            Some(Tok::Ident(h)) if &**h == "fn" => {
                let d1 = d.tree(1, stmt.span)?;
                let mut items = self.items(ts, Mode::Flat, d1)?;
                if let Some(b) = block.take() {
                    items.push(self.block_node(b, d1)?);
                }
                Ok(call(items))
            }
            _ => {
                if ts.is_empty() && block.is_none() {
                    return Err(err(DiagCode::EmptyGroup, stmt.span, "empty statement"));
                }
                self.expr(ts, block, d, stmt.span)
            }
        }
    }

    fn import(&mut self, stmt: &Stmt, code: Span, top: bool) -> PResult<Node> {
        if !top {
            return Err(err(
                DiagCode::ImportNotTopLevel,
                code,
                "`import` is allowed only at the top level",
            ));
        }
        if stmt.continued || stmt.block.is_some() {
            return Err(err(DiagCode::BadImport, stmt.span, "an import is one line"));
        }
        let text = self
            .src
            .get(code.start as usize..code.end as usize)
            .unwrap_or("");
        let decl = parse_import(text, code).map_err(|m| err(DiagCode::BadImport, code, m))?;
        self.env
            .bind(Rc::clone(&decl.prefix), Rc::clone(&decl.path));
        Ok(Node::new(
            NodeKind::Import(Box::new(decl)),
            code,
            Vec::new(),
        ))
    }

    /// `let`/`var`/`upd`: `Call[head, TARGET, EXPR]`.
    fn binding(&mut self, ts: &[Token], block: &mut Option<&BlockSkel>, d: Depth) -> PResult<Node> {
        let Some(head_tok) = ts.first() else {
            return Err(missing_value(self.nowhere()));
        };
        let d1 = d.tree(1, head_tok.span)?;
        let head = match &head_tok.kind {
            Tok::Ident(name) => Node::atom(Atom::Sym(Rc::clone(name)), head_tok.span),
            _ => return Err(missing_value(head_tok.span)),
        };
        let rest = ts.get(1..).unwrap_or(&[]);
        let missing = || missing_value(head_tok.span);
        if rest.is_empty() {
            return Err(missing());
        }
        let mut i = 0;
        let target = self.item(rest, &mut i, Mode::Flat, d1)?;
        let expr_ts = rest.get(i..).unwrap_or(&[]);
        let value = if expr_ts.is_empty() {
            match block.take() {
                Some(b) => self.block_node(b, d1)?,
                None => return Err(missing()),
            }
        } else {
            self.expr(expr_ts, block, d1, head_tok.span)?
        };
        Ok(call(vec![head, target, value]))
    }

    /// The expression rule: arrow, then pipe, then fallback, then operand.
    /// Splitting at the first `->` repeats on the body, so `a -> b -> c` is
    /// `Arrow[a, Arrow[b, c]]`; the chain is built without recursion.
    fn expr(
        &mut self,
        ts: &[Token],
        block: &mut Option<&BlockSkel>,
        d: Depth,
        at: Span,
    ) -> PResult<Node> {
        let arrows = top_level(ts, |t| t.is_op(Op::Arrow));
        if arrows.is_empty() {
            return self.pipeline(ts, block, d, at);
        }
        let d1 = d.tree(arrows.len(), at)?;
        let mut lhss = Vec::with_capacity(arrows.len());
        let mut start = 0;
        let mut arrow = at;
        for &k in &arrows {
            arrow = ts.get(k).map_or(at, |t| t.span);
            lhss.push((
                self.items(ts.get(start..k).unwrap_or(&[]), Mode::Flat, d1)?,
                arrow,
            ));
            start = k + 1;
        }
        let body_ts = ts.get(start..).unwrap_or(&[]);
        let mut body = if body_ts.is_empty() {
            match block.take() {
                Some(b) => self.block_node(b, d1)?,
                None => return Err(err(DiagCode::ArrowWithoutBody, arrow, "`->` needs a body")),
            }
        } else {
            self.pipeline(body_ts, block, d1, arrow)?
        };
        for (mut children, arrow) in lhss.into_iter().rev() {
            let start = children.first().map_or(arrow, |c| c.span);
            let span = start.join(body.span);
            children.push(body);
            body = Node::new(NodeKind::Arrow, span, children);
        }
        Ok(body)
    }

    /// Pipe segments split at each `>` that is not a segment's first token;
    /// each segment splits at `?` into operands. The splitting and folding
    /// live in helpers so this recursive frame stays small.
    fn pipeline(
        &mut self,
        ts: &[Token],
        block: &mut Option<&BlockSkel>,
        d: Depth,
        at: Span,
    ) -> PResult<Node> {
        let ops = split_operands(ts);
        let d2 = d.tree(ops.len().saturating_sub(1), at)?;
        let d3 = d2.tree(1, at)?;
        let mut value: Option<Node> = None;
        for op in &ops {
            let mut items = self.items(op.ts, Mode::Operand, d3)?;
            if op.last {
                if let Some(b) = block.take() {
                    items.push(self.block_node(b, d3)?);
                }
            }
            value = Some(fold_operand(value.take(), items, op, at)?);
        }
        value.ok_or_else(|| err(DiagCode::EmptyGroup, at, "empty expression"))
    }

    fn items(&mut self, ts: &[Token], mode: Mode, d: Depth) -> PResult<Vec<Node>> {
        let mut i = 0;
        let mut out = Vec::new();
        while i < ts.len() {
            out.push(self.item(ts, &mut i, mode, d)?);
        }
        Ok(out)
    }

    /// One item: an atom, a pair, a splat, a negation, a list or a group.
    ///
    /// Prefix chains (`& -x`, `a: b: 1`) are collected in a loop and folded
    /// afterwards, so only groups, lists and interpolation recurse.
    fn item(&mut self, ts: &[Token], i: &mut usize, mode: Mode, d: Depth) -> PResult<Node> {
        let Some(here) = ts.get(*i).map(|t| t.span) else {
            return Err(missing_operand(None, None, self.nowhere()));
        };
        let mut wraps: Vec<Wrap> = Vec::new();
        loop {
            let Some(tok) = ts.get(*i) else { break };
            let wrap = match &tok.kind {
                Tok::Op(Op::Amp) => Wrap::Splat(tok.span),
                Tok::Neg => Wrap::Neg(tok.span),
                _ if tok.is_key() && ts.get(*i + 1).is_some_and(|t| t.kind == Tok::PairColon) => {
                    let key = self.key(tok, d)?;
                    *i += 1;
                    Wrap::Key(key, ts.get(*i).map_or(tok.span, |t| t.span))
                }
                _ => break,
            };
            *i += 1;
            wraps.push(wrap);
        }
        if ts.get(*i).map_or(true, is_close) {
            return Err(missing_operand(wraps.last(), ts.get(*i), here));
        }
        let mut node = self.base(ts, i, mode, d.tree(wraps.len(), here)?)?;
        while let Some(wrap) = wraps.pop() {
            node = match wrap {
                Wrap::Splat(span) => Node::new(NodeKind::Splat, span.join(node.span), vec![node]),
                Wrap::Neg(span) => Node::new(NodeKind::Neg, span.join(node.span), vec![node]),
                Wrap::Key(key, _) => {
                    Node::new(NodeKind::Pair, key.span.join(node.span), vec![key, node])
                }
            };
        }
        Ok(node)
    }

    /// A pair key: an identifier becomes a keyword; a string or number stays.
    #[inline(never)]
    fn key(&mut self, tok: &Token, d: Depth) -> PResult<Node> {
        match &tok.kind {
            Tok::Ident(name) => Ok(Node::atom(Atom::Keyword(Rc::clone(name)), tok.span)),
            Tok::Str(parts) => self.string(parts, tok.span, d),
            _ => Ok(self.leaf(tok, Mode::Operand)?.node),
        }
    }

    /// An item without prefixes: a group, a list, a string, or an atom with
    /// an optional range.
    fn base(&mut self, ts: &[Token], i: &mut usize, mode: Mode, d: Depth) -> PResult<Node> {
        let Some(tok) = ts.get(*i) else {
            return Err(missing_operand(None, None, self.nowhere()));
        };
        *i += 1;
        let span = tok.span;
        match &tok.kind {
            Tok::LBrace => return self.group(ts, i, span, d),
            Tok::LBracket => return self.list(ts, i, span, d),
            Tok::Str(parts) => return self.string(parts, span, d),
            _ => {}
        }
        let leaf = self.leaf(tok, mode)?;
        let range = ts
            .get(*i)
            .filter(|t| leaf.rangeable && t.is_op(Op::Range) && !t.space_before);
        match range {
            Some(dots) => self.range(leaf.node, dots.span, ts, i),
            None => Ok(leaf.node),
        }
    }

    /// An atom token: its node, the pair key it would be, and whether it may
    /// start a range.
    #[inline(never)]
    fn leaf(&self, tok: &Token, mode: Mode) -> PResult<Leaf> {
        let span = tok.span;
        let plain = |atom: Atom| Leaf {
            node: Node::atom(atom, span),
            rangeable: false,
        };
        let number = |atom: Atom| Leaf {
            node: Node::atom(atom, span),
            rangeable: true,
        };
        Ok(match &tok.kind {
            Tok::Int(n) => number(Atom::Int(*n)),
            Tok::Float { value, exact } => number(Atom::Float {
                value: *value,
                exact: *exact,
            }),
            Tok::Ratio(r) => number(Atom::Ratio(*r)),
            Tok::Ident(name) => {
                let atom = match &**name {
                    "nil" => Atom::Nil,
                    "true" => Atom::Bool(true),
                    "false" => Atom::Bool(false),
                    _ => Atom::Sym(Rc::clone(name)),
                };
                number(atom)
            }
            Tok::Qualified { prefix, name } => {
                if !self.env.is_bound(prefix) {
                    return Err(err(
                        DiagCode::UnboundQualifier,
                        span,
                        "qualified name without an `import` binding its prefix",
                    ));
                }
                let mut leaf = plain(Atom::Qualified {
                    prefix: Rc::clone(prefix),
                    name: Rc::clone(name),
                });
                leaf.rangeable = true;
                leaf
            }
            Tok::Keyword(k) => plain(Atom::Keyword(Rc::clone(k))),
            Tok::Wildcard => plain(Atom::Wildcard),
            Tok::ConsoleReg(n) => plain(Atom::ConsoleReg(*n)),
            Tok::Path(text) => plain(Atom::Path(Rc::clone(text))),
            Tok::Url(text) => plain(Atom::Url(Rc::clone(text))),
            Tok::Op(Op::Arrow) if mode == Mode::List => {
                return Err(err(DiagCode::MisplacedArrow, span, "`->` inside `[..]`"));
            }
            Tok::Op(Op::Question) if mode == Mode::List => {
                return Err(err(DiagCode::MisplacedFallback, span, "`?` inside `[..]`"));
            }
            Tok::Op(op) => plain(Atom::Op(*op)),
            Tok::RBrace | Tok::RBracket => {
                return Err(err(DiagCode::StrayChar, span, "unmatched closing bracket"));
            }
            Tok::PairColon | Tok::BlockColon => {
                return Err(err(DiagCode::MisplacedColon, span, "misplaced `:`"));
            }
            Tok::Neg | Tok::LBrace | Tok::LBracket | Tok::Str(_) => {
                return Err(err(DiagCode::StrayChar, span, "unexpected token"));
            }
        })
    }

    /// `a..b` and `a..` written without spaces: `Call[Op(..), a, b?]`.
    #[inline(never)]
    fn range(&self, start: Node, dots: Span, ts: &[Token], i: &mut usize) -> PResult<Node> {
        *i += 1;
        let mut children = vec![Node::atom(Atom::Op(Op::Range), dots), start];
        let end_tok = ts.get(*i).filter(|t| {
            !t.space_before
                && matches!(
                    t.kind,
                    Tok::Int(_)
                        | Tok::Float { .. }
                        | Tok::Ratio(_)
                        | Tok::Ident(_)
                        | Tok::Qualified { .. }
                )
        });
        if let Some(t) = end_tok {
            *i += 1;
            children.push(self.leaf(t, Mode::Operand)?.node);
        }
        let span = span_of(&children[1..]).join(dots);
        Ok(Node::new(NodeKind::Call, span, children))
    }

    /// A `{..}` group: an arrow reads as that arrow (the lambda), anything
    /// else as `Block[expr]`.
    fn group(&mut self, ts: &[Token], i: &mut usize, open: Span, d: Depth) -> PResult<Node> {
        let d1 = d.nest(open)?;
        let unclosed = |end: Span| {
            err(
                DiagCode::UnclosedGroup,
                open.join(end),
                "`{` is not closed on this line",
            )
        };
        let close = match matching(ts, *i) {
            Some(j) if ts.get(j).is_some_and(|t| t.kind == Tok::RBrace) => j,
            _ => return Err(unclosed(ts.last().map_or(open, |t| t.span))),
        };
        let inner = ts.get(*i..close).unwrap_or(&[]);
        let span = open.join(ts.get(close).map_or(open, |t| t.span));
        *i = close + 1;
        if inner.is_empty() {
            return Err(err(DiagCode::EmptyGroup, span, "`{}` needs an expression"));
        }
        let mut node = self.expr(inner, &mut None, d1, span)?;
        if matches!(node.kind, NodeKind::Arrow) {
            node.span = span;
            return Ok(node);
        }
        Ok(Node::new(NodeKind::Block, span, vec![node]))
    }

    /// A `[..]` list of items.
    fn list(&mut self, ts: &[Token], i: &mut usize, open: Span, d: Depth) -> PResult<Node> {
        let d1 = d.nest(open)?;
        let close = match matching(ts, *i) {
            Some(j) if ts.get(j).is_some_and(|t| t.kind == Tok::RBracket) => j,
            _ => {
                let end = ts.last().map_or(open, |t| t.span);
                return Err(err(
                    DiagCode::UnclosedBracket,
                    open.join(end),
                    "`[` is not closed",
                ));
            }
        };
        let inner = ts.get(*i..close).unwrap_or(&[]);
        let span = open.join(ts.get(close).map_or(open, |t| t.span));
        *i = close + 1;
        let items = self.items(inner, Mode::List, d1)?;
        Ok(Node::new(NodeKind::List, span, items))
    }

    /// A string literal: a `Str` atom, or `Interp` when it has a `{}` part.
    fn string(&mut self, parts: &[StrPart], span: Span, d: Depth) -> PResult<Node> {
        if parts.iter().all(|p| matches!(p, StrPart::Text { .. })) {
            let mut text = String::new();
            for p in parts {
                if let StrPart::Text { text: t, .. } = p {
                    text.push_str(t);
                }
            }
            return Ok(Node::atom(Atom::Str(Rc::from(text.as_str())), span));
        }
        let d1 = d.nest(span)?;
        let mut children = Vec::with_capacity(parts.len());
        for p in parts {
            match p {
                StrPart::Text { text, span } => {
                    children.push(Node::atom(Atom::Str(Rc::clone(text)), *span))
                }
                StrPart::Expr { tokens, span } => {
                    if tokens.is_empty() {
                        return Err(err(DiagCode::EmptyGroup, *span, "`{}` needs an expression"));
                    }
                    children.push(self.expr(tokens, &mut None, d1, *span)?);
                }
            }
        }
        Ok(Node::new(NodeKind::Interp, span, children))
    }

    /// A block body: statements, grouped into `IfChain`s.
    pub(crate) fn block_node(&mut self, b: &BlockSkel, d: Depth) -> PResult<Node> {
        let d1 = d.nest(b.colon)?;
        let nodes: Vec<Node> = b
            .stmts
            .iter()
            .map(|s| self.statement(s, d1, false))
            .collect();
        Ok(Node::new(NodeKind::Block, b.span, group_if_chains(nodes)))
    }
}
