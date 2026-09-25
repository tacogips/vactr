//! Reader output: nodes, atoms and trivia (design 6.1, 6.5.4).

use std::rc::Rc;

use crate::reader::import::ImportDecl;
use crate::reader::span::{NodeId, Span};
use crate::value::ratio::Ratio64;

/// One S-expression node. Children are ordered as the table in 6.5.4 says.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub span: Span,
    pub children: Box<[Node]>,
}

/// The node kinds of design 6.5.4.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Atom(Atom),
    /// `[head, args..]`.
    Call,
    /// Items; a dict literal is a list of pairs.
    List,
    /// Statements.
    Block,
    /// `[key, value]`.
    Pair,
    /// `[lhs.., body]`; the body is always the last child.
    Arrow,
    /// `[expr]`.
    Splat,
    /// `[expr]`.
    Neg,
    /// `[lhs, rhs]`.
    Fallback,
    /// `[parts..]`, each a `Str` atom or an expression.
    Interp,
    /// `[if-stmt, elif-stmt.., else-stmt?]`.
    IfChain,
    Import(Box<ImportDecl>),
    Error,
}

/// Leaf values. The reader stores names as text, never interned ids.
#[derive(Clone, Debug, PartialEq)]
pub enum Atom {
    Int(i64),
    Float {
        value: f64,
        exact: Option<Ratio64>,
    },
    Ratio(Ratio64),
    Str(Rc<str>),
    Keyword(Rc<str>),
    Sym(Rc<str>),
    Qualified {
        prefix: Rc<str>,
        name: Rc<str>,
    },
    Op(Op),
    Wildcard,
    ConsoleReg(u32),
    Nil,
    Bool(bool),
    /// Produced only by the expander (6.5.5).
    Builtin(Rc<str>),
}

/// The operator token class: `+ - * / = < > <= >= .. -> & | ?`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
    Range,
    Arrow,
    Amp,
    Bar,
    Question,
}

impl Op {
    /// The source spelling of the operator.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Eq => "=",
            Op::Lt => "<",
            Op::Gt => ">",
            Op::Le => "<=",
            Op::Ge => ">=",
            Op::Range => "..",
            Op::Arrow => "->",
            Op::Amp => "&",
            Op::Bar => "|",
            Op::Question => "?",
        }
    }
}

/// Comments and directives, in source order (6.5.4 "Trivia").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trivia {
    pub items: Vec<TriviaItem>,
}

/// One comment. The span runs from `#` to the end of the line text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriviaItem {
    pub span: Span,
    pub kind: TriviaKind,
}

/// A `#@` comment is a `Directive`; every other comment is a `Comment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriviaKind {
    Comment,
    Directive,
}

impl Node {
    /// Builds a node with a placeholder id; `read` numbers ids at the end.
    #[must_use]
    pub(crate) fn new(kind: NodeKind, span: Span, children: Vec<Node>) -> Node {
        Node {
            id: NodeId::new(0),
            kind,
            span,
            children: children.into_boxed_slice(),
        }
    }

    /// Builds a leaf atom with a placeholder id.
    #[must_use]
    pub(crate) fn atom(atom: Atom, span: Span) -> Node {
        Node::new(NodeKind::Atom(atom), span, Vec::new())
    }

    /// Visits this node and its descendants in pre-order.
    pub fn walk<F: FnMut(&Node)>(&self, f: &mut F) {
        f(self);
        for child in self.children.iter() {
            child.walk(f);
        }
    }

    /// True when this node or a descendant is an `Error` node.
    #[must_use]
    pub fn contains_error(&self) -> bool {
        matches!(self.kind, NodeKind::Error) || self.children.iter().any(Node::contains_error)
    }

    /// The symbol name when this node is a `Sym` atom.
    #[must_use]
    pub fn sym_name(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Atom(Atom::Sym(name)) => Some(name),
            _ => None,
        }
    }
}
