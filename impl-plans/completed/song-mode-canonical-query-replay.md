# Canonical owner execution sharing and query replay

**Status**: Completed
**Updated**: 2026-10-03
**Design Reference**: [Collector review](../../design-docs/specs/design-song-mode.md#canonical-collector-review-owner-selection-and-clocks), [Remaining occupancy direction](../../design-docs/specs/design-song-mode.md#remaining-occupancy-implementation-direction)

## Purpose

Connect retained original occupancy to ordinary song queries. Multiple Slice
addresses within the same authenticated owner execution must share its actual
callback evaluation. Observation replay alone does not meet this requirement.
This phase preserves the full song-mode scope; immutable density/configuration
admission and uniform varying-seed bounds remain subsequent required work.

## Related plans

- **Previous / Depends On**: [Canonical occupancy foundation](../active/song-mode-canonical-index-occupancy.md), TASK-001 independently verified.
- **Meter prerequisite**: [Native shared metering](../completed/song-mode-canonical-native-meter.md), including genuine transitive query accounting.
- **Next**: Canonical occupancy TASK-002 immutable consumers and pre-Reserve handoff, then TASK-003 uniform bounds.
- **Acceptance owner**: [Song reconciliation](../active/song-mode-reconciliation.md).

## Source-grounded review

Current `observe_part` discards canonical SongEvents. Ordinary snapshot query
still invokes its evaluator. The latest foundation draft now shares an owner
execution across different Slice addresses; verify that output before changing
it, rather than rebuilding another per-address journal. Retaining
final root SongEvents would mix later placement/edit context with the original
payload execution. Cache raw successful `q(pattern, local_cycle, state)` Events
inside `pattern_rows`, before onset exclusion, clipping, offset translation and
`expand_event`. Replay then executes those existing downstream stages unchanged.
Outer edits and canonical route policy retain their current order.

## Exact eight-path Rust manifest

| Path | Deliverable |
| --- | --- |
| `src/pattern/eval.rs` | Thin private replay installation; reuse the meter phase's existing observation extraction and keep parent headroom. |
| `src/pattern/eval/song_observation.rs` | Shared execution collection, original Index observations, ordered callback journal and actual cumulative accounting. |
| `src/pattern/eval/song_replay.rs` | New opaque execution key, raw owner output records and borrowed replay view; extracted QState observation/replay methods. |
| `src/song/query.rs` | Private retained query entry and owner-cycle lookup/capture at pattern_rows, preserving ordinary public wrapper. |
| `src/song/snapshot.rs` | Snapshot-owned shared execution view supplied privately to ordinary query. |
| `src/song/snapshot/occupancy.rs` | Address selection from shared executions; publish only a complete successful batch. |
| `src/song/routing/index.rs` | Authenticated address-to-execution binding, preserving original recipe authority. |
| `src/song/snapshot/occupancy/replay_tests.rs` | New genuine private candidate/freeze/query fixtures; no public record constructors or evaluator access. |

Every touched Rust source must remain below 1000 lines. Substantive replay code
belongs in the children. No dependency, host or transport edits in this phase.
Source release waits for the previous foundation hold and independent results.

The meter phase has already extracted observer helpers into song_observation;
the current eval parent is921 lines. Do not duplicate that extraction into the
new replay child. Confirm the verified baseline before adding its thin hook.

## Private interface deliverables

- `query_part_with_replay(part, span, context, replay_view)` supplies the private
  shared view; existing public `query_part` remains its compatibility wrapper.
- `QState::install_song_replay(replay_view)` propagates original authority into
  nested selected-source queries without resetting seed, work or depth.
- `QState::replay_owner_cycle(key)` returns an optional owned raw Event array
  under checked lookup/copy accounting.
- `QState::retain_owner_cycle(key, events)` charges storage before allocation
  and retains original successful events, not reconstructed expected results.

The private execution key includes original snapshot/Song and immutable payload
authority, owner revision, track/root, full placement and producer-entry trace,
actual seed context, complete owner-local cycle window and duration. Issuer and
Slice prefix select observations from that execution; they do not create a new
callback execution. Equal NodeIds, callable/argument pairs or independently
minted revisions never establish authority. No cross-placement normalization.

A fractional arrangement offset can make adjacent root canonical cycles revisit
the same owner-local cycle. The root request window is not that execution's
identity. A genuine half-cycle-offset fixture must establish one original q
execution for that owner-local cycle across both root requests, preserving the
full original placement and seed rather than normalizing unrelated placements.

Use opaque OwnerCycleExecution/ReplayView types in the new replay child. The
collector owns pending raw executions; the snapshot publishes an immutable view
only after the whole retention batch succeeds. Check actual q faults before
publishing any successful execution. Raw Events, pre-subject observations and
the original callback journal form that same transactional result. A record
must retain actual execution-depth evidence or conservatively require the
original admitted depth on replay; cached lookup cannot waive the depth limit.
Keep snapshot/index parent changes thin and delegate substantial work to the
new child, preserving their current source headroom.

Replay of a required retained execution must fail truthfully if missing, rather
than silently calling its callback again. Unrelated ordinary sources retain
their existing evaluation behavior. Lookup, key equality, copying, inherited
depth and nested work must debit the original caller's ledger. A successful
raw owner replay bypasses q, then follows the original downstream expansion and
edit path. Review nested raw-output capture so transitive callbacks inside q
are skipped together; do not invent a callback dictionary for later expansion.

## Tasks

### TASK-001: Execution authority and parent extraction

**Status**: Completed
**Parallelizable**: No

- [x] Read verified foundation outputs and seal exact eight-path source intent.
- [x] Reuse the verified QState extraction without changing existing behavior.
- [x] Define full-context opaque execution keys and checked record/view APIs.
- [x] Select multiple authenticated addresses from one original execution.

### TASK-002: Ordinary query consumption

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Capture original raw successful owner-cycle Events at the stated stage.
- [x] Supply snapshot view privately to ordinary queries and nested QState.
- [x] Replay exact context without another q/callback evaluation.
- [x] Preserve onset filtering, clipping, source expansion, identities and edits.
- [x] Charge actual lookup/copy/storage/depth work and refuse before growth.

### TASK-003: Genuine evidence and independent verification

**Status**: Completed
**Parallelizable**: No; depends on TASK-002.

- [x] Two Slice addresses share one actual owner execution and journal.
- [x] Fractional/reordered ordinary queries reuse original raw output.
- [x] Adjacent root windows at a half-cycle offset reuse one owner-local execution.
- [x] Different seeds, windows, placements and snapshots do not alias.
- [x] Empty subjects retain pre-subject Index observations without fake notes.
- [x] Nested source/edits preserve original full identities and callback reuse.
- [x] Exact sufficient/one-less cumulative work and depth boundaries are real.
- [x] Independent nonempty tests, native/WASM checks, strict lint and scoped format pass against held source hashes.

## Completion boundary

This phase completes shared execution and ordinary query reuse only. No host
admission readiness is inferred from the cache. Immutable density/configuration
consumers, composed-stage early exits, production pre-Reserve handoff and
seed-independent varying-repeat bounds remain mandatory in the linked plans.
Do not archive the reconciliation plan or complete the song-mode goal here.

## Progress log

### 2026-10-03 — Source-grounded review and bounded next phase

Specialized read-only Rust review identified the discarded output and separate
per-address execution seams, plus missing immutable consumer authority. The
eight-path follow-up targets the actual pattern_rows execution stage and uses
private fixtures to inspect original authority without expanding public APIs.
No Rust changes or Cargo commands are authorized by this planning entry; the
previous foundation is still being held for independent verification.

The foundation author subsequently identified fresh VmQuery adapters created by
native part-events inside callbacks. Its direct instruction debit does not yet
cover that nested fuel restoration. Full TASK-001 dependency acceptance requires
an actual VM/native shared-meter bridge and genuine nested budget fixture; this
query-replay plan does not waive that prerequisite.

### 2026-10-03 — Executable handoff after joined native-meter verification

Specialized source review confirms the eight-path raw-output/view handoff and
local-cycle key. The joined meter/split checkpoint passes307 unique actual
tests, native checking, strict lint and WASM with923 held inputs exact; receipt
/tmp/vactr-canonical-meter-final-001.json. Meter extraction is reused. All prior
checker processes are terminal before the next source release. Replay code and
its acceptance fixtures remain unimplemented; Ready marks the bounded handoff,
not proof of replay, immutable admission or uniform varying-seed authority.

### 2026-10-03 — Released eight-path source intent

Implement raw target-owner cycle sharing before q, with original immutable Song + owner revision/track/root/placement/offset/duration + entry producer + actual seed + local cycle. Root request windows select observations and never define execution identity. Keep whole batch publication transactional and required misses terminal before q. Meter and existing observation extraction are reused. Baselines:

- `src/pattern/eval.rs`: 921 lines SHA256 `6167d6c79af06c9cbe7112ce5d60e2981cd7aa0e41d775a7ce6ff72127509fc8`.
- `src/pattern/eval/song_observation.rs`: 211 lines SHA256 `7ff3e353d85f4f0f13215210b0372db3e786583cf7b4b24d56f409ed6059fda7`.
- `src/pattern/eval/song_replay.rs`: new declared child.
- `src/song/query.rs`: 719 lines SHA256 `a139f1946690ff16677068377fa725731864cdac74804f1813ad6c8c75e38666`.
- `src/song/snapshot.rs`: 949 lines SHA256 `7797ab5be3bdfc0d2656cef45e1a142269807a290d7bdc0f211ac6a46508ae6a`.
- `src/song/snapshot/occupancy.rs`: 790 lines SHA256 `36b7429bb4b8e6ee63dad559793f6f373e26b7267ffcd23b098c624fe3c26580`.
- `src/song/routing/index.rs`: 935 lines SHA256 `d6c5517e6a5d257c4c458071cf45930dc2f25a74af202d98a5682e13599e239b`.
- `src/song/snapshot/occupancy/replay_tests.rs`: new declared child.

### 2026-10-03 — Eight-path raw replay source held; independent gates pending

Shared immutable OwnerCycleExecution records retain actual successful raw q
Events, original pre-subject observations and ordered original callback journal.
The key binds strong original Song authority, actual owner revision/track/root,
full placement/offset/duration, entry producer, scoped seed and owner-local whole
cycle. Root request windows select coverage rather than creating executions.
Only target owner q executions are retained: dependent computational native
queries remain metered and independent of target filtering.

pattern_rows looks up before q; a genuine miss of a required owner cycle fails
before any callback, while unrelated payloads retain their original evaluation.
A successful replay follows the existing onset filtering, clipping, offset,
source expansion and edit/policy order. Q faults are checked before retention.
Snapshot publishes the immutable execution view and addressed records only after
the complete successful batch, preserving no partial-cache authority on failure.
All lookup, key, raw-event, observation, journal and view copies charge before
growth through the original shared ledger. Ordinary snapshot query retains that
same remaining work through its existing immutable descriptor copies.

The original VM tick does not record its complete peak in collector.peak_depth;
replay therefore conservatively requires original admitted max_depth and no
larger incoming entry depth. It does not treat pattern peak as a VM certificate.
Addressed observation/journal copies from an existing execution are reuse, not
fresh VM invocation or configuration births. Unique execution journals plus an
actual VM read observer establish q-once in the new fixtures.

Nine new genuine tests in song::snapshot::occupancy::replay_tests:

- half_cycle_owner_executes_once_across_root_windows_and_reordered_queries
- parallel_slice_addresses_share_original_execution_and_raw_identity
- required_missing_local_cycle_fails_before_callback_and_foreign_view_refuses
- empty_subject_retains_prefilter_index_without_ordinary_callback_replay
- varying_repeat_seed_and_full_placement_are_distinct_execution_authority
- raw_replay_keeps_nested_source_origin_and_later_delete_overwrite_order
- actual_replay_copy_budget_and_conservative_vm_depth_cannot_be_waived
- q_fault_keeps_raw_events_observations_and_journal_unpublished
- nested_selected_part_keeps_inherited_slice_authority_and_raw_inner_owner

The tests compare full FrozenSongEvent identity, source origins/timings, routes,
notes/controls and spans against ordinary original-snapshot queries before
retention. They also guard actual cut slot reads after retention, measure genuine
public-query exact/one-less copy budgets, and preserve runtime DivisionByZero
as a transactional control. Existing ten occupancy fixtures remain intact.
Written tests have not been compiled or passed yet. Scoped format/check exit0;
all eight Rust files are below1000. No author Cargo, dependency or host edits.
Immutable admission/uniform varying-seed bounds remain required next phases.

The final source hold also adds an actual two-inherited-Slice SourcePart witness: original full EventHandles/FrozenSourceOrigins and inner timing frames compare exactly before/after raw replay. Raw outer execution observations retain distinct inner owner revisions instead of relabeling them. No callback reads are permitted during reordered ordinary replay. The initial eight-fixture receipt was not checked; this final nine-fixture receipt supersedes it before independent execution.

### 2026-10-03 — First checkpoint: genuine chord fixture input repair

Independent native check79582 exits0 with only pending callable/result field
warnings. Whole occupancy56139 exits101: original ten pass, new two pass and
seven fail at their uncached baseline count assertions. The actual input
`note [60 64]` is a melodic control pattern, not simultaneous tones. Shared
program and nested SourcePart fixture now use supported `chord [:c :five]`,
which yields60/67 by the real chord-quality table. Existing expected4/2 counts
and every replay/identity/callback/budget assertion remain. Overwrite uses72
to distinguish its replacement from both dyad pitches. Only precise documented
pending-consumer allowances are added to RetainedQueryCall callable/result
fields; no module allowance or production behavior change. Scoped format/check
exit0; no author Cargo. The repaired source is held for the independent retry.


### 2026-10-03 — Authentic nested prerequisite execution repair

Independent retry native15693 exits0; occupancy2633 exits101 with18/19 passing.
The sole nested SourcePart witness reaches the actual denied `cut` read in the
earlier TransformInstrument source walk, before the outer cached q. The source
walk is semantically required even though selected original rows are removed
later. Its original q execution had not been collected. No ordinary-query or
callback assertions are relaxed.

The existing eight-path manifest now derives prerequisite permissions from the
minted owner's frozen Edit.source ancestry and actual payload.sources roots and
tracks. The metered symbolic topology walk traverses Repeat children once, checks
incoming depths before alias skips, charges scans/storage before growth, and
never derives authority from event family. Permissions apply only inside the
addressed edit's earlier source walk or an actual selected-source q; unrelated
sibling tracks/placements outside that source traversal retain their exclusion.
Actual raw executions still bind the strong original Song, revision/track/root,
full placement, offset/duration/local cycle, seed and complete producer entry.

Collection separates genuine admitted entry-depth contexts from semantic key
identity. It cannot reuse a shallow execution as a deeper execution certificate.
Ordinary replay scans all matching semantic records for sufficient actual entry
context and original admitted max_depth; an earlier shallow record cannot hide
a sufficient later record. Missing required records still fail before q, with
no ordinary recapture. Complete VM peak evidence remains unavailable, so the
conservative original admitted limit remains mandatory.

The nested fixture now distinguishes the actual outer execution from earlier
inner-source and nested-selected inner execution contexts, checks each exact
frozen root/revision/track and local geometry, and preserves full original
FrozenSongEvent/source timing equality plus zero actual callback reads across
reordered queries. The earlier unrelated runtime division-by-zero control and
journal exclusion witness remains unchanged. Static prerequisite q results with
no original VM journal or pre-filter Index observation need no raw record;
primary addressed empty outputs always retain their genuine execution.

Source implementation is held for independent compilation and whole19 evidence;
no author Cargo was run and no acceptance is inferred from this repair. Uniform
admission, density/configuration consumers and seed-independent bounds remain
required next work.


### 2026-10-03 — Genuine prerequisite graph fixture correction

Held0004 native37512 exits0; occupancy71445 exits101 with14/19 passing and all
original ten intact. Four failures are total raw execution counts, not actual
callback counts: two target cycles plus genuine base-source contexts produce six
records; one parallel target q plus base contexts produces three; nil subject
retains its target plus the earlier base q. The nested exclusion omitted its
required original base Capture owner. No callback replay comparison failed at
these assertions; downstream comparisons remain mandatory and are not claimed
passed yet.

The four fixtures now count only the exact minted target revision/track/root,
retaining their original2/1/1/2 target execution expectations. An independent
frozen topology inspection checks every additional record against complete
source ancestry payload revision/track/root/duration, complete local cycle, and
unique full owner/producer-entry/seed/entry-depth context. Genuine base Capture
journals must contain the actual `sound-kit` VarRef dereference: sound::lookup
calls QueryVm::sound_kit, and MeteredSongQuery::sound_kit routes through metered
deref and retain_call. These are real dependency records, not fresh target cut
invocations. Explicit actual VM ReadObserver checks enforce target cut reads2,
2,1 and one per varying placement, separately from journal-copy counts. The
existing cuts(view), full EventHandle/origin/control/route equality and forbidden
post-retention read checks remain. Nested source graph includes authenticated
base Capture roots as well as both actual inner depth contexts and outer target.

Only the replay fixture and its cfg(test) producer-entry accessor changed;
production behavior remains held0004. Scoped format/check0 and all eight files
below1000. Whole19 independent retry is required; no author Cargo run or claimed
replay completion. Subsequent admission and uniform bounds remain pending.


### 2026-10-03 — Independent replay348 pass; narrow strict lint repair

Held0005 native check exits0; all348 selected genuine tests pass (occupancy19,
affected library202 and public127, including both end-to-end witnesses). Strict
Clippy26131 exits101 solely for needless_lifetimes on the replay QState impl at
line127. The impl now uses anonymous lifetimes exactly as suggested; runtime
behavior and every fixture remain unchanged. WASM and final formatting gates
were not run after that lint failure. Evidence:
`/tmp/vactr-canonical-replay-final-004.json` and
`/tmp/vactr-canonical-replay-clippy-004.log`. All925 inputs matched the held source.
No author Cargo was run. The exact eight-path source is reheld for remaining
independent gates; clock/admission/uniform work is not started or claimed complete.

### 2026-10-03 — Bounded replay phase independently accepted

All348 distinct selected tests pass:19 canonical occupancy/replay,202 affected
library and127 public tests including both original end-to-end fixtures. Current
lifetime-only repair also passes the focused19 without double counting. Native,
strict all-target Clippy, pure WASM and scoped eight-file format/line gates exit0.
Checker proves all925 inputs held, with only the anonymous impl lifetime change
from the broad runtime checkpoint; root independently reconstructs its exact
previous source SHA. Receipt: /tmp/vactr-canonical-replay-final-005.json, SHA256
121ae7fe8724b4f2b8764df8835c062afc57388553ddb7d00b928b9f0d675226.
The original strict-lint101 remains recorded in earlier evidence.

Fresh WASM18e96985ad78810a8230652ea9040cc384436ca468bd0a2239a864cb517dc7bd
passes590 editor tests in78 files (20688/0) and frontend build (4046/0).
Dist WASM matches exactly. Root receipt:
tmp/song-mode-riela/ROOT-editor-canonical-replay-20261003.json.

This archives only the declared replay phase. Runtime clock capture, immutable
admission consumers, pre-Reserve retention and uniform varying-seed bounds
remain mandatory; the full song-mode goal stays active. All source/checker and
frontend processes are terminal before the next Rust phase is released.
