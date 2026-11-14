# Frozen issued song events

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Production provenance review](../../design-docs/references/song-mode/production-provenance-review-20261003.md)

## Purpose and dependencies

Freeze genuine issued query results into private envelopes without changing
public FrozenSongEvent or frozen-origin equality. Preserve every contributing
invocation and selected-source member proof for actual routing consumption.

- **Previous**: [Query-issued authority](song-mode-issued-query-authority.md).
- **Related**: [Immutable route authority](song-mode-immutable-route-authority.md).
- **Next**: [Authenticated geometry resolver](song-mode-issued-route-resolution.md), then
  [Ready and scheduler consumption](song-mode-issued-playback.md).

The preceding joined query/compatibility checkpoint is verified against held
inputs:405 Rust tests,590 frontend tests, native/strict/WASM/format/build gates.
Author source feasibility and the one-ledger interfaces below are reviewed.
Root source release uses the exact five-path intent saved before edits.

## Exact Rust manifest

| Module | Path | Status |
|---|---|---|
| Thin snapshot/PreparedSong forwarding | `src/song/snapshot.rs` | Not Started |
| Descriptor freeze extraction and issued envelopes | `src/song/snapshot/issued.rs` (new) | Not Started |
| Issued origin-freeze companion | `src/song/source.rs` | Not Started |
| Frozen source certificate sidecar | `src/song/source_uses/origin.rs` | Not Started |
| Public compatibility tests | `tests/song_issued_events.rs` (new) | Not Started |

Five paths; keep touched source files below1000 lines. Extract the existing
snapshot event freezing into its declared child before growth. Private authority
fixtures belong in that child's cfg(test) section; external tests cannot call
crate-private issuance or construct proof values.

## Private declarations

Author-confirmed owning interfaces follow. All proof fields are private;
constructors require original successful query evidence.

```rust
pub(crate) struct FrozenIssuedSongEvent {
    descriptor: FrozenSongEvent,
    invocations: Vec<Rc<InvocationSeal>>,
    source_contributions: Vec<FrozenIssuedSourceContribution>,
}
pub(crate) struct FrozenIssuedBatch {
    events: Vec<FrozenIssuedSongEvent>,
    transcript: Rc<IssuedQueryTranscript>,
}
struct FrozenIssuedSourceContribution {
    leaves: Rc<IssuedSourceLeaves>,
    augmented_origin: FrozenSourceOrigin,
    members: Vec<FrozenIssuedMemberCopy>,
}
struct FrozenIssuedMemberCopy {
    member_slot: usize,
    origin: FrozenSourceOrigin,
}
fn query_issued_rows(
    snapshot: &mut SongSnapshot, span: TimeSpan, limits: &SongLimits,
    work: &SharedIndexWork, depth: u32,
) -> Result<IssuedQueryBatch, Failure>;
fn with_copy_budget<T>(
    work: &SharedIndexWork, depth: u32,
    copy: impl FnOnce(&mut u32, u32) -> Result<T, Failure>,
) -> Result<T, Failure>;
impl SongSnapshot {
    pub(crate) fn query_issued(
        &mut self, span: TimeSpan, limits: &SongLimits,
        remaining: &mut u32, depth: u32,
    ) -> Result<FrozenIssuedBatch, Failure>;
}
impl PreparedSong {
    pub(crate) fn query_issued(
        &mut self, span: TimeSpan, limits: &SongLimits,
        remaining: &mut u32, depth: u32,
    ) -> Result<FrozenIssuedBatch, Failure>;
}
```

The child snapshot/issued.rs supplies query_issued_rows using the parent's
private original Song, evaluator and optional replay. Call the genuine existing
query::issued::query_part_issued; do not duplicate its realization algorithm.
The snapshot child can access parent-private fields, so occupancy.rs need not
be a sixth path. Existing occupancy::query_rows remains the compatibility seam. Preserve incoming depth and the same original
counter; only an actual top-level caller may explicitly supply zero depth.
It follows the exact outer/inherited origin order and carries authentic boundary,
member and full use/copy evidence. Public descriptor equality must exclude opaque
allocation identity. No scalar-equal timing or handle can mint a certificate.

## Tasks

### TASK-001: Freeze seam and private batch

**Status**: Completed
**Parallelizable**: No

- [x] Finalize signatures against the accepted live query bridge.
- [x] Extract existing descriptor freezing coherently without semantic drift.
- [x] Keep the actual immutable transcript alive for all issued envelopes.

