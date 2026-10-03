# Issued invocation compatibility witnesses

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers and authentic capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Align the original cached invocation witness with genuine fresh query issuance.
Preserve shared raw execution, complete descriptors and original clock geometry;
fresh boundary/member/proof allocations are required by the accepted live design.

- **Previous**: [Query authority](../active/song-mode-issued-query-authority.md), implemented
  and focused8 accepted, broad occupancy51/52 exposes this legacy assertion.
- **Next**: Complete fresh query-authority gates, then
  [frozen handoff](../active/song-mode-frozen-issued-events.md).
- **Related**: [Reconciliation](../active/song-mode-reconciliation.md).

This companion explicitly declares the additional fixture path before edits.
It does not expand the preceding eight-module plan. No production query or
clock mapping semantics may change as a shortcut for a failing witness.

## Exact Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/song/snapshot/occupancy/invocation_tests.rs` | Genuine cached-child correspondence witness | Not Started |
| `src/pattern/eval/song_clock.rs` | Narrow test-only exact owner/frame basis comparison | Not Started |

The clock parent is988 lines; keep it strictly below1000. The comparator is
test-only and cannot issue proof values or replace production authentication.
If another path or extraction is necessary, declare it before edits.

## Private signatures

```rust
#[cfg(test)]
impl CanonicalClockProjection {
    pub(crate) fn same_frame_basis(&self, other: &Self) -> bool;
}
fn same_source_entry(a: &OwnerInvocation, b: &OwnerInvocation, comparison_remaining: &mut u32) -> bool;
```

Known clock owner/frame values must compare exactly. Unknown/permit-only
contexts must not be accepted as equivalent evidence. The fixture then follows
each genuine source request, producer and parent clock recursively, including
actual empty returned sets. Compare complete frozen raw-member descriptors
and route/commit metadata, preserving inherited origin order. Do not infer
membership from descriptor equality: each member must remain the exact member
in its actual sealed boundary. Assert fresh boundaries/member bags where
issuance actually rebinds them; retain original raw execution Rc identity.

Keep owner, entry, seed, depth, observation count and full consumed-index
bijection assertions. The accepted query8 witness separately proves that the
fresh transcript and actual invocation clock share exact mapped boundary/member
Rcs. Old/new boundary pointer equality contradicts fresh issuance and must not
be substituted for these stronger owning relationships.

## Tasks

### TASK-001: Owning compatibility witness

**Status**: Completed
**Parallelizable**: No

- [x] Add the narrow test-only exact owner/frame comparator within line cap.
- [x] Preserve the original genuine fixture input and all key/raw-execution checks.
- [x] Replace stale boundary/member identity assumptions with the complete
  descriptor, recursive geometry and authentic fresh-member relationships above.
- [x] Preserve callback denial, unchanged execution inventory and bijection.

### TASK-002: Independent acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-001

- [x] Author quiet compile-only and scoped formatting complete, all handles terminal.
- [x] Full original texts and938-input held hashes identify both plan manifests.
- [x] Fresh focused occupancy52 plus issued8 pass with actual name inventory.
- [x] Fresh405 distinct regression cases, native, strict lint, WASM and scoped
  formatting gates pass against unchanged held inputs.
- [x] Fresh frontend tests/build use that exact WASM artifact after checker terminal.

## Completion criteria

- [x] All tasks and independent gates pass, with prior failure evidence preserved.
- [x] No old pointer return, disabled issuance, fake source evidence or weakened
  membership check is introduced to satisfy the fixture.
- [x] Frozen handoff, actual resolver/playback/admission/varying-seed support and
  remaining clock hooks stay required for the full song-mode goal.

## Progress log

### 2026-10-03 — Source-grounded compatibility repair declared

Broad query0004 native succeeds; occupancy51PASS/1FAIL stops remaining gates.
Original fixture compares parent projection equality, whose source boundaries
compare allocation identity, and old/new returned-member Rc identity. Genuine
issued replay intentionally replaces these allocations while sharing the raw
execution. Author confirms the test-only two-path repair is feasible; root
declares this bounded companion before releasing changes. No edits yet.

### 2026-10-03 — Owning fixture correction implemented

Added test-only exact Known owner/frame comparison; Unknown/Permit contexts
return false. The original cached-child fixture retains all input, keys, shared
raw execution/depth, observation counts, callback denial and consumed bijection.
It compares source request/producer/parent frame chains recursively and complete
frozen member descriptors plus route/commit. Both old and rebound source chains
are validated through the existing retained_sources/member owning API, requiring
the returned reference to be the exact member Rc in the actual sealed boundary.
Rebound boundaries, raw-member origins and their proof bags must be fresh.

Descriptor/projection comparison uses one separate configured1M test allowance
shared across all candidate comparisons and membership checks, without resetting
original capture work or changing production limits. Independent acceptance
remains pending. Only these two declared paths change versus query0004.

### Accepted joined005 independent gates

Fresh focused60 all pass; fresh broad405 unique pass, native/strict all-targets
lint/WASM/union9 scopedformat0. All938 held hashes match after terminal. Checker
final SHA1bcf4b424f6b99344e883c047570585eb92c4d67d3c84f242d02cda3861145e0;
root590 frontend78files/build0 and target/deps/dist SHA8284bb661be6b80478581fb9975cbf1bcf0813d129dc45009e6c7f08c01f0995 match;
rootreceipt7766920ea2a56f58dd7c49eea4329fdb1ea3e2debed3d3b20094b5caaca9f614.
Original fixture graph/key/raw/depth/denied-read/bijection scope is preserved.
Compatibility work is complete; the full feature still requires the following
frozen/resolver/playback/admission/varying phases. Historical focused log
overwrite is not claimed reproducible; fresh broad acceptance is preserved.
