//! Namespace identities. Placeholder until TASK-005 (design 6.5.1).

id_newtype!(
    /// The generation of a top-level form, bumped on each re-evaluation.
    FormGen(u64)
);

/// A late-bound var slot reference (shell until TASK-005).
///
/// Derives `Clone` in addition to `Debug` because `Value` derives `Clone`
/// and holds this type by value (design 5.1).
#[derive(Clone, Debug)]
pub struct VarSlotRef {
    _private: (),
}
