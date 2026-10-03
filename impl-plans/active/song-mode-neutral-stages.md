# Explicit neutral song stages implementation plan

**Status**: Completed
**Plan ID**: SONG-NEUTRAL-STAGES
**Created / Last Updated**: 2026-10-02
**Session target**: 1–3 sessions
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Intent and source evidence

Materialize actual immutable bus graphs for already-admitted branch, track and
master logical stages, including a valid empty chain when no chain was declared.
The accepted restricted sum topology retains all three stages even when neutral.
The actual Engine requires private_fx, track_template and master resource references
before activation; prepare_routes currently preserves absent policy graphs as None.
This bounded control-thread bridge produces graphs a preparation owner can upload.
It does not silently change policy None to a user FX declaration or install graphs.

Source evidence: SONG-SOURCE-WHOLE/0007-neutral-stage-readonly-preflight.json.
prepare.rs285–300 chain_frames(None) already charges Room memory and max(Room*4).
Private generation, track and master slot counts also already exist. Preserve all
pricing/lifecycle/provenance/detector metadata. No components/density/prepare edits
are required just to materialize neutral definitions.

**DOCUMENT ONLY**: root must review and separately release exact current source
hashes. The clock child has completed its scoped acceptance; routing.rs is
557 lines and requires a fresh serial source release. The geometry author owns
a separate active source batch. Source-whole plan and other plans/Rust stay held.
No Cargo, dependencies, wire tags, clocks or implicit extra paths are authorized.

## Manifest

```json
{
  "planId": "SONG-NEUTRAL-STAGES",
  "planPath": "impl-plans/active/song-mode-neutral-stages.md",
  "dependsOn": [
    "SONG-08B",
    "SONG-09P",
    "SONG-09"
  ],
  "writePaths": [
    "src/song/routing.rs",
    "src/song/routing/stages.rs",
    "tests/song_neutral_stages.rs",
    "impl-plans/active/song-mode-neutral-stages.md"
  ]
}
```

This bounded prerequisite may be implemented before full SONG-08B/SONG-09
acceptance after fresh serial source release and mandatory verification. Its
completion does not clear the remaining parent geometry, DSP, host preparation
or complete playback requirements.

## Modules and exact declarations

| Path | Deliverable | Status |
|---|---|---|
| src/song/routing.rs | child declaration/public typed facade | Completed |
| src/song/routing/stages.rs | bounded borrowed-topology materialization | Completed |
| tests/song_neutral_stages.rs | true empty/explicit/admission/activation evidence | Completed |

Imports are existing Arc, BusDef, BusId, SongDetectorTarget, SongRoutePlan,
SongLimits and Failure. The helper returns one descriptor per logical target;
private descriptor graph is reused across already-admitted generations by the
future preparation owner, which supplies distinct actual lease/full-key identity.

### src/song/routing.rs facade and src/song/routing/stages.rs

```rust
#[derive(Clone, Debug)]
pub struct SongRouteBus {
    pub target: SongDetectorTarget,
    pub template: Arc<BusDef>,
}
pub fn materialize_route_buses(
    plan: &SongRoutePlan, limits: SongLimits,
) -> Result<Vec<SongRouteBus>, Failure>;
```

No graph-kind inference from numeric IDs: Branch targets are PrivateFx, Track
are Track, Master is Master when the actual preparation owner assigns leases.
Graph definition IDs are candidate-local metadata, never host IDs or instance
identity. Existing named graphs may be used by multiple descriptors; descriptors
remain distinct by the actual admitted target. Keep SongDetectorTarget unchanged.

Private helper signatures, declared before implementation:
```rust
fn neutral_graph(id: BusId) -> Arc<BusDef>;
fn next_neutral_id(next: &mut u64) -> Result<BusId, Failure>;
fn charge_stage_work(remaining: &mut u32, count: usize) -> Result<(), Failure>;
fn graph_for_branch(
    plan: &SongRoutePlan, branch: &SongBranchRoute, remaining: &mut u32,
) -> Result<Option<Arc<BusDef>>, Failure>;
```

The neutral definition uses the actual two-field BusDef constructor:
`Arc::new(BusDef { id, chain: Box::new([]) })`.

The output is borrowed-policy lookup plus immutable Arc clones/new empty graphs:
- Branch Some(effect_template): locate exactly named BusDef in plan.topology.buses;
  unknown names fail rather than becoming neutral. Branch None: actual empty chain.
- Track Some(template): retain the exact Arc and complete chain. Track None: empty.
- Master Some(template): retain exact Arc and chain. Master None: empty.

Preserve plan.branches' effect_template, track template Option, master Option,
immutable topology, source certificates, sidechains, configured source member
selection and all required capacity fields. Never append synthesized graphs to
admitted historical topology or turn them into original policy identity.

