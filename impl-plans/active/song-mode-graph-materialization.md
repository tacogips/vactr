# Song graph materialization implementation plan

**Plan ID**: SONG-GRAPH-MATERIALIZATION
**Status**: In Progress
**Design Reference**: [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and ownership

Separate actual Native graph compilation from queue submission so a preparation owner can retain one compiled allocation through bounded queue refusal, retry and cancellation. Compilation uses the real constructed Native engine environment. Browser keeps its existing atomic encoded graph upload path.

ROOT0365 serial source release supersedes the earlier Ready-only hold for exactly the four declared Rust paths. All source and plans are now held for root-coordinated mandatory verification; no author Cargo is permitted. No general host framework, new codec/tag, capacity reconstruction, legacy graph behavior change or playback completion is included.

## References and dependencies

- `song-mode-host-command-submission.md`: checked command refusal and exact upload ownership.
- `song-mode-host-clock.md` and accepted actual capacity request/receipt semantics: separate time/capacity authorities.
- `song-mode-resource-staging.md`: actual ResourceReady, cancellation and garbage handoff.
- `song-mode-graph-resource-closure.md`: immutable fixed-resource provenance prerequisite.
- `tmp/song-mode-riela/SONG-HOST-PREPARATION/0012-native-materialization-readonly-preflight.md`: actual private BuildEnv and compilation/refusal evidence.
- `0013-fixed-pcm-admission-addendum.md`: fixed graph PCM still needs union/dedup demand accounting.

## Exact future Rust manifest

| File | Observed lines | Deliverable |
|---|---:|---|
| `src/host/caps.rs` | 438 | Additive default materialization method |
| `src/host/native/audio.rs` | 941 | Compact Native AudioHost delegate |
| `src/host/native/audio/song.rs` | 780 | Actual-env compilation helper and genuine private memory tests |
| `tests/song_graph_materialization.rs` | 378 | Actual Native/Browser ownership and Engine readiness witnesses |

Every touched Rust stays below 1000 lines. No unowned adapter, codec, DSP or existing fixture edits. Native parent has limited headroom; if compact delegation does not fit, stop for a prior cohesive split/manifest amendment. Child private tests can call existing private NativeAudioHost::pair to construct exact memory geometry without adding a test-only public API.

## Source-grounded contract

NativeAudioHost owns private `env: BuildEnv`, obtained from `Engine::build_env()` immediately after construction. It includes actual sample rate, capability set and actual first voice memory allocation. Public scalar capacity reports cannot reconstruct it.

Current child `song_graph` compiles Template/BusTemplate and submits NativeSongInstall. Its map_err drops the refused install off-thread, so retrying recompiles although the borrowed GraphHandle remains owned. Current `submit_song_native` already returns the exact install on refusal. Add a pure compilation boundary; do not change that existing submission ownership contract.

Wasm `song_graph` encodes the complete graph record, then actual push_record accepts it wholly or refuses it without partial bytes. The borrowed graph Arc remains owner-held. Accepted upload is not ResourceReady.

## Exact declarations

### Capability default

```rust
pub trait AudioHost {
    fn materialize_song_native(
        &self,
        lease: crate::song::routing::SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<Option<crate::dsp::ring::NativeSongInstall>, Failure>;
}
```

Add this method with an actual default `Ok(None)` in implementation. None means the host uses the existing borrowed graph upload strategy; it does not assert that an unsupported host can submit or stage the graph. Its existing submit_song_graph error remains authoritative. No native payload is sent by materialization.

### Native actual environment helper and delegate

```rust
impl NativeAudioHost {
    pub(super) fn materialize_song_graph(
        &self,
        lease: crate::song::routing::SongLeaseKey,
        graph: &GraphHandle,
    ) -> Result<crate::dsp::ring::NativeSongInstall, crate::vm::fail::Failure>;
}
// Existing AudioHost impl in native/audio.rs overrides the trait method,
// delegates to the child helper, and returns Some only on success.
```

Inst GraphHandle requires Instrument kind; Bus requires PrivateFx or Track; Master requires Master. Control/Analysis/Sample keys are invalid for graph compilation. Preserve complete epoch/resource ID/generation in the produced install, including legal fullwidth values; no implicit ID coercion or lease substitution. Validate the resulting NativeSongInstall using its existing exact payload/kind rules. Materialization cannot prove that a lease has been reserved or is fresh in the callback; actual Engine staging remains that authority.

Inst compilation uses `Template::from_inst(def, &self.env)`. Bus/master uses actual `BusTemplate::from_def`. Compiled payload retains actual graph IDs, parameters, resource/control/analyzer sites and ownership; private declaration/resource rewriting belongs to the preparation owner before compilation. Graph/compile errors are Failure, never None and never fallback to browser or legacy graph install.

The helper allocates only on the control thread. It sends no commands, dirties no capacity cache, changes no graph state and consumes no acknowledgments. Existing submit_song_graph behavior remains compatible; it may share this helper internally without double compilation/enqueue. Existing borrowed method's refusal Failure does not become proof that Native allocation ownership was retained.

## Consuming owner protocol

1. Materialize once after private graph/resource/bank rewrite and actual host admission.
2. Some install is retained by the owner. Submit with existing submit_song_native; store the exact returned install on refusal and retry it unchanged. No recompilation between retries.
3. None retains the original GraphHandle and uses existing submit_song_graph. A failed push leaves no partial record; graph ownership remains intact.
4. Actual ResourceReady validates full key; complete resources plus actual Seal Ready permits readiness. Only actual Applied acknowledgment permits publication.
5. Unsent compiled allocations are dropped on the control thread during cancellation. Accepted owners remain callback-owned until existing actual garbage/lease handoff, with exact Box/Arc identity.

Fixed graph IR/table PCM provenance is not yet demand admission. The later owner must union event samples and fixed-site assets, deduplicate captured identities, charge actual frames/bytes/per-slot extents/native PCM limits, and retain dependency leases. Controls copied into distinct banks and analyzer mutable stage banks require explicit multiplicity accounting. This four-path child changes no routing demand or pricing and makes no full host-fit claim.

## Tasks

### TASK-001: Additive actual-env compilation seam

**Status**: Completed
**Parallelizable**: No
**Deliverables**: caps.rs, Native parent/child

- [x] Exact default None and Native Some semantics, strict kind/payload validation.
- [x] Actual self.env compilation; compact sibling delegate, all files below1000.
- [x] Pure materialization with no enqueue/ACK/cache/state side effects.
- [x] Compilation errors never downgrade to None; legacy graph submission unchanged.

### TASK-002: Real allocation, memory and protocol witnesses

**Status**: Completed
**Parallelizable**: No (depends on TASK-001)
**Deliverables**: Native child private tests and public new test target

```rust
fn native_materialization_uses_actual_allocated_voice_memory();
fn native_materialization_preserves_instrument_and_bus_boxes_under_pressure();
fn materialization_rejects_wrong_kind_without_enqueue_or_fallback();
fn browser_materialization_uses_atomic_graph_upload_and_real_resource_ready();
fn materialized_graph_cancellation_returns_exact_owner_once();
fn native_materialization_clock_rejects_actual_fractional_rate();
fn unsent_materialized_owner_is_dropped_on_control_thread_without_submission();
```

- [x] Native private test uses actual `pair(EngineConfig, AtomicCells)` at unusual valid sample rate, reads actual Engine allocation, and compares real helper output with that authority. Configure a fixed-memory graph that fits exactly and another actual Engine one float short; short helper fails rather than using nominal capability memory. No fabricated BuildEnv and no direct fake self.env write.
- [x] Actual Template instrument Box and BusTemplate Box addresses/full leases survive at least two full-ring refusals, then the same allocation transfers after drain. Retry uses the returned install; helper is not invoked again.
- [x] Real configured Engine consumes the accepted upload and emits actual ResourceReady. Retire/cancel returns exact Native owner pointers through garbage, no duplicate ownership or early resource reuse.
- [x] ROOT0367 exact Native direct-clock guard matches actual Engine integral-rate requirement; real fractional32768.5 pair refuses purely and callback rejects same request, integral32768/full-u64 clock and legacy now preserved.
- [x] Fullwidth legal epoch/resource/generation and graph option payloads retain exact bytes/POD identity; wrong kind and actual graph compile error consume neither queue nor ACKs.
- [x] Actual WasmAudioHost default strategy uses borrowed GraphHandle with real encode→ByteInbox→configured Arena Engine; actual ResourceReady is produced. Full ABI ring refusal contains no partial graph record, and caller graph Arc remains owned.
- [x] Cancellation before submission drops sole retained allocation off callback; cancellation after acceptance uses actual full-key safe handoff. A configured process allocation/deallocation probe remains zero while owned payloads are returned, not destroyed by the callback.
- [x] Old sender command, graph, sample and staging regression cohorts remain unmodified and pass.

### TASK-003: Independent gates and owner handoff

**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)

