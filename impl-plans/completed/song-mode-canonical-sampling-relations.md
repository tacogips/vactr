# Canonical structural sampling and selected-source relations

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Authentic clock capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose

Complete the mandatory timing relations that cannot be represented by an
invertible affine map: sampled content inherits a structural whole, while its
selected source retains a distinct original whole and issued handle. Capture
each relationship at its actual query boundary, including callable sounds.

## Dependencies and boundaries

| Dependency | Required state |
| --- | --- |
| [Runtime clock frames](../completed/song-mode-canonical-clock-frames.md) | Genuine affine/reversal fixtures and independent gates pass before release |
| [Clock transaction fixtures](../completed/song-mode-clock-transaction-fixtures.md) | Valid sampling/publication witnesses and cohesive clock-test extraction independently verified |
| [Selected-source extraction](song-mode-source-query-extraction.md) | Cohesive query boundary moved; extraction-only evidence retained before sampling changes |
| [Ordinary replay](../completed/song-mode-canonical-query-replay.md) | Verified immutable execution authority and callback reuse |
| [Canonical occupancy](song-mode-canonical-index-occupancy.md) | Original owner/address and cumulative meter preserved |

This phase supplies authentic sampling/source relationships. Immutable geometry
consumption, production pre-Reserve retention and uniform varying-seed bounds
remain mandatory subsequent work. Empty output cannot manufacture a source
membership witness. Temporary Unknown paths remain open implementation work.

## Exact eight-path Rust manifest

| Path | Deliverable | Status |
| --- | --- | --- |
| `src/pattern/eval/song_clock.rs` | Checked structural relations, single-use sampling permits, selected owner links and child-owned dispatch/barrier policy | COMPLETED |
| `src/pattern/combinators/control.rs` | Capture timing event before sampling subject/value | COMPLETED |
| `src/pattern/combinators/structure.rs` | Actual Cat/FastCat/Off maps and Grid structural sampling | COMPLETED |
| `src/pattern/combinators/region.rs` | First-structure Slice/Splice relation before subject sample | COMPLETED |
| `src/pattern/combinators/time.rs` | Segment's actual whole/part/ordinal before sample | COMPLETED |
| `src/pattern/combinators/sound.rs` | Sound's actual whole/part, then actual returned child sampling | COMPLETED |
| `src/pattern/eval/song_clock/tests.rs` | Update the genuine sampling-barrier regression to reflect newly instrumented paths; preserve full original clock/work/depth and failure witnesses | COMPLETED |
| `src/song/source/sampling.rs` | New extracted query_source/source-trace helpers, actual returned source link and genuine tests | COMPLETED |

The stage-1 eval/sample and query/dispatch hooks delegate policy to song_clock;
this phase does not require a ninth parent path. Observation/replay context
accounting also delegates to that child. The two-path source extraction plan
moves query_source and its cohesive identity helpers first. This phase changes
the extracted child, leaving the parent forwarding seam unchanged. Preserve
existing helper paths/imports and original tests. Every touched Rust file stays
below1000 lines; tests belong in the declared children. No host, resource,
dependency, Git or undeclared Rust edits.

## Private authority declarations

Names may be reconciled with the verified frame implementation before release.
These are opaque query-issued authorities, without public constructors.

```rust
pub(crate) struct CanonicalSamplingRelation {
    // Private actual point, structural whole/part, edge and parent clock.
}

pub(crate) struct SamplingPermit {
    // Private exact child/point/producer binding; consumed before recursion.
}

pub(crate) struct SelectedSourceBoundary {
    // Private original selected Part/track/selector and request relationship.
}
```

A scoped caller relation provides a single-use permit only for the actual
sampled child, point and producer edge. Consume it before child recursion.
Ordinary, nested or unrelated sampling without its own permit stays Unknown.
Do not grant a permit to arbitrary queries performed by a resolving callback.
Restore parent state on both success and Result failure. Continuous
whole=None remains None; do not fabricate a slot.

## Source-grounded boundary specification

