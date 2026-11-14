# Actual owner invocation retention

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Current completion path](../../design-docs/specs/design-song-mode.md#design-and-implementation-review-current-completion-path), [Authentic clock capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose

Retain the genuine full execution key and actual rebound observations for each
owner invocation. Immutable geometry needs the current selected-source caller
relationship, including successful empty output, rather than the raw cached
inner execution's earlier clock. Current occupancy retention discards nested
owner observation copies; recover this evidence during actual execution.

## Related plans

- **Depends On**: [Sampling relations](../completed/song-mode-canonical-sampling-relations.md), [Replay binding](../completed/song-mode-sampled-replay-binding.md).
- **Next**: Bounded authenticated invocation lookup and configuration geometry.
- **Required later**: Owned view threading into SongRoutePlan, actual-query
  provenance for route events, immutable density/admission, production retention
  before Reserve, useful uniform varying-seed support and public playback/export.

## Exact seven-path Rust manifest

| Path | Deliverable |
| --- | --- |
| `src/pattern/eval/song_observation.rs` | Opaque actual invocation record and precharged collector storage |
| `src/pattern/eval/song_replay.rs` | Owning full-key issuance, original execution association and success sealing |
| `src/song/query.rs` | Thin actual pattern_rows entry/success hooks; ordinary query unchanged |
| `src/song/snapshot/occupancy.rs` | Transactional publication of actual invocation authority; cohesive test extraction before growth |
| `src/song/snapshot.rs` | Snapshot-owned lifetime/identity seam only if records cannot reside in existing retained records |
| `src/song/snapshot/occupancy/tests.rs` | Extract existing inline tests without changing module paths or assertions |
| `src/song/snapshot/occupancy/invocation_tests.rs` | Genuine owning-module acceptance fixtures |

Seven paths only. occupancy.rs currently has999 lines and must be split before
growth. Keep every touched file below1000 lines, preserve existing private helper
paths and test names, and record extraction-only evidence. No dependencies,
public API changes, host edits, evaluator accessor, Git operations or unrelated
source changes. The snapshot seam is conditional; do not add unused storage.

## Opaque declarations

Proposed names may be reconciled with existing private types before source hold.
Actual fields remain private and cannot be minted from caller-supplied identity.

```rust
pub(crate) struct OwnerInvocation;
pub(crate) struct OwnerInvocationMark;

impl QState<'_, '_> {
    pub(crate) fn begin_owner_invocation(
        &mut self,
        owner: &CanonicalOwnerFrame,
    ) -> Result<Option<OwnerInvocationMark>, Failure>;

    pub(crate) fn finish_owner_invocation(
        &mut self,
        mark: Option<OwnerInvocationMark>,
        execution: Option<Rc<OwnerCycleExecution>>,
    ) -> Result<(), Failure>;
}
```

The mark captures the real original Song, complete owner placement/offset/
duration/local cycle, seed and producer entry while QState has them. On success,
associate it with the genuine raw execution and actual rebound observation
copies from that invocation. Do not infer entry or seed from output fields,
NodeId or an observation prefix. Raw execution sharing remains unchanged.

## Tasks

### TASK-001: Cohesive extraction and original authority

**Status**: In Progress
**Parallelizable**: No

- [ ] Extract existing occupancy inline tests; retain source equivalence evidence.
- [ ] Define private invocation and scoped mark with full original execution key.
- [ ] Identify actual raw miss/hit and computational-query exclusion seams.

### TASK-002: Actual invocation capture and transaction

**Status**: In Progress
**Parallelizable**: No; depends on TASK-001.

- [ ] Capture before actual owner q; seal only after genuine success.
- [ ] Preserve actual rebound inner rows rather than substituting raw cached rows.
- [ ] Retain a sealed empty-success invocation without inventing source membership.
- [ ] Charge scans, owned copies and storage before growth under the same ledger.
- [ ] Failed runtime/work/depth batch publishes no partial invocation authority;
  earlier snapshot records remain intact.
- [ ] Ordinary and computational queries retain their original behavior.

### TASK-003: Genuine acceptance

**Status**: In Progress
**Parallelizable**: No; depends on TASK-002.

- [ ] One original inner q reused by two actual source parents has one raw
  execution and distinct authentic invocation bindings; callbacks are not reread.
- [ ] Exact full key is preserved across fractional/reordered queries, offsets
  and repeat placements; different actual seeds remain distinct.
- [ ] Empty subject and empty selected-source output retain their own evidence.
- [ ] Foreign original authority and equal static NodeId cannot mint a record.
- [ ] Complete capture/publication exact-work success and one-less/depth failure
  preserve prior publication and restore scoped state.
- [ ] Existing clock7, sampling9, occupancy19, relevant public regressions,
  native/WASM, strict lint and scoped format pass on exact held inputs.

## Completion criteria

- [ ] All actual invocation evidence is privately issued and transactionally retained.
- [ ] No changed raw execution identity, repeat-seed substitution or callback replay.
- [ ] Invocation ownership is acyclic: invocation points to raw execution/context;
  raw execution may refer only to already-sealed child invocation templates, never its own or any ancestor invocation; returned origins have no invocation backedge.
- [ ] All declared acceptance gates pass; every touched Rust file is below1000.

## Completion boundary

This phase supplies the missing genuine retained inputs for an actual immutable
geometry consumer. It does not make a cloned unused view production integration.
Later geometry must authenticate exact source/use/configuration, enforce source
START eligibility, union eligible uncut wholes and only then clip to owner.
Later route-event provenance must be issued at actual query time. Missing or
ambiguous immutable coverage fails without VM calls. Exact required executions
and permitted first-time varying-seed domains remain distinct authorities;
observed seeds cannot certify unobserved returns or justify a work-sized pool.

## Progress log

### 2026-10-03 — Source-grounded missing invocation evidence identified

Sampling checkpoint0004 passes364 distinct Rust tests, native/lint/WASM/format
and590 fresh-artifact editor tests/build. All928 input hashes match after the
frontend gates. Read-only specialist review identifies the discarded nested
bound rows and missing full invocation association. Root authorizes this
bounded prerequisite before lookup/geometry, preserving the entire production
completion path and full varying-domain requirement.

### 2026-10-03 — Implementation intent and extraction-only evidence

Original five existing baselines sealed in owner-invocation-source-intent-0001.json. Existing occupancy tests were extracted with exact dedented body equivalence before semantic changes; receipt owner-invocation-extraction-only-0001.json. Actual q entry owns original key; sealing associates successful raw execution and invocation-bound rows. Cached parent executions also retain/rebind already-sealed genuine child invocation templates because replay skips nested q scopes. Root approves this creation-ordered DAG refinement: execution holds completed descendants only, invocation refers to its own raw execution, execution never points to its own enclosing/ancestor invocation. No Cargo or production integration claim.

### 2026-10-03 — Coherent implementation and six genuine fixtures written

Actual entry marks retain the complete original q key and entry clock; successful
sealing associates the authentic raw execution and bound observation suffix.
Raw executions retain only completed child invocation templates, and cached
outer hits rebind their entry clocks and rows using the current authenticated
source relation. Empty successful invocations retain their entry boundary even
without Index observations. Publication moves scratch invocation storage only
at the existing whole-transaction commit. The conditional snapshot.rs seam is
unchanged. Exact inherited ledger checks precede owned copies and storage.

Six owning-module fixtures are written for two-parent sharing/empty membership,
zero-observation source entry clocks, cached outer replay with denied callback
reads, same/vary actual seeds and placements, full-transaction measured exact
work/one-less and depth refusal, and runtime fault/foreign original authority.
All acceptance checks remain pending independent compilation and execution; no
Cargo was executed by the author. Immutable lookup, geometry, admission and
uniform varying-domain consumers remain required later.

### 2026-10-03 — First independent compile and owning fixture repair

Held0001 native check passed. Lib-test compilation failed with eleven E0599
errors because the new fixture used a nonexistent PreparedSong.snapshot_mut
accessor; no tests executed. The owning snapshot descendant fixture now borrows
the existing private prepared.snapshot field directly, following the extracted
original fixtures. Production code and all six fixture assertions are unchanged.
Receipt0002 awaits independent execution; no author Cargo.

### 2026-10-03 — Complete fixture API correction after second compile

Held0002 native passed but lib-test compilation still failed before execution:
six snapshot_mut calls on measured/fresh/foreign variables remained, and one
SongSnapshot.query call omitted its limits argument. The previous log's claim
that all eleven calls were fixed was incorrect. Exhaustive source inspection
now finds zero snapshot_mut uses in the child, all mutable access uses the
existing private field, and the query call passes the actual required limits.
No production edits or assertion changes; execution remains pending.

### 2026-10-03 — First runtime verdict and authentic intended-owner selection

Held0003 native passed and whole occupancy ran: old19 passed, new6 had three
passes and three failures. Two-parent/empty fixtures selected any source-linked
owner, including the genuine base chord owner, rather than the intended inner
Transform. Selection now derives the inner source index from the actual latest
frozen Transform Edit, then checks its original revision, payload root, track
and duration. Cached-child matching checks the complete actual entry relation,
admitted raw depth and original raw Rc alongside owner/entry/seed, avoiding an
arbitrary first equal-semantic record. Assertions and production are unchanged;
independent execution remains necessary to distinguish any remaining defect.

### 2026-10-03 — Explicit membership equality and complete descendant bijection

Held0004 native passed but test compilation failed before execution because
SongEventOrigin has no PartialEq. The fixture now compares exact source request,
producer and parent projection, then equal-length original member Rc identities.
No production equality was introduced. Cached outer coverage separately uses
ALL genuine source-linked invocations, verifies intended-inner and grandchild
representation, and requires an equal-sized consumed-index bijection with full
owner/entry/seed/depth/raw identity and actual source entry relationship. The
first two fixtures retain intended-inner topology selection. Production remains
unchanged and acceptance is pending.

### 2026-10-03 — Parent-authorized compile-only diagnostic

Root explicitly amended the author restriction to permit quiet cargo check
--tests only. Command `CARGO_TERM_QUIET=true mise exec -- cargo check --tests`
redirected /tmp/vactr-owner-invocation-author-compile-001.log; original session
85183 terminated exit0 (chunk8c02fa). No edits occurred during the process.
All fixtures compile on the current source; no author tests, lint or WASM ran.
Independent behavior/gate acceptance remains pending.

### 2026-10-03 — Behavioral acceptance and narrow return-type lint repair

Held0005 independently passed370 distinct behavioral tests and native check.
Strict Clippy failed type_complexity on replay_owner_cycle's tuple return
(raw /tmp/vactr-owner-invocation-clippy-005.log); WASM/format had not run.
A private ReplayedOwnerCycle alias names that existing tuple without changing
execution/invocation semantics or suppressing the lint. All930 held inputs
matched, all checker handles terminated before this authorized source edit.

Authorized compile-only check --tests passed: quiet original53743 terminal0
chunk800298, /tmp/vactr-owner-invocation-author-compile-002.log. No edits while
live; no behavioral tests/Clippy/WASM by author. Held0006 final gates pending.

### 2026-10-03 — Independent final acceptance and historical audit limitation

Final006 SHA941226a5ceba47459c8e8a7b8e24818faaf7d92a7bbe506dd80a13da6a5cce1c
records370 distinct fresh behavioral passes, native/strict/WASM/seven-format
exit0, all930 inputs unchanged. Root fresh590 editor tests and build accepted
artifact/dist; receipt ROOT-editor-owner-invocation-20261003.json
SHA58d71bf9c990dae42146d051318c86e15931ac200dc06ef2b43d32167db9a52a.
Historical extraction receipt is retained, but original inline/pre-format body
was not saved and reconstructed byte equivalence is unproven. Original19 tests
passed; this is behavioral evidence, not reproduced historical byte equality.
Root owns final archive metadata; lookup/geometry/full admission remain required.
