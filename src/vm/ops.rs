//! The instruction set (design 8.2).
//!
//! A stack machine with one operand stack and a frame array. The kernel has
//! no loop form, so there are no back edges: `match` compiles to forward
//! tests and jumps, and iteration lives in fuel-metered natives.
//!
//! Differences from the 8.2 sketch, recorded in the ME-VM progress log:
//! `CallKw` takes a call-site index (the site holds the positional, pair
//! and splat layout, which a splat makes dynamic); `MakeList` takes a
//! list-site index (layout and `ListProv`); `DefGlobal`, `UpdGlobal`,
//! `LoadCapture`, `UpdCell`, `MakeCell`, `MakeShape`, `TestTruthy`, `TestShape`,
//! `TestKey`, `GetKey`, `TestLenMin` and `Dup` are added.

/// One instruction. Indices are into the current proto's tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// Push `consts[k]`.
    LoadConst(u16),
    /// Push local `i`.
    LoadLocal(u16),
    /// Pop into local `i`.
    StoreLocal(u16),
    /// Push captured value `i` of the running closure.
    LoadCapture(u16),
    /// Push a snapshot of `globals[g]` (a `let`): an eager read.
    LoadGlobal(u32),
    /// Push `VarRef(globals[g])` (a `var` or `fn`): late-bound.
    LoadGlobalRef(u32),
    /// Push `VarRef` of the tweak slot `globals[g]` (section 13).
    LoadTweak(u32),
    /// Pop a value and (re)define the slot of `defs[d]`; push the value.
    DefGlobal(u32),
    /// Pop a value and `upd` the top-level var `globals[g]`; push the value.
    UpdGlobal(u32),
    /// Pop a value, pop a local var cell, write the cell; push the value.
    UpdCell,
    /// Pop a value and push a fresh local var cell holding it.
    MakeCell,
    /// Build a list from `list_sites[s]`.
    MakeList(u16),
    /// Pop `n` key/value pairs (2n values) and push a dict.
    MakeDict(u16),
    /// Pop the captures of `protos[p]` and push a closure.
    MakeClosure(u16),
    /// Pop the captures of `protos[p]` and push a thunk.
    MakeThunk(u16),
    /// Pop the fields of `shapes[s]` (declared order) and push the value.
    MakeShape(u16),
    /// Call with `n` positional arguments.
    Call(u8),
    /// Call with the layout of `call_sites[s]`.
    CallKw(u16),
    /// Call a collection literal head with `n` arguments (index or key).
    CallValue(u8),
    /// Force the top of the stack: run a thunk, deref a `VarRef`.
    Force,
    /// Deref a `VarRef` on the top of the stack (a local var cell).
    Deref,
    /// Jump forward by `off` ops (relative to the next op).
    Jump(i32),
    /// Pop and set the match flag: equal to `consts[k]`.
    TestLit(u16),
    /// Pop and set the match flag: a variant whose tag is `shapes[s]`'s.
    TestVariant(u16),
    /// Pop and set the match flag: a struct of `shapes[s]`'s type.
    TestShape(u16),
    /// Pop and set the match flag: a sequence of exactly `n` items.
    TestLen(u16),
    /// Pop and set the match flag: a sequence of at least `n` items.
    TestLenMin(u16),
    /// Pop and set the match flag: a dict, struct or variant with the
    /// keyword `consts[k]` as a key.
    TestKey(u16),
    /// Pop and set the match flag: truthy.
    TestTruthy,
    /// Pop a sequence and push its items from `n` on as a list.
    SplitRest(u16),
    /// Pop a sequence, struct or variant and push item (or field) `i`.
    BindField(u16),
    /// Pop a dict, struct or variant and push the value at `consts[k]`.
    GetKey(u16),
    /// Jump forward by `off` when the match flag is clear.
    JumpIfNoMatch(i32),
    /// Fail with the code `FailCode::ALL[c]`.
    Fail(u16),
    Pop,
    Dup,
    Ret,
}