| Caller | Actual evidence before child query |
| --- | --- |
| `control::pair_up` | Timing event v, or retained subject event e, including whole/part/anchor/producer |
| `region::query_slice` | Index event ie and exact subject edge before first-structure sample |
| `structure::query_grid` | Active mask event b before content sample |
| `time::query_segment` | Constructed whole, clipped part and generated ordinal |
| `sound::query_sound` | Cycle whole/part before traced_source; permit activated only at sampled_source after actual child resolution |

Sound's structured PParam::Pat branch queries directly; keep it distinct from
callable/unstructured sampling. The child retains its source whole/handle;
SourceSample::into_event supplies the separate Sound structural whole.

Cat knows its selected child ordinal and actual shift before recursion. FastCat
composes that map with division by the actual child count. Off captures the
evaluated transformed-branch shift; its original sibling remains identity.
Retain full copy/producer identities and exact signed maps.

query_source knows its immutable selected Part/track/selector and request
relationship before nested_part. The source whole and issued handle become
known only when each genuine row returns. Link that returned authority after
recursion, preserving original source whole and source START separately from
the inherited structural whole. A requested point cannot supply an invented
source whole. Nested owner basis reset must retain the authentic boundary
relationship rather than imply identity with the parent's clock.

Original pre-subject Index observations remain useful when subjects are empty,
but source membership requires its own actual authority. No callback-journal
duration guesses, clipped-note reconstruction or family-only identity joins.
Frame allocation, projection, traversal and copied variable identity data must
charge the original ledger before growth; immutable context sharing remains
shallow and transactional.

## Tasks

### TASK-001: Sampling authority and source extraction

**Status**: Completed
**Parallelizable**: No

- [x] Inspect verified stage-1 source and seal this exact eight-path release.
- [x] Inspect extraction-only evidence and preserve selected-source helper interfaces.
- [x] Define original boundary authority and consumed sampling permit.
- [x] Keep unrelated/nested sampling Unknown and restore state after faults.
- [x] Preserve shallow immutable context sharing and checked accounting.

### TASK-002: Genuine runtime hooks

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Capture Control and Slice timing events before sampling.
- [x] Capture Grid/Segment actual structural whole/part/branch.
- [x] Capture Sound structural context and actual returned child separately.
- [x] Instrument actual Cat/FastCat/Off shifts/count/copy identities.
- [x] Capture selected-source request then link actual returned source authority.
- [x] Lift only the dispatch barriers whose runtime hooks now prove a relation.

### TASK-003: Original execution acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-002.

- [x] Empty subjects retain Index timing without fake source membership.
- [x] Update the prior Grid Unknown fixture only where the actual new hook proves its relation; retain a genuine uninstrumented-path Unknown witness and add direct Grid relation checks.
- [x] Nested source wholes and inherited structural wholes remain distinct.
- [x] Fractional Grid/Segment and callable Sound compare genuine original outputs.
- [x] Signed Cat/Off copies preserve full branch identities and gaps.
- [x] Continuous whole=None remains None.
- [x] A callback's unrelated sample cannot consume the caller's permit.
- [x] Parent relation restores after actual failure and source recursion.
- [x] Retained reordered replay executes no original callbacks again.
- [x] Exact sufficient/one-less shared work and inherited depth remain enforced.
- [x] Independent relevant tests, native/WASM, strict lint and scoped format pass against held source.

## Completion boundary

Completing these relations does not prove resource admission. Immutable lookup,
eligibility at original source START, exact connected union per authenticated
use/configuration, owner clipping, pre-Reserve publication and uniform varying
domains remain required in the canonical occupancy and reconciliation plans.

## Progress log

### 2026-10-03 — Source-grounded sampling handoff prepared

Specialized read-only review identified the actual pre-query timing events,
Segment's location in time.rs, Sound's additional sampling edge and the
post-query source handle/whole link. Root inspected those source boundaries.
Keeping sampling/dispatch policy in the stage-1 child allows eight paths by
using sound.rs instead of eval.rs. Stage1 is still being authored; this plan
does not release Rust edits or claim relation implementation.

### 2026-10-03 — Authorized source implementation begins

