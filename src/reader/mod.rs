//! Reader: source text to S-expression nodes (design section 6).
//!
//! `read` is the phase-2 reader of the two-phase frontend (5.7); the layout
//! in `layout.rs` groups lines into statements and `line.rs` reads each one.
//! The reader never panics: packages and the editor feed it untrusted text.

pub mod import;
pub(crate) mod layout;
pub(crate) mod lexer;
pub(crate) mod line;
pub mod node;
pub mod sexpr;
pub mod span;

pub use import::{prescan_imports, AliasEnv, ImportDecl};
pub use node::{Atom, Node, NodeKind, Op, Trivia, TriviaItem, TriviaKind};
pub use sexpr::{print, print_all};

use crate::reader::layout::group_if_chains;
use crate::reader::line::{Depth, Parser};
use crate::reader::span::{FileId, NodeId, Span};
use crate::types::diag::{DiagCode, Diagnostic};

/// The nesting cap for groups, lists, interpolation and blocks (6.5.4).
pub(crate) const MAX_NESTING: u32 = 128;

/// The result of one `read`.
#[derive(Clone, Debug)]
pub struct ReadResult {
    pub nodes: Vec<Node>,
    pub diags: Vec<Diagnostic>,
    pub trivia: Trivia,
    next_id: NodeId,
}

impl ReadResult {
    /// The first node id not used by this read.
    #[must_use]
    pub fn next_node_id(&self) -> NodeId {
        self.next_id
    }
}

/// Reads `src` as file `file`. The alias environment is cloned, and each
/// top-level `import` binds its prefix for the forms after it.
#[must_use]
pub fn read(src: &str, file: FileId, env: &AliasEnv) -> ReadResult {
    if src.len() > u32::MAX as usize - 1 {
        return ReadResult {
            nodes: Vec::new(),
            diags: vec![Diagnostic::error(
                DiagCode::SourceTooLarge,
                Span::new(file, 0, 0),
                "source is larger than 4 GiB",
            )],
            trivia: Trivia::default(),
            next_id: NodeId::new(0),
        };
    }
    let mut trivia = Trivia::default();
    let mut lines = layout::split_lines(src, file, &mut trivia);
    let stmts = layout::statements(&mut lines);
    let mut parser = Parser::new(src, file, env.clone());
    let nodes: Vec<Node> = stmts
        .iter()
        .map(|s| parser.statement(s, Depth::TOP, true))
        .collect();
    let mut nodes = group_if_chains(nodes);
    let mut next = 0u32;
    for n in &mut nodes {
        number(n, &mut next);
    }
    ReadResult {
        nodes,
        diags: parser.diags,
        trivia,
        next_id: NodeId::new(next),
    }
}

/// Assigns sequential ids in pre-order.
fn number(node: &mut Node, next: &mut u32) {
    node.id = NodeId::new(*next);
    *next = next.saturating_add(1);
    for child in node.children.iter_mut() {
        number(child, next);
    }
}

#[cfg(test)]
mod tests;