## Identity, allocation and bounds contract

Output order is plan.branches order, then plan.tracks order, then one Master.
Require unique logical Branch and Track targets under bounded inspection; fail an
invalid duplicate instead of silently dropping a descriptor. No event/Part query,
certifier, user VM, score/repeat expansion or per-note key appears in this helper.

Choose empty graph IDs by checked allocation above every actual topology BusDef ID
and track destination ID. Named graphs retain their original ID/Arc. Check the
needed neutral count and available u32 range before constructing any neutral Arc.
An all-explicit plan needs no successor ID and must not fail merely because an
existing graph uses u32::MAX. Overflow is addressed when an actual neutral ID cannot
be represented. Branch and neutral stage identity remains the typed target.

Validate limits and charge actual bounded target/output storage, graph lookup and
uniqueness work before Vec/Arc allocations or collection scans. Use one remaining
counter from limits.max_nodes throughout; no reset/refund/default quota bump.
Nested Part semantic depth is already proven by admission; this helper does not
traverse Parts and cannot introduce an artificial Part depth from flat stage rows.
Charged work includes all inspected graph names and targets, even a failing lookup.
No callback-side allocation is introduced; materialization occurs before uploads.

Every touched Rust<1000, plan<=1000, three modules and three tasks. routing.rs line
count must be rechecked after clock author completion and serial ownership handoff.
If facade additions cannot fit, stop before growth and request a prior exact path
amendment; no source restoration or arbitrary helper module is implicit.

## Genuine test declarations and expected evidence

```rust
fn neutral_stage_graphs_compile_without_changing_admission();
fn explicit_graph_arcs_and_distinct_target_ownership_are_preserved();
fn stage_materialization_has_checked_ids_and_cumulative_work();
fn materialized_neutral_stages_activate_real_native_and_arena_audio();
```

### Materialization and immutable admission

Use actual frozen candidates through evaluate_song_candidate/assets admission and
prepare_routes. Plain finite custom tone has no private/track/master declarations:
`inst tone:
	sin-osc 440
song {part [tone: {s :tone}] duration: 1}
 tail-seconds: 0 > play-song` (keep valid continuation formatting in actual source).
Use the accepted parser/native contract, no invented content native or handles.

Assert descriptor count=branches+tracks+1, exact target coverage, empty chains
compile through BusTemplate::from_def with n==0, all neutral IDs distinct and all
plan required/bytes/delay/generations, topology/source certificates and existing
None semantics unchanged. Independently verify chain_frames(None) already prices
neutral Room storage through existing public route admission fields; do not change
pricing to make tests pass.

Explicit real candidate private/track/master graphs must retain Arc addresses and
all parameter/UGen/effect chains. Shared same named FX on distinct Branch targets
remains multiple owned descriptors despite identical graph pointers. Unknown
policy names cannot degrade into empty graphs. Real mixed neutral/explicit cases
exercise ID exhaustion with exact u32::MAX boundaries and tiny/full shared fuel.

### Actual configured installation/Activate and nonzero audio

This proof is executable inside tests/song_neutral_stages.rs using existing public
Engine/stager/rings/arena codecs; no production Rig/test accessor changes needed.
Build an actual EngineConfig with measured region extents large enough for existing
admitted private delay+Room and voice memory, not an enlarged synthetic capacity
report. Example 48kHz four bus slots with actual bus_seconds96 (measured region
lengths asserted), one voice, bounded templates and ACK ring. This 96-second
example is illustrative; it is not an admission assumption. For homogeneous
initial slots, prove the per-slot extent from measured bus_frames/bus_slots with
exact divisibility and compare it against compiled graph/private-delay needs.
Derive available from
engine.song_remaining_capacities after configure_song_staging_for_transport; feed
that exact report into the genuine snapshot prepare_routes, not a guessed nominal.
Assert all required totals fit and every actual bus/voice region supports its
compiled staged graph/private delay. If this measured geometry fails, preserve the
actual failure and repair only after root authorization; no host-fit waiver.

Install the actual frozen custom-tone InstDef, plus the materialized empty Branch,
Track and Master graphs, through both NativeSongInstall and actual encoded graph
records into ByteInbox. Remap candidate target→full actual lease keys locally in
the fixture; use real reserve/begin commands and actual callback-produced
ResourceReady receipts. No fabricated Ready/LeaseReturned/Applied or manually set
slot state. ConfigureBranch references all three actual leased resources; Seal
produces real Ready, Activate at the actual engine frame produces real Applied.

