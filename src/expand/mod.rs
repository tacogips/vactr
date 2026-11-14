//! Expander: reader nodes to kernel forms (design 6.4, 6.5.5).
//!
//! `expand` is a pure `Node -> Node` pass over one top-level form. It removes
//! all sugar (`if`/`elif`/`else`, binding `if`, `for`, `?`, `-x`, string
//! interpolation) and validates the kernel shapes. Like the reader, it never
//! panics on untrusted input.

mod expander;
pub mod kernel;
mod sugar;

pub use kernel::is_kernel;

use crate::reader::node::Node;
use crate::reader::span::NodeId;
use crate::types::diag::{DiagCode, Diagnostic};

/// Expansion state shared by the forms of one read: the next free node id.
#[derive(Debug)]
pub struct ExpandCx {
    next_id: u32,
}

impl ExpandCx {
    /// Starts numbering synthesized nodes at `first_free`, normally
    /// `ReadResult::next_node_id()`.
    #[must_use]
    pub fn new(first_free: NodeId) -> Self {
        Self {
            next_id: first_free.get(),
        }
    }

    /// A fresh id for a synthesized node.
    pub(crate) fn fresh(&mut self) -> NodeId {
        let id = NodeId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

/// Expands one top-level form. The first diagnostic ends expansion of the
/// form. A form that contains an `Error` node returns `read-error-present`,
/// which callers do not show because the reader already reported the error.
pub fn expand(n: &Node, cx: &mut ExpandCx) -> Result<Node, Diagnostic> {
    if n.contains_error() {
        return Err(Diagnostic::error(
            DiagCode::ReadErrorPresent,
            n.span,
            "the form has a reader error",
        ));
    }
    expander::form(n, cx)
}

#[cfg(test)]
mod tests;