- [x] All author paths/plans held with immutable before/post hashes before joined checker.
- [x] Native/browser checks, strict Clippy, exact format/diff, fresh private/public inventories, unfiltered fixtures, sender/staging regressions and supplemental nextest independently pass.
- [ ] Preparation owner receives materialized ownership contract; actual readiness and remaining geometry are not declared complete by this seam.

## Module status

| Module | Status | Tests |
|---|---|---|
| Default capability | Completed | ROOT0372 independently verified default None |
| Native compiler/delegate | Completed | ROOT0372 actual allocation/rate/strict compilation |
| Real memory/Box/Arena witnesses | Completed | ROOT0372 exact 96/95, retained pointers, actual readiness/return and callback allocation0 |

## Completion criteria

- [ ] All three tasks and all real ownership/error/memory criteria verified.
- [x] Four-path manifest and file bounds preserved; no dependencies, tags or unowned source changes.
- [x] Actual compiled allocation is stable across refusal/retry/retirement; Browser whole-record semantics preserved.
- [ ] Parent consumes proven child contract without claiming complete finite playback/export or unpriced PCM fit.


## Progress log

### 2026-10-02: Document-only Ready handoff

Inspected actual env construction, private pair availability, child compilation/drop-on-refusal and Browser record admission. Four-path plan includes the real Native parent trait impl. No Rust, existing held cohort plans, Cargo, index or dependency changes. Await root review and separate serial source release.

