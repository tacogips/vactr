# Selected-source route inventory implementation plan

**Status**: Completed
**Plan ID**: SONG-07A
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Session target**: 1–3 sessions
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails) and [Snapshot isolation](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and source evidence

Preserve every reachable selected-source Part policy in the certified copied route inventory. A transformed track may select a separate captured Part carrying private FX absent from root arrangement ancestry. SONG-07 dependency discovery reaches that Part, but FrozenPattern currently exposes only families/named_buses. Route admission cannot certify those inherited configurations from that DTO.
This is a source-grounded bounded prerequisite for SONG-08, not a new Riela acceptance. Existing SONG-06A/07 seals remain historical evidence; this repair replaces only explicitly owned source hashes after independent verification.

## Manifest

```json
{
  "planId": "SONG-07A",
  "planPath": "impl-plans/active/song-mode-source-route-inventory.md",
  "dependsOn": [
    "SONG-06A",
    "SONG-07"
  ],
  "writePaths": [
    "src/song/assets.rs",
    "src/session/song/freeze.rs",
    "src/session/song/inventory.rs",
    "src/session/song.rs",
    "src/song/snapshot.rs",
    "tests/song_assets.rs",
    "tests/song_candidate.rs",
    "impl-plans/active/song-mode-source-route-inventory.md"
  ],
  "sharedPaths": [
    "src/song/assets.rs",
    "src/session/song/freeze.rs",
    "src/session/song/inventory.rs",
    "src/session/song.rs",
    "src/song/snapshot.rs",
    "tests/song_assets.rs",
    "tests/song_candidate.rs",
    "impl-plans/active/song-mode-source-route-inventory.md"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/assets.rs",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "src/session/song/freeze.rs",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "src/session/song.rs",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "src/song/snapshot.rs",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "tests/song_assets.rs",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "tests/song_candidate.rs",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "impl-plans/active/song-mode-source-route-inventory.md",
      "intendedEdit": "Serial narrow metadata repair; preserve sealed prior behavior and owner source hashes."
    },
    {
      "path": "src/session/song/inventory.rs",
      "intendedEdit": "Own copied Inventory and selected-source policy fixtures; serial split from owned freeze/main."
    }
  ]
}
```

## Related Plans and dependencies

- **Previous / Depends On**: [SONG-06A assets](song-mode-assets.md), [SONG-07 candidate preparation](song-mode-candidate-evaluation.md)
- **Next**: [SONG-08 audio commands](song-mode-audio-contracts.md), then [SONG-09 DSP routing](song-mode-dsp-routing.md)

| Dependency | Required output | Status |
|---|---|---|
| SONG-06A | Independently sealed bounded dependency walker and closed resources | Completed |
| SONG-07 | Independently sealed candidate freeze and copied route inventory | Completed |
| SONG-08 | POD work may proceed independently; route certification awaits this repair | In Progress |

## Execution and preservation contract

Use the required rust-coding author and independent check-and-test-after-modify agent. Read exact files/dependencies immediately before edits, record immutable SHA-256 before/after intents under tmp/song-mode-riela/SONG-07A and compare current prehash before writing. Only listed paths are owned. Every touched Rust file remains below 1000 lines; move helpers into the already-owned session/song.rs before freeze.rs/snapshot.rs growth, or request an exact prior manifest amendment. No implicit additional source paths. Preserve unrelated editor/canvas/session publish changes, Cargo.lock, dependencies and prior behavior. No Git mutations, broad formatting, archive moves or shared index edits. Owner alone updates this plan; final indexes/archive belong SONG-16. Poll foreground checks to terminal; never restart only because observation times out.

## Modules and declaration contracts

| File | Deliverable | Status |
|---|---|---|
| src/song/assets.rs | Typed reachable selected-source inventory from existing bounded dependency walk | Completed |
| src/session/song/freeze.rs | Delegate copied routing inventory to owned inventory module; preserve private value freeze | Completed |
| src/session/song/inventory.rs | Copied symbolic Inventory and policy fixtures, split from freeze/main | Completed |
| src/session/song.rs | Cohesive nonrecursive metadata helpers as necessary to preserve file limits | Completed |
| src/song/snapshot.rs | Immutable selected track/family and source-Part index descriptors | Completed |
| tests/song_assets.rs | Dependency discovery and exact consumed-work/alias/no-callback preservation | Completed |
| tests/song_candidate.rs | External selected-source FX, nested symbolic policies and copied descriptor fixtures | Completed |

```rust
pub struct FrozenSelectedSource {
    pub root_part: usize,
    pub track: KwId,
    pub family: Vec<FrozenSound>,
}
pub struct FrozenPattern {
    pub id: NodeId,
    pub families: Vec<(FrozenSound, FrozenAudioRoute)>,
    pub named_buses: Vec<KwId>,
    pub sources: Vec<FrozenSelectedSource>,
}
impl SongAssetDependencies {
    pub(crate) fn selected_sources(&self) -> &[Rc<SongSource>];
}
```

Existing dependency discovery may retain private typed source references internally to copying; no Rc<Part>, mutable buffer or evaluator/registry handle enters public frozen DTOs. Preserve root_part as the actual arrangement root and reference supplemental Part nodes by checked indices. Retain selected track, full copied audio family, revisions, sequence offsets, repeat counts, edit/FX policies and source-origin cutoffs. Deduplicate by actual identity, not compact syntax hashes. Share the existing exact consumed-work admission and visited identities; do not duplicate the exhaustive dependency walker or expand repetitions. Recursive source metadata must pass default-stack depth-200 probes and reject over-limit/cyclic traversal explicitly before allocation.

## Tasks

### TASK-001: Typed bounded source dependencies
**Status**: Completed
**Parallelizable**: No
**Deliverables**: Existing dependency walker exposes selected sources without evaluation or repeat expansion.
- [x] Record fresh exact source/design/plan hashes and import contracts.
- [x] Preserve existing consumption counts, alias semantics and no-callback evidence.

### TASK-002: Copied reachable policies
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No
**Deliverables**: FrozenSelectedSource descriptors and supplemental symbolic Part roots.
- [x] External captured source FX policies are discoverable for route certification.
- [x] Preserve full identities, selected track/family and source-origin cutoffs.
- [x] Admit all descriptor/Part collections before copying with one bounded work budget.
- [x] Reuse owned files for helpers so all touched Rust files remain below 1000 lines.

### TASK-003: Behavioral proof and independent clearance
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No
**Deliverables**: Nonzero targeted fixtures, complete gate logs and sealed hashes.
- [x] Test a selected source from an external Part with FX absent from root ancestry, including nested sequence/repeat and same-family nonadjacent configurations.
- [x] Test default-stack depth200, explicit depth300 failure, shared aliases and no repeated score materialization.
- [x] Run full required gates and poll every retained foreground handle to terminal.
- [x] Independent checker verifies final source hashes and exact counts before this plan is Completed.

## Admission handoff

SONG-08 must consume the supplemental descriptors before claiming complete route configuration coverage. Nested selected-source timing can be arbitrary; unscaled Capture duration is not a certified minimum after fast/slow transforms. A conservative symbolic cardinality reservation may retain distinct generations for every potentially referenced nested source placement/configuration within its containing Capture, using checked Sequence/Repeat cardinalities without expansion. Explicitly reject excessive reservation before activation, with responsible track/family/placement; do not replace preparation admission with a later capacity failure. Full placement identity distinguishes nonadjacent same-template generations, preserving private old tails. Ordinary top-level Part placements can use certified exact boundaries/tails for reuse. Identical adjacent configuration retention remains an optional optimization. No event-density assumption or undocumented runtime fallback.

## Future verification

Use CARGO_TERM_QUIET=true for every Cargo command. For nextest also set NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1.

| Gate | Required evidence |
|---|---|
| mise exec -- cargo check --features host-native | Native integration passes |
| mise exec -- cargo check --target wasm32-unknown-unknown --no-default-features --features host-wasm | Browser core compiles |
| mise exec -- cargo clippy --all-targets --features host-native -- -D warnings | Strict joined warnings gate |
| mise exec -- cargo test --features host-native --test song_assets --test song_candidate | All asset/candidate fixtures execute with nonzero counts |
| mise exec -- cargo test --lib freeze_tests | Real normal-stack source/Part depth fixtures |
| mise exec -- cargo test --lib song::snapshot | Ownership/control/lease regressions |
| mise exec -- cargo test --lib session::tests | Legacy session compatibility |
| mise exec -- cargo test --doc song::snapshot | Snapshot privacy compile-fail |
| mise exec -- cargo nextest run --features host-native -E 'binary(song_assets) or binary(song_candidate)' | Nonzero repeated phase fixtures |
| Scoped rustfmt --check --config skip_children=true and git diff --check | Owned formatting and whitespace only |

## Completion criteria

- [x] Every listed deliverable and fixture complete; no public mutable alias introduced.
- [x] All gates pass with exact distinct/repeat counts, logs, terminal exits and no foreground processes remaining.
- [x] Independent pre/post hashes match final author seal; all touched Rust files below 1000 lines.
- [x] SONG-08 route-certification dependency satisfied; source policies include external/nested selections.
- [x] Own progress completed; archive/index reconciliation deferred to SONG-16.

## Progress Log

### Session: 2026-10-01 — source-grounded prerequisite
**Tasks Completed**: Planning only.
**Tasks In Progress**: None.
**Evidence**: SONG-08/09 read-only preflight identifies missing external selected-source policy metadata. Original isolated evaluation/freeze remains verified; full route admission awaits this repair.
**Verification**: No Rust edits or new checks in this planning step.

### Session: 2026-10-01 — cohesive source inventory split
ROOT0040 authorizes the seventh Rust path src/session/song/inventory.rs before
writing it. Move existing copied Inventory implementation and source-policy
fixtures from freeze/main, register it through owned session/song.rs, and preserve
crate-private ownership. Real normal-stack policy-depth overflow remains a
required repair; retain failed probes and verify200 success/300 typed failure
without wider stacks or lowered limits. Seven modules remain below the plan
limit; every final Rust file remains below1000 lines. No eighth path authorized.

### Session: 2026-10-01 — bounded source policy metadata implementation
**Tasks In Progress**: Typed reachable selected-source discovery and supplemental symbolic inventory implementation, normal-stack evidence and final author gates. ROOT0040 authorized cohesive inventory.rs split before its first write.
**Implementation**: Existing dependency walker records private typed references with the same visited identities and exact work accounting; copied descriptors retain selected track/full family and source Part indices. Actual arrangement root identity remains unchanged. Supplemental Parts preserve symbolic offsets/repeats/FX/cutoffs without query or repeat expansion; descriptor copying shares an aggregate checked budget.
**Evidence retained**: Native check-first21970 terminal exit0. Constructor failures source-tests-first/second and depth SIGABRT fourth/fifth are preserved. Diagnosis10945 terminal101 confirms actual supplemental Part200 completes on normal stack; remaining Rev300 failure motivated small checked unary dispatch, retaining depth256. No increased thread stack or reduced limits.
**File ownership**: Only the seven amended Rust paths and this own plan; no dependencies/Git/index/archive/unrelated edits. Final seal and independent verification remain pending.

### Session: 2026-10-01 — final author readiness
**Tasks Completed**: TASK-001 and TASK-002. TASK-003 author fixtures and gates complete; mandatory independent clearance remains pending. Header remains In Progress.
**Implementation**: Cohesive inventory module retains full source Part identity, selected track/full family, nonadjacent FX configurations, symbolic Sequence offsets/Repeat counts and source-origin cutoffs. Private dependency references never enter the public immutable DTO. Shared visited/work accounting, pre-admission, cycle rejection and original alias semantics are retained.
**Normal-stack proof**: Four direct policy fixtures pass, including actual supplemental Part200 and alternating Fast/Slow200 inventory traversal and explicit depth300 rejection under the unchanged256 policy. Historical constructor and SIGABRT logs remain preserved; final dispatch uses small checked recursive frames without increased test stacks.
**Terminal author evidence**: Session10316 exited0. Ledger `tmp/song-mode-riela/SONG-07A/author-final-results.json` records all13 gates0: native/host-wasm, asset23+candidate14, source-policy4, alias1, freeze6, snapshot8, legacy78, privacy1, strict all-target Clippy, nextest37, scoped rustfmt and diff. Total135 distinct fixtures plus37 repeated nextest fixtures. No foreground author process remains.
**Seal**: Exact seven-source SHA/line-count inventory is `tmp/song-mode-riela/SONG-07A/0007-author-source-seal.json`; maximum972 lines. Source hashes remained unchanged through final gates and match intent0006. Only this own plan changed for readiness. Independent verification is required before SONG-08 route certification or plan Completed; archive/index reconciliation stays deferred to SONG-16.

### Session: 2026-10-01 — independent completion
**Tasks Completed**: TASK-001, TASK-002 and TASK-003; SONG-07A selected-source metadata prerequisite independently verified.
**Evidence**: `/tmp/vactr-song07a-independent-002/final-results.json` records all13 gates exit0,135 distinct fixtures plus37 nextest repeats, terminal session35655 exit0, no remaining foreground processes and no review findings. All seven pre/post source hashes match author seal0007, maximum972 lines. Default-stack source-policy200 success and300 typed rejection are verified. The concurrent SONG-08 compile failure in independent run001 remains historical evidence; the successful stable retry is run002.
**Ownership**: This completion changes only the own plan under immutable doc-only intent0008. Seven sealed Rust source hashes are unchanged before/after. No archive/index/Git/dependency changes. SONG-08 may consume the verified supplemental policies; actual routing admission/playback remain later phases, and SONG-09 remains gated on independently verified SONG-08.