### TASK-002: Authentic inherited origin freezing

**Status**: Completed
**Parallelizable**: No; depends on TASK-001

- [x] Freeze actual live source certificates in exact inherited-frame order.
- [x] Preserve every distinct contributing invocation through chords and union.
- [x] Charge all copying, controls and proof retention to original remaining work.
- [x] Publish only the complete successful batch; failures preserve spent work.

### TASK-003: Owning witnesses and acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-002

- [x] Genuine nested/chord and equal-handle distinct-invocation fixtures pass.
- [x] Reordered/fractional queries preserve actual seeds and source member proofs.
- [x] Foreign/swapped member and exact/one-less work failures publish no batch.
- [x] Public descriptor/equality compatibility tests pass.
- [x] Independent native/tests/lint/WASM/scoped format gates pass.

## Completion criteria

- [x] All tasks and actual owning fixtures pass against held source inputs.
- [x] No evaluator retention or proof reconstruction from public DTOs.
- [ ] Authenticated resolver and scheduler consume these envelopes in subsequent
  required phases; unused sidecars do not establish production completion.
- [ ] Pre-Reserve admission and useful varying-seed support remain required.

## Progress log

### 2026-10-03 — Source-grounded candidate handoff

Reviewer identifies snapshot.rs182–239 descriptor freezing and live copy_origin
as actual proof-loss seams. Snapshot has942 lines and requires extraction before
growth. Root corrects private-fixture placement: external integration tests cannot
access crate-private proof APIs. No source release or verification claim.

### 2026-10-03 — Live bridge and union handoff reviewed

Read-only review confirms the five-path manifest is sufficient. The snapshot
child owns the deferred adapter under one outer owning collector created from
incoming remaining, exact original Song attached, MeteredSongQuery and original settings,
optional ReplayView, unchanged incoming depth. Write remaining back on every
success or error, including settings/replay validation failures. The batch owns
only immutable transcript/evidence, never the collector or evaluator.

Freeze every authentic bag-and-augmented-origin contribution, including pairs
absent from the surviving union descriptor. Authenticate it via
transcript.source_members and actual
is_sealed_member before copying each genuine member's full outer/inherited origin
chain. member_slot is a correspondence index into that authenticated bag result,
not an authority constructor. Preserve all row invocation contributions.
Separately retain the public augmented descriptor: Slice timing metadata may
legitimately differ from the raw member sealed by its boundary while sharing
that member's authentic proof bag. Preserve both meanings and frame order.

Descriptor copy functions consume &mut u32 while transcript authentication uses
SharedIndexWork. Bridge each copy from that collector's current remaining and
debit the actual spent difference on both success and error. Do not borrow the
collector through transcript calls, reset counters, or retain a mutable ledger
in the immutable output. Charge proof vectors and retained Rc copies before
allocation. Public wrappers keep descriptor and frozen-origin equality unchanged.

Required private fixtures prove union bags missing from the surviving descriptor,
nested/chord authentic replay, inherited frame order, foreign/swapped evidence
refusal, reordered windows, and exact/one-less complete freeze failure atomicity.
External fixtures assert public compatibility. Current predecessor has a focused
member/clock identity failure under repair; this plan remains Planning and no
Rust edits are released here.

### 2026-10-03 — Post-seal Slice metadata must survive live union

Source-grounded follow-up disproves recovering all timing metadata from raw
sealed members: complete_source_boundary seals before query_slice appends timing
with Rc::make_mut. The current bag-only union loses a discarded augmented origin.
The live phase must preserve private bag-and-actual-origin pairs at expansion,
then retain distinct pairs through union. Equal bag identity alone is insufficient
for deduplication. This correction is released within the preceding declared
query/issued.rs path, not deferred to a freezer that cannot recover lost data.

FrozenIssuedSourceContribution separately freezes its actual augmented origin and
keeps authentic raw-member correspondence. Validate their original handle/whole,
frame chain and timing relationship under the original work/depth. Metadata may
be appended legitimately after sealing; field equality alone does not mint raw
membership. Preserve every genuine contributor's metadata for the subsequent
resolver, including pairs sharing a bag but differing in augmented origin.
Five-path freeze scope remains feasible after that prerequisite correction.

### 2026-10-03 — One owning collector across query and freeze finalized