0001 ROOT0365 implementation begins within exact four Rust paths. Pure materialize_song_graph owns actual-env compilation; existing borrowed song_graph delegates and preserves its prior refusal behavior. Default trait method returns None without submission. Genuine memory/ownership/Browser fixtures and mandatory checker remain pending.

0003 fixture declarations: Native child private materialization_config(memory_seconds: f32) -> EngineConfig, materialization_graph() -> GraphHandle, render_materialization(side: &mut AudioSide) -> Vec<SongHostAck>, materialized_pointer(install: &NativeSongInstall) -> *const (). New public target uses actual Native and Browser/Arena EngineIo with callback allocation probe; no production test API. Exact 32768 Hz physical allocation witnesses use binary-exact 96/32768 and 95/32768 seconds for two fixed-state SpectrumPair nodes. Queue refusal retains the returned upload twice, then actual ResourceReady and garbage return authenticate the same allocation. No Cargo run.

0004 ROOT0367 same-manifest refinement: existing AudioHost::song_clock(&self) -> Result<SongHostClock, Failure> validates actual self.env.sr finite/integral/supported8000..192000 and equal to FrameClock integer rate before returning frame/rate. Failure is HostUnavailable; legacy now/FrameClock unchanged. Private native_materialization_clock_rejects_actual_fractional_rate() constructs genuine pair32768.5, proves direct observation failure/purity and actual callback RequestClock Malformed parity; integral32768 and full-u64 frame remain valid. Fixture API audit corrects BusTemplate field to bus.

