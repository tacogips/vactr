//! Visual chain nodes. Placeholder until TASK-006 (design 6.5.1).

/// A visual chain node (shell until TASK-006).
#[derive(Debug)]
pub struct TexNode {
    _private: (),
}

id_newtype!(
    /// A visual output.
    OutId(u32)
);
