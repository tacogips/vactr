//! The canonical printer (6.5.4 printer table). The golden tests use it.

use std::fmt::Write as _;

use crate::reader::node::{Atom, Node, NodeKind};

/// Prints one node in canonical S-expression form.
#[must_use]
pub fn print(node: &Node) -> String {
    let mut out = String::new();
    write_node(&mut out, node);
    out
}

/// Prints every node, joined with `\n`.
#[must_use]
pub fn print_all(nodes: &[Node]) -> String {
    nodes.iter().map(print).collect::<Vec<_>>().join("\n")
}

fn write_seq(out: &mut String, open: &str, nodes: &[Node], close: char) {
    out.push_str(open);
    for (k, n) in nodes.iter().enumerate() {
        if k > 0 {
            out.push(' ');
        }
        write_node(out, n);
    }
    out.push(close);
}

fn write_node(out: &mut String, node: &Node) {
    let ch = &node.children;
    match &node.kind {
        NodeKind::Atom(a) => write_atom(out, a),
        NodeKind::Call => write_seq(out, "(", ch, ')'),
        NodeKind::List | NodeKind::Pair => write_seq(out, "[", ch, ']'),
        NodeKind::Block => write_seq(out, "{", ch, '}'),
        NodeKind::Arrow => {
            let (body, lhs) = match ch.split_last() {
                Some((body, lhs)) => (Some(body), lhs),
                None => (None, &ch[..]),
            };
            out.push_str("(-> ");
            write_seq(out, "(", lhs, ')');
            if let Some(body) = body {
                out.push(' ');
                write_node(out, body);
            }
            out.push(')');
        }
        NodeKind::Splat => write_tagged(out, "&", ch),
        NodeKind::Neg => write_tagged(out, "#neg", ch),
        NodeKind::Fallback => write_tagged(out, "#?", ch),
        NodeKind::Interp => write_tagged(out, "#interp", ch),
        NodeKind::IfChain => write_tagged(out, "#if-chain", ch),
        NodeKind::Import(decl) => {
            out.push_str("(#import ");
            write_quoted(out, &decl.path);
            if let Some(alias) = &decl.alias {
                out.push_str(" as ");
                out.push_str(alias);
            }
            if decl.open {
                out.push_str(" open");
            }
            out.push(')');
        }
        NodeKind::Error => out.push_str("(#error)"),
    }
}

fn write_tagged(out: &mut String, tag: &str, children: &[Node]) {
    out.push('(');
    out.push_str(tag);
    for n in children {
        out.push(' ');
        write_node(out, n);
    }
    out.push(')');
}

fn write_atom(out: &mut String, atom: &Atom) {
    match atom {
        Atom::Int(n) => {
            let _ = write!(out, "{n}");
        }
        Atom::Float { value, .. } => {
            let text = value.to_string();
            out.push_str(&text);
            if value.is_finite() && !text.contains(['.', 'e', 'E']) {
                out.push_str(".0");
            }
        }
        Atom::Ratio(r) => {
            let _ = write!(out, "{}/{}", r.num(), r.den());
        }
        Atom::Str(s) => write_quoted(out, s),
        Atom::Keyword(k) => {
            out.push(':');
            out.push_str(k);
        }
        Atom::Sym(s) | Atom::Builtin(s) | Atom::Path(s) | Atom::Url(s) => out.push_str(s),
        Atom::Qualified { prefix, name } => {
            out.push_str(prefix);
            out.push('.');
            out.push_str(name);
        }
        Atom::Op(op) => out.push_str(op.as_str()),
        Atom::Wildcard => out.push('_'),
        Atom::ConsoleReg(n) => {
            let _ = write!(out, "_{n}");
        }
        Atom::Nil => out.push_str("nil"),
        Atom::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
    }
}

/// A string literal in source syntax, with the escapes the lexer reads.
fn write_quoted(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            _ => out.push(c),
        }
    }
    out.push('"');
}