### 2026-10-02: Complete source/fixture batch HELD before first mandatory checks
0001/0003/0004 immutable intents precede each source batch; 0002 was an intermediate milestone only. Default None and actual Native Some share the existing compiler and preserve borrowed submission semantics. New private tests use actual pair96/95 floats at32768Hz, strict kinds and malformed edges without enqueue, Inst and Bus Boxes retained through two genuine full-ring refusals, actual callback clock-request draining, ResourceReady/Ready and exact off-thread garbage pointers. ROOT0367 adds the actual fractional-rate direct-clock guard, matching Engine clock rejection and keeping legacy now unchanged. New public tests use actual Native callback allocation/deallocation0 probes with cancellation and unsent control-thread drop, and actual Wasm host whole-refusal/borrowed upload through ByteInbox to Arena ResourceReady/Ready/lease return. Three public test functions and four new private test functions are written; inherited browser modules and existing private tests will be inventoried separately by the checker. Scoped formatting succeeded on exactly four owned Rust files; every file is below1000. No Cargo has run, no fixture passing or phase completion is claimed. All four Rust files and own plan HELD; neutral plan separately completed only under ROOT0364 accepted evidence. Fixed PCM union pricing, source-resource closure, owner bridge and full finite playback/export remain later obligations.

### 2026-10-02: ROOT0370 actual test-compile repair, held
Initial mandatory checker original43342 terminal101 polld0b72a: native/wasm passed, Clippy test compilation reported E0599 because AudioSide.acks is AckProducer and has no pop. Raw /tmp/vactr-graph-materialization-independent-001/clippy.log retained; no behavioral tests ran. 0007 fixture-only intent changes render_materialization(side: &mut AudioSide, host: &mut NativeAudioHost) -> Vec<SongHostAck> and its actual callers to consume host.acks AckConsumer, applying existing capacity observation semantics without public poll/drain (which would destroy returned graph owners before pointer inspection). All exact memory/full-key/Box/refusal/Ready/return/rate assertions preserved. Production prefix is unchanged and recorded before/after. No author Cargo, no acceptance claim.

### 2026-10-02: ROOT0372 independent seam acceptance and completion audit
Mandatory retry original25523 terminal0 poll171a86,38 gates passed;813 actual qualified Cargo fixtures plus2 source-origin privacy doctests =815 distinct. Fresh actual names equal inventories; all77 source/plan hashes unchanged. /tmp/vactr-graph-materialization-independent-002/final-results.json SHA030808e70c0518902b0c445b647d3e5044b3eb374fcd53285642f5d2e4cecc90. Supplemental nextest original5bfdb6 terminal0,11 actual dedicated executions,0 selected skips; final SHA9624c879fe549242226c37b6faa0a96de66354782d3669d13cd477b9f9670c03. Root acceptance ROOT0372 SHA cb31e45c6913417b95eb6f46632bc1221f12efa7c55f89ee76efe5427c282b74 independently audited raw names/logs. Native/browser/strict Clippy/scoped format/diff and old DSP/audio/staging/sender/capacity/source regressions passed. Actual own memory96/95, all graph kinds/fullkeys/strict errors, two refused same Boxes/ready/garbage, Browser atomic upload/Arena Ready, unsent control-thread drop/callback0, exact fractional/integral Native clock parity are proven. Initial43342 E0599 and repair receipts remain retained. TASK001/002 and independent gate criteria are complete; TASK003 parent preparation-owner consumption and the literal final integration criteria remain unchecked/In Progress. No existing preparation owner consumes this seam yet, so this plan truthfully remains In Progress rather than claiming Completed. No Rust/Cargo/index/archive changes in this document-only reconciliation.