Author preflight corrects the query_issued_rows signature: borrow the owning
SharedIndexWork, not remaining alone. Outer snapshot::issued::query_issued creates
one collector, attaches exact original, runs live query and transcript checks,
freezes every descriptor/contribution with the SAME work, then writes remaining
back outside its result closure on success or failure. Discarding the collector
between query and freeze would lose the original work/depth context.

with_copy_budget reads current remaining and maximum depth, checks inherited
remaining depth, releases the collector borrow, executes the existing bounded
copy, charges actual spent difference even on error, and returns the original
outcome. No collector borrow overlaps transcript authentication; no fresh budget
or retained mutable ledger in the output. Public descriptor extraction preserves
existing wrappers. Exact/one-less measures the complete owning query-and-freeze,
not an isolated descriptor copy.

Private fixtures use actual candidate/prepare/retention and the genuine
index-first slice {beat -> p} input for post-seal union metadata. Current
query/compatibility joined verification remains pending; five declared paths
are feasible, but Rust release requires prerequisite acceptance and Ready review.

### 2026-10-03 — Ready after joined predecessor acceptance

Joined query0005/compatibility0001 passes fresh405 Rust cases, native, strict
all-target lint, fresh WASM and scoped9 format. Root tests590/78files and build
pass using that exact WASM; source/cohort938 and target/deps/dist artifacts all
match after every handle is terminal. Root frontend receipt SHA256
7766920ea2a56f58dd7c49eea4329fdb1ea3e2debed3d3b20094b5caaca9f614.
Historical overwritten focused occupancy005 raw remains an explicit evidence
limitation; fresh broad52/final receipts establish current acceptance.

The five-path manifest and one owning query/authentication/copy ledger are Ready.
Implement genuine private bank/control coverage alongside nested/chord/index-first
union cases, and document exact actual fixture names before focused release.
Every touched source must stay below1000; declare any extra path/extraction
before edits. Frozen source/proof handoff is a required next stage, with actual
route resolver and scheduler consumption still outstanding full-goal work.

### 2026-10-03 — Source execution started

Exact five-path source release follows accepted405 Rust plus590 frontend/build
gates and post938 equality. Implement the declared descriptor extraction first,
then one-ledger query/authentication/source freezing. Original full five texts
are saved in frozen-issued-source-intent-0001.json. No dependencies or additional
Rust paths are authorized. Private owning fixture code will be recorded before
final source hold; no author behavioral tests.

### 2026-10-03 — Core extraction and first owning fixtures

Actual snapshot descriptor freezing is extracted into the declared child,
leaving public wrappers equivalent. New private envelopes keep immutable
transcript, all invocation seals, and actual source contribution copies with
authenticated raw member slots. One collector spans query/authentication/copy;
remaining is written back after success or failure. Origin comparison uses a
checked nonallocating field/chain work walk, including sample paths and route
family comparisons, rather than discarded deep copies.

Author compile001 original27841 terminated6de3c8/0, with pending unused-fixture
warnings before tests existed. Compile002 original16224 terminated9a24d2/101
on a public fixture Ratio64::from integer assumption; repaired with actual
Ratio64::new. No behavioral tests or gate acceptance claimed. Genuine private
fixture bodies cover bank/control direct chord, nested cached frames, fractional
index-first union postseal metadata, real empty/deleted sources, same/vary,
foreign/swapped proof, exact/one-less/depth/fault prior-view preservation.

### 2026-10-03 — Complete owning fixture inputs and work scope

Seven private fixture names are authored under snapshot::issued::tests; the
public compatibility fixture lives in the declared integration child. Nested
and empty source inputs use authentic unstructured subject callbacks with
structured Index, and retention locates the genuine frozen site plus complete
original graph trace rather than assuming empty prefix. Fractional metadata
evidence is captured from actual live origin pointers before freezing, then
every bag and augmented descriptor is matched one-to-one to its frozen sidecar.
Scalar-equal descriptors may have distinct genuine pointer/proof contributions;
no invented payload inequality is required. Whole operation exact/one-less uses
the actual PreparedSong query_issued entry, not the separate fixture comparison
allowance. Bank/control capture is genuine sample-play instrument closure.

Compatibility plan is archived after accepted405/590 gates; query plan source
tasks are verified but overall remains In Progress until actual consumers and
the genuine bank extension are accepted. Author compile003 and004 terminated0
with empty logs. Final fixture prefix audit and all-handle terminal evidence will
be captured in held0001. No behavioral tests or full feature completion claim.