Commit a real queried/routed mono event to that branch with the actual private
instrument mapping, finite onsets, constant controls and no fabricated handle.
Output before activation stays silent; after activation assert finite, genuinely
nonzero oscillator audio for Native and Arena transports and exact partition
comparison when the current accepted runtime promises it. Keep callbacks bounded;
this fixture is a single admitted song witness, not a generalized preparation
manager, broad full-host acceptance or direct snapshot seed proof.

No current root-private engine function is assumed. Existing public Engine::process,
staging configuration, build_env, ring encoders, ByteInbox and ACK consumers are
sufficient, as exercised by real tests/song_dsp.rs. Record any actual missing
public contract before expanding the manifest or weakening the proof.

## Dependencies and related plans

| Dependency | Output | Gate |
|---|---|---|
|08B admission|typed topology and real capacity pricing|existing preparation baseline|
|09P staging|actual silent graph adoption and ownership|verified provider baseline|
|09 runtime|actual optional-stage references require concrete resources|active wider phase|
|Clock child|routing.rs serial write ownership|hold before this source release|

**Previous**: [Audio contracts](song-mode-audio-contracts.md),
[Host adapters](song-mode-host-adapters.md).
**Next**: parent preparation owner consumes descriptors, source-whole/route geometry
remain separate prerequisite phases; full playback/export completion unchanged.

## Tasks

### TASK-001: Bounded typed stage materialization

**Status**: Completed
**Parallelizable**: No — facade and helper ownership.
**Deliverables**: routing.rs and stages.rs.

- [x] Fresh exact serial source release; declarations and immutable before receipts.
- [x] Exact typed target descriptors, Arc preservation and genuine empty chains.
- [x] Checked neutral IDs and preallocation cumulative work, no topology mutation.
- [x] Existing admission memory/count/pricing remains unchanged, all Rust<1000.

### TASK-002: Actual immutable/capacity fixtures

**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No — same owned test file.

- [x] Real plain/explicit/mixed candidate materialization and pointer/chain assertions.
- [x] Tiny/exact cumulative work and ID boundaries reject honestly.
- [x] Actual configured region and available capacity invariants, no fake report.

### TASK-003: Native/Arena installation and independent evidence

**Status**: Completed
**Depends On**: TASK-001, TASK-002
**Parallelizable**: No — final stable source cohort.

- [x] Actual uploads/remaps/ResourceReady/Ready/Applied from real Engine callbacks.
- [x] Pre-activation silence and genuinely nonzero post-activation event audio.
- [x] Distinct typed stage ownership and unchanged admitted logical topology.
- [x] Required independent native/browser, strict Clippy, fresh unfiltered inventories,
  actual tests and supplemental nextest; all touched format/diff and hash seals.

## Completion criteria

- [x] All three tasks proven with real terminal evidence, three modules within bounds.
- [x] Empty stages are actual valid graphs and named graphs retain original Arc/chains.
- [x] Real admission totals unchanged and fit measured physical fixture geometry.
- [x] Actual Native/Arena ownership and nonzero activation evidence independently pass.
- [x] No pricing/geometry waiver, fake receipts or implied full Song completion.

## Execution and progress

Required specialized rust-coding author and automatic independent checker after any
Rust edits. Immutable fresh numbered before/post receipts; scoped formatting only,
no dependencies, Git/index/archive or unrelated plan changes. Future Cargo through
mise uses CARGO_TERM_QUIET=true; nextest also NEXTEST_STATUS_LEVEL=fail,
NEXTEST_FAILURE_OUTPUT=immediate-final and NEXTEST_HIDE_PROGRESS_BAR=1. No Cargo or
source work until root's explicit subsequent release/all-writer gate coordination.

### 2026-10-02 — document-only Ready

Inspected real pricing(None), typed target topology, Engine activation's full-stage
reference requirement, BusTemplate empty-chain compilation and Native/Arena test
APIs. Exact three future source paths proposed; no source work performed. Baselines
observed in SONG-NEUTRAL-STAGES/0001; routing clock-owned hash is deliberately not
claimed stable. Source-whole plan stays held, full Song goal remains unfinished.

### Session: 2026-10-02 — neutral implementation readiness
Read-only receipt SONG-NEUTRAL-STAGES/0003 confirms the exact three-path
manifest is sufficient with existing constructors and public Native/Arena APIs.
Clarified serial ownership and measured per-slot fit proof. No source, Cargo
verification, parent acceptance or completed implementation is claimed.

### Session: 2026-10-02 — ROOT0356 source release
0004 records fresh exact three-Rust baseline/new absences and both allowed documents before source edits. Bounded next_neutral_id uses a u64 cursor and checked conversion so assigning final representable u32::MAX succeeds; all-explicit plans require no successor. Materialization is control-thread only, shared quota preflight before output and graph allocations. Actual measured staging/audio fixtures and mandatory checker evidence remain pending.

