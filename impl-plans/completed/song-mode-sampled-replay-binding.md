# Sampled source replay binding

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Authentic clock capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose

Bind copied original owner-local observations to the actual current selected
source invocation. Raw execution identity deliberately excludes its caller's
sampling relation; copying the old parent relation would retain stale evidence.

## Related plans

- **Depends On**: [Sampling relations](song-mode-canonical-sampling-relations.md), actual source boundary and child-owned binding policy.
- **Next**: Joined independent sampling/replay verification and immutable consumers.

## Exact two-path manifest

| Path | Deliverable |
| --- | --- |
| `src/pattern/eval/song_replay.rs` | Thin authenticated observation-copy hook invoking current binding policy |
| `src/pattern/eval/song_clock.rs` | Child-owned checked binding operation preserving original inner affine/reversal owner basis |

No change to original owner-cycle/seed/entry identity, public APIs or dependencies.
Do not add the caller relation to raw execution identity merely to evade reuse.
Do not overwrite original issuer whole/START or invent returned source membership.
Charge lookup/copy/new relationship storage against the incoming shared ledger.
Keep each file below1000 lines.

## Tasks

### TASK-001: Actual invocation binding

**Status**: Completed
**Parallelizable**: No

- [x] Inspect current copy_execution and the actual active selected-source boundary.
- [x] Preserve original stored owner context; bind a copy to the current invocation.
- [x] Reject foreign/missing boundary evidence without manufacturing parent identity.
- [x] Preserve scoped restoration and charged shallow immutable sharing.

### TASK-002: Genuine acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Reuse one genuine inner execution from two distinct parent samples without stale relations.
- [x] Require identical original inner identities and distinct actual parent bindings.
- [x] Deny repeat callback reads; verify genuine returned source whole/handle and empty-source behavior.
- [x] Exact sufficient/one-less work and failure restoration pass.
- [x] Joined sampling/replay tests, native/WASM, strict lint and format pass on held inputs.

Fixtures live in the sampling plan's declared test children. This companion
authorizes the additional replay owning-module hook. Its gates can share the
sampling phase's independent run, with the combined exact path union recorded.

## Completion boundary

This binding is prerequisite timing evidence. It does not implement immutable
admission, varying-domain support or production retention before Reserve.

## Progress log

### 2026-10-03 — Actual replay-copy gap found during implementation

Author source inspection finds copy_execution copies clock context unchanged,
although the current source invocation has a different parent sample/producer.
Root explicitly authorizes this bounded companion while the source is editable
and no checker command is live. Implementation remains unverified.

### 2026-10-03 — Binding implementation intent

Store the genuine original q-entry clock beside each execution without changing its semantic key. At observed copy, compare exact owner basis and selected immutable source descriptor, then rebind only that entry boundary through descendant immutable contexts. Keep cached originals untouched and preserve original child query/returned membership within the same raw execution. Actual top boundary remains pending until its own source rows return. Charge original ledger before each context/link copy and inherit actual copy entry depth. No Cargo.

### 2026-10-03 — Owning replay hook and fixture source ready

copy_execution retains genuine q-entry context and invokes the child-owned rebind policy for observed copies. Original raw key/cache contexts stay unchanged. Rebinding authenticates the original immutable selected source and owner, replaces only the actual entry boundary, preserves nested requests/producer/original returned memberships and uses pointer identity only for own-copy change detection. Actual source parent/returned evidence is not inferred from output notes. Source selector comparisons and new context/link copies are precharged against the inherited ledger. The two-parent/empty source test compares original inner identities and counts genuine executions independently of copied observations; the measured exact/one-less work test includes sampling/rebinding allocations. Independent behavioral acceptance remains pending.

### 2026-10-03 — Authentic independent-owner insertion repair source ready

Independent0002 source8 had3PASS5FAIL; native and clock7 passed. A real latent None→Some path was identified after the fixture omitted authentic dependency preparation: prior source q may be retained independently, then reused under a genuine selected-source boundary. Insertion now validates the actual immutable source Part's exact revision/track/payload root/duration and matching full owner context, retains Unknown barriers, original frame arrays and completed descendant membership, and propagates only the actual source parent boundary. Storage/topology comparisons debit the original shared ledger before growth. Existing Some→Some rebind preserves its strict original selected-source descriptor check. Independent tests remain pending.

### 2026-10-03 — Bounded phase accepted

Root accepts held0004 after364 fresh distinct Rust tests (clock7, sampling9,
occupancy19, affected202, public127), native compilation, strict all-target
Clippy, pure WASM and eleven-file formatting/line checks. All928 inputs match
and all checker processes are terminal. Receipt:
/tmp/vactr-canonical-sampling-final-004.json, SHA256
e7fd38dee8e054a9aedbadf67e2b0ddeee51ca2342030ca4013df75e30b0de38.
Extraction-only stored source evidence was checked before acceptance.

Fresh WASM b24694ea0994b0d2d2e867f33f41959b682fbc124a8713d6dfe9f828fa78f2f1
passes590 editor tests in78 files and frontend build; dist matches. Root test
37866/fd5b50/0 and build43380/ecb391/0 are terminal. Root independently rechecks
all928 source hashes after frontend gates. Receipt:
tmp/song-mode-riela/ROOT-editor-canonical-sampling-20261003.json.
Historical compiler/fixture failures remain recorded. Full song mode remains
in progress: actual invocation retention, immutable geometry/admission,
production pre-Reserve integration and useful uniform varying-seed support
are mandatory subsequent phases.
