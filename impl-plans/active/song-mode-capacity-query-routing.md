# Native capacity query origin routing implementation plan

**Status**: Completed
**Plan ID**: SONG-09PQ
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application), [Bounds](../../design-docs/specs/design-song-mode.md#bounds)

## Purpose and evidence

Keep native measured capacity refreshes internal to their provider, while public
capacity requests retain every success and typed failure receipt. Retry006
original31983 terminated101 after36 staging/bank/carrier fixtures passed: the
actual130-rejection native runtime fixture received unsolicited internal capacity
reports. Native constructor refresh and post-mutation refresh currently escape
poll/drain. The existing cache records only the last epoch and cannot distinguish
public requests with the same epoch, nor capacity failure from unrelated rejection.

Consume the dedicated CapacityRejected result from SONG-09PQ-C. Do not change
legacy Runtime unsolicited-song behavior or weaken the existing130 assertions.
This repair does not establish full staging, private playback, routing or export.

## Manifest

```json
{
  "planId": "SONG-09PQ",
  "dependsOn": ["SONG-09PQ-C"],
  "writePaths": [
    "src/dsp/arena/song.rs",
    "src/dsp/engine/song.rs",
    "src/dsp/engine.rs",
    "src/host/native/audio.rs",
    "src/host/native/audio/song_capacity.rs",
    "tests/song_capacity_routing.rs",
    "tests/song_resource_carriers.rs",
    "impl-plans/active/song-mode-capacity-query-routing.md"
  ]
}
```

Exactly seven Rust modules. Carrier tests transfer serially after the carrier
author's explicit held receipt. routing.rs/caps.rs and song_audio_wire.rs are
read-only inputs. Parent09P source/plan remains held except these prior-approved
serial overlaps. No source writes are released by this planning document.

## Related plans and dependencies

- Previous: [Dedicated capacity results](song-mode-capacity-result-carriers.md).
- Parent: [Actual resource staging](song-mode-resource-staging.md).
- Next: complete staging acceptance, full08 integration and private DSP playback.

| Dependency | Required output | Status |
|---|---|---|
| SONG-09PQ-C | CapacityRejected full-epoch carrier/tag10 and held fixture ownership | COMPLETED |
| SONG-09P | Held configured Engine provider and actual native construction | IN_PROGRESS |

## Exact declarations before implementation

```rust
// New private native/audio/song_capacity.rs; imports existing host/ring POD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapacityQueryOrigin {
    Internal { epoch: SnapshotEpoch },
    Public { epoch: SnapshotEpoch },
}

struct SongCapacityCache {
    report: Option<SongCapacityReport>,
    origins: VecDeque<CapacityQueryOrigin>,
    origin_limit: usize,
    enabled: bool,
    next: u64,
    unsent: Option<SnapshotEpoch>,
    internal_inflight: bool,
    dirty: bool,
}

impl SongCapacityCache {
    fn new(control_slots: usize, ack_slots: usize,
           pending_results: usize, configured: bool) -> Self;
    fn request(&mut self, controls: &mut Producer<NativeRecord>);
    fn retry(&mut self, controls: &mut Producer<NativeRecord>);
    fn invalidate(&mut self);
    fn post(&mut self, controls: &mut Producer<NativeRecord>,
            record: NativeRecord) -> Result<(), NativeRecord>;
    fn observe(&mut self, message: HostMsg) -> Option<HostMsg>;
    fn capacities(&self) -> Result<SongHostCapacities, SongRejectCode>;
    fn analysis(&self) -> Result<SongAnalysisCapacity, SongRejectCode>;
}

// Relocate these existing definitions into the owned private native helper.
impl NativeAudioHost {
    fn request_song_capacity(&mut self);
    pub fn song_remaining_capacities(&self) -> Result<SongHostCapacities, Failure>;
    pub fn song_analysis_capacity(&self) -> Result<SongAnalysisCapacity, Failure>;
    pub fn submit_song_preparation(&mut self, p: SongStagePreparation)
        -> Result<(), Failure>;
    pub fn submit_song_native(&mut self, install: NativeSongInstall)
        -> Result<(), NativeSongInstall>;
}
```

Private cache methods can use pub(super) visibility for parent integration;
parent NativeAudioHost fields remain private and accessible to its child module.
The by-value queue failure returns the original NativeRecord; retain a narrowly
documented result_large_err allowance rather than allocating an error Box.
No helper runs in the audio callback. Existing public native host signatures stay
unchanged. Remove the former arena cache and both engine reexports entirely.

### Complete capacity results

Existing Engine RequestCapacity success remains CapacityReport. Only its failures
become CapacityRejected{epoch,reason}; unrelated Song commands retain Rejected.
Both paths use the existing critical receipt/backpressure mechanism. No discarded
capacity errors, fake successful report or serial overflow assumption is allowed.
Dedicated carrier tag10 has11 bytes; ACK_MAX stays74. Codec implementation is the
previous child, not this manifest. Change the actual unconfigured browser request
fixture expectation in resource_carriers only after that child holds.

### Origin admission and boundedness

- Allocate the FIFO on native host construction from checked actual
  controls.capacity + acks.capacity + pending capacity-result slots. The current
  engine's song_ack/song_followup provide a conservative two-slot pending bound.
  Capacity replies cannot appear without an accepted RequestCapacity.
- This bounds successful requests not yet observed: requests still in controls,
  completed results in acknowledgments, plus retained callback results. Public
  and internal requests share the same ordered native control transport.
- Reserve FIFO room before pushing each capacity request; append its exact origin
  only after push succeeds. On failure retain/return the original public record;
  do not advance identifiers or create a phantom origin. Queue pressure remains
  observable through existing host failure/counter behavior.
- Automatic refresh has at most one in-flight request and one exact unsent epoch.
  Once an epoch is assigned, a full queue retains it for retry. A successful
  subsequent mutation sets dirty and invalidates cached capacities.
- Retry does not bypass earlier successful control records. Clear dirty when the
  internal request successfully enters the queue. Mutations after that point make
  its eventual result stale; consume it but request only one follow-up refresh.
- A matching internal failure clears the in-flight request and leaves the getter
  honestly unavailable. Preserve a dirty refresh requested by a successful post-probe mutation: it
  admits one follow-up; a failure without subsequent mutation does not
  automatically retry. A later explicit request or mutation may refresh.
- Use the actual configure Result at construction. An unconfigured/quad provider
  issues no internal probe. Public RequestCapacity still reaches the engine and
  its dedicated failure is forwarded. Checked sizing/identifier exhaustion disables
  internal probing and leaves honest unavailable getters; no wrapping epochs.
- Completed requests are classified by FIFO origin, never an epoch range. Compare
  the exact response epoch to the head as an invariant check. Public same-epoch
  success/failure and unrelated Rejected messages must remain unchanged.
- Only CapacityReport/CapacityRejected consume query origins. Unmatched messages
  are forwarded unchanged and cannot update the cache. A mismatched head/result
  invalidates internal freshness; it is not silently classified as internal.

### Poll, drain and freshness

poll_msg pops at most one actual acknowledgment. After consuming an internal
result it may return None; it must not continue into a second receipt and evade
Runtime's bounded receipt/backpressure behavior. drain filters authenticated
internal results and preserves all public/non-capacity messages in original order.
Both paths retry retained requests without new identifiers. Successful relevant
native/control mutations invalidate the report; publication/retirement of physical
resources must not leave an obsolete over-report treated as current. Avoid probes
for ordinary note/cell traffic that does not change measured capacity. The exact
mutation/result classification is reviewed against current Engine physical state.

## Module status and size feasibility

| Module | Deliverable | Status |
|---|---|---|
| arena/song.rs | Remove host-only cache; retain all resource behavior | COMPLETED |
| engine/song.rs | Remove cache reexport; dedicated query failure dispatch | COMPLETED |
| engine.rs | Remove retired cache reexport | COMPLETED |
| native/audio.rs | Register helper; retain configure Result; one-ack poll/filter drain | COMPLETED |
| native/audio/song_capacity.rs | Bounded coalesced cache and relocated host helpers | COMPLETED |
| tests/song_capacity_routing.rs | Genuine host/runtime and cache pressure/collision fixtures | COMPLETED |
| tests/song_resource_carriers.rs | Serial producer expectation change only | COMPLETED |

Removal makes arena/song and native/audio smaller. engine995 loses a reexport;
engine/song930 has room for typed failure dispatch. New helper/test modules must
remain below1000. No additional path, broad formatting or dependency change is
implicit. New test binary may include private helper fixtures through existing
module tests; do not expose private APIs solely to test them.

## Tasks

### TASK-001: Transfer, declarations and source increment
**Status**: Completed
**Parallelizable**: No

- [x] Acquire carrier hold; record fresh seven-source baselines/new absences.
- [x] Mark plan In Progress before mutation and record numbered immutable intents.
- [x] Relocate cache/helpers, integrate typed query failures and exact origin FIFO (independently executed and accepted).
- [x] Preserve successful actual staging and original owned queue rejection.

### TASK-002: Meaningful behavior and compatibility fixtures
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No

- [x] Actual legacy constructor/drain/poll produces no unsolicited song receipts.
- [x] Existing130 native/runtime rejections arrive once, in order, unchanged.
- [x] Public same-epoch requests before/after internal requests retain success and
      typed failure; unrelated same-epoch rejection cannot consume a query origin.
- [x] Real full controls/acks and bounded ledger preserve unsent identity and FIFO.
- [x] One poll consumes at most one acknowledgment; internal consumption may
      return None while the next public receipt remains queued.
- [x] Multiple mutations produce one in-flight plus one needed follow-up; stale
      replies cannot restore getter freshness. Internal failure has no retry storm.
- [x] Genuine configured provider reports actual dimensions; failed/quad provider
      remains honestly unavailable and forwards public dedicated query failure.
- [x] Actual codec/engine expectation and serial/checked-failure private cases
      preserve exact full-width identity and old generic rejection semantics.

### TASK-003: Held verification and independent acceptance
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No

- [x] Establish all-author hold before any Cargo. Retain exact SHA/line receipts.
- [x] Quiet native/host-wasm/strict Clippy and scoped fmt/diff pass on held bytes.
- [x] Fresh qualified inventories and unfiltered new query, carrier, wire, staging,
      bank and relevant legacy runtime/native suites execute with nonzero counts.
- [x] No identical repeated nextest execution; fresh nonempty dedicated inventory and unique runs recorded. (Fresh qualified Cargo inventories and executions; no nextest execution.)
- [x] Mandatory independent checker verifies stable cohort; retain failures and
      every originating foreground terminal handle. No phase completion from preflight.

## Completion criteria

- [x] Every module/task/behavior criterion complete; touched Rust remains below1000.
- [x] Internal results are consumed exactly once; public results remain observable.
- [x] Fresh getters reflect actual post-mutation engine reports, never guesses.
- [x] All execution/count/hash evidence recorded and independently accepted.
- [x] Parent staging/full playback status remains honest; archive/index deferred.

## Progress log

### 2026-10-02 — plan-only ROOT0236

Read actual native pair/post/poll/drain, capacity cache, Engine query/error path,
wire codec, generic runtime receipt handling and the existing failing130 fixture.
The dedicated capacity result child removes same-epoch rejection ambiguity.
This document declares seven Rust paths and three serial tasks. No Rust, Cargo,
existing plan, index or Git mutation occurred. Source implementation awaits root
review/release and the carrier author's held transfer.


### 2026-10-02 — coherent implementation held 0005

ROOT0237 verified the exact seven source baselines and transferred the987-line
carrier fixture serially. Removed the old arena-host cache and both engine
reexports. Native host now retains the actual configure result, registers the
private host helper, classifies complete FIFO query results and consumes at most
one acknowledgment per poll. The relocated cache allocates its bounded origins
at construction, coalesces one internal request, retains exact unsent epochs and
post-probe dirty refreshes, and forwards public same-epoch success/failure intact.
Only Engine RequestCapacity failures now use CapacityRejected; unrelated failures
remain generic Rejected. The existing unconfigured carrier expectation changes
only its matching acknowledgment variant.

Five public genuine native/Runtime fixtures, five private real-ring cache fixtures
and one actual Engine serial-exhaustion fixture are written, not executed. They
cover legacy constructor/ordinary traffic, public full-width FIFO/collision,
one-ack polling, unconfigured quad,130 actual rejections, bounded admission,
retained unsent owner, coalescing/stale freshness and failed-query follow-up.
Reviewed post-probe failure correction preserves exactly one dirty follow-up;
a failed query without a subsequent mutation stops automatic retries.
Scoped formatting of only the seven owned Rust paths passed. No author Cargo
ran. All sources/own plan are held pending a root-coordinated mandatory joined
checker with the disjoint staging acceptance author. No phase/full playback
completion or duplicate nextest execution is claimed.


### ROOT0248 private fixture declarations before source

```rust
fn drive_capacity_engine(
    engine: &mut Engine,
    controls: &mut Consumer<NativeRecord>,
    events: &mut EventConsumer,
    acks: &mut AckProducer,
    cells: &mut AtomicCells,
);
fn actual_engine_control_and_ack_pressure_retains_complete_query_origins();
```

Use actual configured Engine construction with actual measured one-slot ACK and
control rings. Fill ACK storage through a genuine public callback result, then
queue another same-epoch public query and retain the exact unsent internal probe
under simultaneous full control/ACK pressure. Critical callback backpressure must
retain the second result and stop consuming the queued internal probe. Drain one
actual result at a time; public replies remain exact and once-only, internal reply
is consumed and the getter changes from pending to actual measured fresh.
A paired genuinely unconfigured Engine pass exercises actual CapacityRejected
under the same pressure, without synthetic reports. Its cache is explicitly
exercised as a failing-query consumer; production native startup still uses the
actual configuration flag and never probes an unconfigured/quad provider.
No callback setter/private Engine access or ring metadata substitution is used.


### ROOT0248 written actual Engine pressure proof

ROOT0247 audited135 actual joined fixtures and32 gate exits; native query12 and
unchanged wire130 behavior passed. The remaining explicit actual ACK-pressure
criterion now has a new private fixture using EngineIO/process and measured
one-slot control/ACK rings. Actual public callback results fill acknowledgment
storage; a second same-epoch public query retains its critical result, while an
exact unsent internal query waits for control room. One-at-a-time drainage proves
FIFO public results, internal-only consumption and measured getter freshness.
A paired real unconfigured callback produces CapacityRejected under the same
pressure; its cache exercise is not a claim native startup probes unconfigured
providers. This new fixture is written, not executed. No production behavior
changed, no author Cargo ran; source/plan held for mandatory checker evidence.


### ROOT0250 — documentation audit of independent evidence

ROOT0247 accepted 32 green gates and 135 distinct actual joined fixtures, original
39469 terminal0, result SHA f52666708d5b5a439c5a875fcb5bb810abb3fd68029f85dfb297adb3ecd5da80.
ROOT0250 accepted the real configured/unconfigured Engine ACK/control pressure
fixture with 12 green gates and 13 actual query fixtures, original13729 terminal0,
result SHA1273f24a81a1f3b859e160c054b98bc41549d0cb3946541e058930bb5d1fb277.
The 13 include the previously run12; they are not13 additional distinct fixtures.
Evidence: /tmp/vactr-song09pq-pressure-independent-001/final-results.json and
tmp/song-mode-riela/ROOT0247-query-routing-physical-scoped-acceptance.json.
All seven owned Rust hashes remain unchanged from0008; all touched files are
below1000. Module implementation and TASK001/002 behavior are proven.
The sole unproven literal TASK003 requirement is a dedicated nextest inventory
and unique nextest run: audited matrices used Cargo test, so this plan stays
In Progress rather than asserting an unexecuted check. No author Cargo ran.
SONG09PQ-C is Completed under ROOT0251/0252; parent full09P, private audible
activation and full song playback remain unfinished. Archive/index are deferred.

### ROOT0250 — final literal criterion reconciliation

Parent review clarified TASK003 prohibits identical repeated nextest execution;
it does not mandate nextest. The existing fresh qualified Cargo inventories and
recorded unique commands satisfy its inventory/execution clause. No nextest ran,
and no tool-specific redundant rerun is claimed. The preceding audit's purported
unproven nextest requirement was an interpretation error, corrected here.
The pressure matrix separately inventories capacity-private, capacity-result and
capacity-routing, then executes their unfiltered suites: 6+1+6=13 actual tests.
Their command/inventory/test logs remain in
/tmp/vactr-song09pq-pressure-independent-001/; ROOT0250 verified all43 cohort
hashes and log evidence. All child requirements are independently proven; this
plan is Completed. The full parent09P/playback goal remains pending.