0005/0006 writes actual four-fixture candidate/materialization/Native-Arena suite. The Engine is configured with real10-second per-slot bus memory and measured region/report fit assertions; no synthetic capacities are supplied to route admission. Per-private-generation Room+unchanged priced delay fits an actual region, while physical nonzero delay implementation remains a separate full09 obligation (today's runtime rejects explicit delay controls). Default event controls stay genuine. Typed neutral uploads use real Reserve/ResourceReady/Ready/Applied and actual queried/routed oscillator rows. Dedicated exact quota assertions price validation, highwater and preadmitted final lookup, including extra inventory and late missing graph. No Cargo or passing evidence yet.

0007 strengthens explicit Arc ownership with a genuine two-track candidate using the same named private FX on both instrument scopes, plus real measured seven-slot Engine configuration. `Rig::with_bus_slots(bytes: bool, slots: usize) -> Self` is fixture-only actual allocation, not a synthetic report. Duplicate target/missing graph/ID boundary changes are explicitly public copied-plan hostile negatives. All actual acceptance remains pending independent execution.

### Session: 2026-10-02 — source HELD for mandatory verification
Typed materialization and four complete declared fixture bodies are written/scoped-formatted. Actual first named lookup is charged as inspected; final immutable lookup has its own conservative full-inventory charge before output allocation (no consumed scan reuse or refunds). Source sizes: routing559/stages169/public654, all below1000. Exact quota fixture covers extra graph scans, late missing-name failure and final representable ID/all-explicitMAX. Native/Arena fixture stages actual candidate graphs and parsed/routed controls through genuine callback ACKs, measures physical regions including Room plus per-generation priced delay fit, and compares differently partitioned oscillator output. These assertions have NOT been executed by the author. No Cargo/live handles; all three Rust and own plan now HELD. Real nonzero delay storage remains separate unfinished full09 work; aggregate fit is not its implementation evidence. Mandatory checker will prove or diagnose this batch; broader geometry/playback unchanged.

0010 ROOT0361 fixture-only declarations: `assert_tone_families(plan: &SongRoutePlan, tracks: usize)` verifies actual distinct Builtin(:tone)/Instrument(id) metadata and shared resolved graph, one generation per branch. Real bus totals5/8 leave4/7 slots after legacy master. Native/Arena fixture must reserve/upload/configure both admitted family branches using independent instrument/private graph keys, then send one authentic queried occurrence to resolve_route's selected branch. All-explicit ID exhaustion substitutions remain explicitly copied DTO boundaries; genuine candidate still preserves its absent policy. No production changes or pricing waiver.

0011 ROOT0361 actual neutral001 checker original6526 terminal101: native/wasm/strict Clippy0, all4 neutral fixtures failed real bus_slots admission before uploads. Raw /tmp/vactr-neutral-stages-independent-001/neutral-tests.log retained; all75 cohort inputs unchanged at failure. Source-grounded custom keyword payload retains Builtin(:tone) and Instrument(id) families resolving the same graph, hence2 branches/4 required buses for one track and4 branches/7 buses for two tracks. Repair only fixtures: actual total5/8 bus allocations (legacy master consumes1), exact dual-family/per-track assertions, all6 single-track graph resource leases genuinely reserved/uploaded and both branches configured. Real chosen event branch comes from resolve_route and matches its emitted instrument. Genuine shared explicit graph assertions select named branches on DISTINCT tracks; unselected Builtin branch's neutral None policy stays intact. All-explicitMAX is explicitly copied DTO policy substitution. Exact quota derives all branches/scans and extra graph delta5 for two named branches. Real measured Room+per-generation priced delay fit, actual Native/Arena nonzero audio and strict partition parity retained. Fixture formatting0; no author Cargo or passing claim. Production routing/helper unchanged and all source/plan HELD pending mandatory retry.

### Session: 2026-10-02 — independently Completed, scoped neutral stages
ROOT0364 root independently audited all75 unchanged inputs and raw name equality: original55288 terminal0 poll44328a,798 distinct qualified fixture names including actual neutral4; supplemental nextest originalf1b449 terminal0,4 executions. Matrix raw SHA4dbcf2af14813be7024f6c179de415d49e2d91fa562ebff0ea8813533aa115cc; nextest final SHA f7442555d6ed5ffbf72a3501f0303cf4163d0fd431f891ed1484d7c75398b697. Genuine dual-family/Arc/ID/quota/actual Native-Arena ResourceReady/Ready/Applied/nonzero strict partition parity and all own criteria independently verified. Native/wasm/strictClippy/format/diff succeeded. This child alone is Completed and remains active until archive phase16. Nonzero private delay, broader geometry, preparation owner, full finite playback and export remain unfinished; no capacity/pricing waiver or full Song completion.
