# Song browser ABI export portability implementation plan

**Status**: Completed
**Plan ID**: SONG-09PC-ABI
**Created**: 2026-10-02
**Design Reference**: [Host parity](../../design-docs/specs/design-song-mode.md#routing-and-tails)

## Intent

Native carrier fixtures compile the actual browser sender and ABI. Unconditional
unmangled `free` exports interpose native libc free and recursively deallocate
before the test inventory runs. Export raw names only on wasm32 while retaining
Rust-callable ABI functions and their complete behavior on native fixtures.

## Ownership

One Rust write path: `src/host/wasm/abi.rs`.
Plan write path: this file. All SONG-09PC and SONG-08D source/plan paths stay held.
No dependency, unsafe-body, memory-allocation, wire-format or signature changes.

## Declarations

```rust
pub extern "C" fn alloc(len: u32) -> *mut u8;
pub unsafe extern "C" fn free(ptr: *mut u8, len: u32);
pub extern "C" fn outbox_ptr() -> *const u8;
pub extern "C" fn outbox_len() -> u32;
pub extern "C" fn outbox_clear();
```

These functions keep their current Rust names, signatures and visibility. Every existing `no_mangle`
attribute becomes conditional on target_arch wasm32. This preserves actual
WebAssembly symbol names and removes native global-symbol collisions.

## Tasks

| Task | Deliverable | Status | Dependency |
| --- | --- | --- | --- |
| ABI001 | Conditional raw ABI export attributes | Completed | ROOT0212 |
| ABI002 | Independent native/browser and carrier verification | Completed | ABI001 |

## Completion criteria

- [x] Author records exact source intent and held hashes before/after changes.
- [x] All raw ABI exports retain unmangled symbols under wasm32.
- [x] Native carrier binary inventory and actual sender tests execute normally.
- [x] Native/browser compile, strict Clippy and scoped formatting pass.
- [x] Joined source hashes remain stable throughout independent verification.

## Related plans

- Parent: [Song staging carriers](song-mode-resource-staging-carriers.md).
- Downstream: [Actual resource staging](song-mode-resource-staging.md).

## Progress log

### 2026-10-02 — Native symbol collision discovered

Independent runner002 original54005 terminated101 at carrier inventory; native,
wasm32 and strict all-target Clippy passed. Zero behavioral tests ran. All32
source hashes remained unchanged. ROOT0212 verifies retained logs and authorizes
this separate one-path prerequisite. Do not hide the failure with stack tuning
or replace the actual browser sender fixture with a model.

### 2026-10-02 — ABI001 implemented and held

Immutable0001 records current source/child and parent-plan baselines before replacing all five export attributes with target_arch wasm32 cfg_attr. Rust signatures and function bodies are untouched; native fixtures still execute actual browser code. Scoped formatting only; no Cargo. Parent09PC and08D remain held. Independent joined verification is required before completion.

### 2026-10-02 — Independent scoped acceptance ROOT0215

Native and wasm32 checks and strict all-target Clippy passed; actual browser sender and18 carrier fixtures execute normally without stack changes. The unchanged wire suite passed21 and continuation engine/native suite20; scoped formatting/diff and33 hashes were independently verified. Evidence: independent003 final-results SHA `e6a008f9713d7c3344a0e09f5b08762602cdd992f6459fb8fd6596a9b8a387f8`; continuation final-results SHA `8ea49555aeec36a87ddee12ffc1ecc9c9413914cdec176180e16fbd57bb8fe49`, original90257 terminal0. Root accepts ABI/carrier scope only, with59 distinct passing cases across the combined matrices. Unrelated08D geometry failures remain pending. Immutable parent0018 records document-only completion; abi.rs remains held at6d528e21bda6e811229c8a1c00400a61f0f05bc08e199d72327a635663548c3b. No Rust/Cargo change or playback claim.
