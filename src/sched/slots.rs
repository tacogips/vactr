//! Slot identities. Placeholder until TASK-007 (design 6.5.1).

id_newtype!(
    /// A playing slot.
    SlotId(u32)
);

id_newtype!(
    /// A control id in the POD handoff (design 11.4).
    CtlId(u16)
);