Clock final004 gates and927 unchanged inputs accepted by root. Current exact baselines recorded in canonical-sampling-source-intent-0001.json. Extraction preserves query_source and source framing, while shared identity cost helpers stay in parent because expand_event uses them. Sampling follows that extraction-only receipt; original owner, work, depth and completed clock tests remain required. No Cargo, dependencies, host or Git modifications.

### 2026-10-03 — Scoped implementation and genuine fixture source held

Actual Control/Slice/Grid/Segment/Sound sampling now creates a single-use child/point/full-producer permit before the actual child query; Sound resolves its callback before permit creation. Cat/FastCat/Off record actual evaluated maps. Selected source query links original returned origins/handles and source wholes separately from structural timing, including genuinely empty returned membership. Nonlinear Sample rejects affine point/whole projection. No admission/uniform-bound consumer is enabled.

Eight new genuine sampling child fixtures cover empty Grid subjects, Cat/FastCat/signed Off, one retained inner execution under two parent sample points (including empty source), continuous None and mismatched/nested samples, Segment/callable Sound, sample/rebind exact sufficient and one-less work, source DivisionByZero with sibling restoration and uncompleted membership, and a real Sound resolver querying an original Part before returning its Slice child. The dependent note determines actual Index value; replay denies both cut and dependent reads and compares complete original output handles/values/controls/whole/producer/origin binding. Original seven clock tests, including normal-stack200Rev, remain unchanged except the permitted replacement of instrumented Grid Unknown witness with unsupported Euclid.

Source ready, not test acceptance. No author Cargo. Exact ten-path union is across extraction2, sampling8 and replay-binding2; no implementation plan exceeds eight paths. Immutable eligibility/union/admission, varying-domain proof and pre-Reserve publication remain pending.

### 2026-10-03 — First independent compilation and narrow fixture repair

Held0001 native66866 passed. Libtest87779 exited101 before executing any test: the Sound dependency fixture asserted deep_eq's Result rather than its successful bool. Fixed only that assertion to expect a successful comparison and still require true. Full log /tmp/vactr-canonical-sampling-clock-001.log SHA eaf56402b9ae86081566b74b3bde5aecc30b9d080391b64ba3597605e8704707; final receipt /tmp/vactr-canonical-sampling-final-001.json SHA b43c3a88c13ad08422b4382c8c36ef6f1a837b6979be73bc8014d2eb7d479648. All928 prior inputs were unchanged. No behavioral acceptance is claimed; production remains identical.

### 2026-10-03 — Independent0002 failures and repair intent

Native passed and clock7 passed; source8 executed3PASS5FAIL with all928 inputs unchanged. Off asserted distinct branch prefixes across cycles incorrectly; the same transformed Child1 legitimately recurs. Named resolver replaces parser-invalid multiline lambda while retaining actual PartEvents-dependent numeric result and denied replay reads. The two empty Segment observation failures remain under diagnosis: segment uses structured op, so no unstructured-index explanation is assumed. Collect must install authentic captured dependencies before observation, matching production retention. That enables an independent original raw owner context to be reused under genuine sampled source; None→Some binding currently rejects this valid context and requires authenticated insertion, preserving immutable frames/descendant boundaries and charged storage. No completion claim or author Cargo.

### 2026-10-03 — Frozen structure diagnosis and evaluated Segment input

Held0003 native passed, clock7 passed, source8 executed6PASS2FAIL. Full source log SHA a0af23d9ebb011dacdb7fcb371990d8c40210e7840589f00772c60af1dbec9b4; final003 SHA f7f98755c58dc2fe742bedeae8c149d28e92f14ae41850dbfc3a27ad00652fb5. All928 remained exact. Diagnostics show Slice's frozen index was Pure(Thunk), structured=false; its subject Pure(Fn) was also false. Segment's native op is structured once evaluated, but positional braces compile a thunk (compiler.rs352/645), and Slice's Late index mask preserves it. This is fixture construction, not a changed operator flag or inferred sampling defect. Both Segment fixtures now evaluate a local let initializer in Value position before constructing Slice from the resulting original Segment Pattern. Callable Sound is split into a separate test so its actual behavior is independently visible. Original2-segment counts, continuous None/permit checks, callback-denied replay and all production relationships remain unchanged. No author Cargo; nine source tests now declared.

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