### Source held0001 — compile-only acceptance pending behavior

Quiet author compile005 original19635 terminated65af8c/0 with empty log; all
author handles001–005 are terminal. Scoped five-file rustfmt/check0. Every
touched Rust file is below1000. Full940 cohort has exactly three changed
existing files and two declared additions versus accepted query938; all other
inputs are unchanged. Source held receipt captures eight actual test names and
all compile logs, including compiler-only002 failure. No behavioral tests,
Clippy or WASM by author. Independent focused and broad verification is pending.

The implemented source moves actual consumer readiness: PreparedSong and its
private snapshot owning entry return immutable issued envelopes backed by one
real successful transaction. Following resolver/playback/admission/vary support
remain mandatory; this entry is not yet invoked by production scheduling.

### Repair0002 — Nested timing preservation requires per-frame evidence

Independent focused001 ran seven private tests: six passed, cached nested source
freeze refused Type. Public/broad gates did not execute. All940 hashes matched
and every checker handle terminated; full five pre-repair texts are retained.
Source review identifies a too-strict inherited descriptor relation: immediate
frame permits appended Slice timings but inherited frames require full equality
including timing vectors. The intended relation permits genuine post-seal append
at each original frame, preserving original prefix and all other fields.

Correction keeps inherited length/order, exact handle/issued/sourcepart/whole,
instrument and entry, and exact raw timing prefixes at every frame. A separately
charged live-chain comparison requires exact authentic bag Rc correspondence at
each frame plus original route/commit equality and inherited depth. Distinct
static refusal messages now identify proof-bag, route, chain-length or descriptor
issues if a different actual mismatch remains. Existing public/foreign fixtures are unchanged; the nested fixture now adds
actual depth evidence before the same owning freeze operation; no authority is inferred from descriptor equality.
The code defect is source-grounded; whether it explains the actual focused
refusal still requires independent execution of this held repair.

Derived depth refinement: copy_origin computes one actual authority_depth from
whole inherited chain and timing/producer counts, then stamps every copied
timing. A genuine outer append therefore changes the certified admission depth
of unchanged inherited timing identities. Public FrozenSliceTiming PartialEq
remains unchanged. The private preservation comparator compares every issued
timing identity field exactly, while requiring independently admitted augmented
depth >= raw depth; both actual copied certificates remain stored unchanged.

The existing nested fixture now directly captures original live pair/member
copies before freezing, checks all original fields/timing identity, and requires
a real inherited certificate change with both depths inside the admitted limit.
Actual augmented/raw depth, issuer and subject handles are printed by that
genuine witness for independent diagnosis. This is not a claimed runtime result.
Author compile006 original81661 terminated4173fb/0; no tests run by author.

### Source held0002 — nested preservation repair, runtime confirmation pending

Author compile007 original44671 terminated92380b/0 with an empty log. All
author handles001–007 are terminal. Scoped five-file format/check passed; lines
are snapshot905, issued777, source972, origin375 and public fixture31. Only
source.rs, origin.rs and snapshot/issued.rs differ from frozen held0001; all
other full940 inputs remain unchanged. The genuine nested fixture compares
actual independently copied inherited certificates and requires a changed
depth before freezing the same issued rows under the same work collector.
No behavioral tests, Clippy or WASM were run by the author. Independent focused
verification must determine whether this exact timing/depth relation resolves
the observed rejection; no actual certificate values are claimed yet.

### Accepted frozen002 scoped source gates; production consumers pending

Focused002 actual8 passed (original36723/10573d/0), with four actual inherited
7/6 admission certificates across tones0/1. This verifies the precise derived
depth mismatch and authentic nested preservation. Broad002 fresh413 distinct
Rust tests/native/strict/WASM/five-file format passed, original62164/f5a524/0;
full940 unchanged. Final /tmp/vactr-frozen-events-broad-002-final.json SHA
cbd6667ec5647d4a39fad8f6b416edb176818ff3aa111acc8cd650d7b9388ede.
Root fresh590 frontend tests and build passed; all source/artifact checks exact,
root receipt cd8cef3de20b34e2bbc0b7007da65079cfd7af06970c3097032149b7d79ec4f4.
All three implementation tasks are verified; overall remains In Progress until
the explicit resolver/scheduler consumer requirement is fulfilled. No production
admission or varying-seed uniform support is inferred from these scoped gates.
