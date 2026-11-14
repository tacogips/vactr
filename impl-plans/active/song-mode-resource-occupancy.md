# Song resource occupancy implementation plan

**Status**: Completed
**Plan ID**: SONG-09PO
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Bounds](../../design-docs/specs/design-song-mode.md#bounds), [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Purpose

Make actual physical occupancy and outstanding song reservations additive.
Current engine/song capacity reporting takes min(constructor capacity minus all
leases, physical free slots); reserve consults constructor capacity alone.
Three free slots, one unadopted song lease and one installed legacy owner must
leave one available slot, rather than the current two. A physically adopted
song lease must count once. Preserve ownership until exact cleanup/handoff.
This prerequisite supports actual staging; it does not activate audio or clear
parent staging, routing, playback or export.

## Exact manifest

```json
{
  "planId": "SONG-09PO",
  "dependsOn": ["SONG-09PQ", "SONG-09PC", "SONG-08C"],
  "writePaths": [
    "src/dsp/engine/song.rs",
    "src/dsp/engine/occupancy.rs",
    "src/dsp/arena/song.rs",
    "src/dsp/arena.rs",
    "src/dsp/bus.rs",
    "src/dsp/bus/occupancy.rs",
    "src/dsp/engine.rs",
    "tests/song_resource_capacity.rs",
    "impl-plans/active/song-mode-resource-occupancy.md"
  ]
}
```

Eight Rust paths; no other source, parent plan, acceptance fixtures, dependency,
index or archive writes. This plan-only draft grants no Rust/Cargo execution.

## Related plans and dependencies

- Previous: [Native query routing](song-mode-capacity-query-routing.md).
- Parent: [Actual staging](song-mode-resource-staging.md).
- Next: [Private DSP routing](song-mode-dsp-routing.md).

| Dependency | Required output | Status |
|---|---|---|
| SONG-09PQ | Measured report cache and exact public/internal result ownership | Completed |
| SONG-09PC | Full-key staged resources and return carriers | Completed |
| SONG-08C | Copied owner-local cell references/ranges | Completed |
| SONG-09P | Existing silent physical stores/slots and exact cleanup | In Progress |

## Declarations before implementation

### src/dsp/engine/occupancy.rs — new Engine child

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SongUnadoptedCounts {
    pub(crate) templates: u32,
    pub(crate) buses: u32,
    pub(crate) samples: u32,
}

impl Engine {
    pub(super) fn song_unadopted_counts(&self) -> Result<SongUnadoptedCounts, SongRejectCode>;
    pub(super) fn song_legacy_claims(&self) -> Result<SongUnadoptedCounts, FaultCode>;
    pub fn song_remaining_capacities(&self)
        -> Result<SongHostCapacities, SongRejectCode>;
    pub fn begin_song_preparation(&mut self, command: SongStagePreparation)
        -> Result<(), SongRejectCode>;
    pub fn reserve_song_resource(&mut self, command: SongResourceReservation)
        -> Result<(), SongRejectCode>;
    pub fn song_bus_regions(&self) -> &[SongFrameRegion];
    pub(super) fn install_bytes<C: CellStore>(&mut self, id: u32, gen: u32,
        bytes: &[u8], cells: &C) -> Result<(), FaultCode>;
    pub(super) fn install_native<C: CellStore>(&mut self, n: NativeInstall, cells: &C,
        garbage: Option<&mut Producer<Garbage>>) -> Option<HostMsg>;
    pub(super) fn return_legacy_install(&mut self,
        garbage: Option<&mut Producer<Garbage>>) -> bool;
    pub(super) fn retain_legacy_owner(&mut self, owner: Garbage,
        garbage: Option<&mut Producer<Garbage>>);
    pub(super) fn begin_legacy_sample(&mut self, resource: u32, gen: u32,
        frames: usize, channels: u8, rate: u32) -> Result<(), FaultCode>;
}
```

Relocate existing capacity/begin/reserve methods from song.rs and existing
install_bytes/install_native from engine.rs, preserving public signatures and
existing output-channel checks, fault/receipt and owned-return behavior.
Register `mod occupancy;` in engine.rs. Child integration methods use pub(super)
so Engine parent and sibling song module can invoke them without public exports. Routine private helper refinements must
remain within this ownership contract; no new public transport or fake capacity.

### src/dsp/arena/song.rs — authoritative ownership and lease admission

```rust
impl SampleStore {
    pub(crate) fn song_contains(&self, key: SongLeaseKey) -> bool;
}
impl SongResourceStager {
    pub(crate) fn begin_with_available(&mut self, command: SongStagePreparation,
        available: SongHostCapacities) -> Result<(), SongRejectCode>;
    pub(crate) fn reserve_with_available(&mut self,
        command: SongResourceReservation, available: SongHostCapacities)
        -> Result<(), SongRejectCode>;
}
```

Count every retained lease lacking its exact physical full-key owner, including
retiring leases. `lease.staged` is not a physical ownership test. Native sample
validation reserves a real Entry before it marks the lease staged; browser
SampleBegin likewise reserves a real entry/extent before completion. These
Reserved/Installing entries are physically present and must not be debited twice.
The reserve helper preserves stale/duplicate/declared-resource/table checks and
checks current available role capacity before insertion. No snapshot reset.

### src/dsp/arena.rs — template/sample legacy admission

```rust
impl Templates {
    pub(crate) fn song_contains(&self, key: SongLeaseKey) -> bool;
    pub(crate) fn build_with_song_claims(&mut self, raw: &RawGraph,
        env: &BuildEnv, resource: u32, gen: u32, unadopted: u32)
        -> Result<(), FaultCode>;
    pub(crate) fn adopt_with_song_claims(&mut self, template: Box<Template>,
        resource: u32, gen: u32, unadopted: u32)
        -> Result<(), Box<Template>>;
}
impl SampleStore {
    pub(crate) fn begin_with_song_claims(&mut self, id: u32, gen: u32,
        frames: usize, channels: u8, rate: u32, unadopted: u32)
        -> Result<(), FaultCode>;
    pub(crate) fn install_arc_with_song_claims(&mut self, id: u32, gen: u32,
        data: Arc<SampleData>, unadopted: u32)
        -> Result<(), Arc<SampleData>>;
}
```

Existing standalone public legacy methods retain their signatures and use zero
external claims. Engine's actual legacy ingress supplies its freshly derived
claims. Physical slot count minus unadopted claims must remain positive before
new allocation. Preserve byte extent and NativeArc PCM checks independently.
No cached guard counter can become stale after adoption/cancellation.

### src/dsp/bus/occupancy.rs — new exact bus ownership/selection helper

```rust
impl BusGraph {
    pub(crate) fn song_contains(&self, key: SongLeaseKey) -> bool;
    pub(crate) fn song_available_frames(&self, unadopted: u32)
        -> Result<u64, SongRejectCode>;
    pub(super) fn legacy_slot_with_song_claims(&self, unadopted: u32)
        -> Option<usize>;
    pub(super) fn slot_withheld_for_song(&self, index: usize, unadopted: u32)
        -> bool;
}
// Existing bus.rs public facade; register mod occupancy.
impl BusGraph {
    pub(crate) fn install_with_song_claims<C: CellRead + ?Sized>(
        &mut self, template: &BusTemplate, master: bool, resource: u32,
        gen: u32, cells: &C, sr: f32, caps: &CapabilitySet,
        unadopted: u32) -> bool;
}
```

Existing public install delegates with zero claims, retaining its public signature
and ordinary first-free selection. The guarded facade in bus.rs uses the helper's
selected index before any configure/retirement writes. Move cohesive selection
and exact ownership logic to the child; child can inspect parent-private BusSlot
mem/song_key. No heap scratch or added public SlotState variant.

Only actual Free slots contribute physical regions. No newly fabricated extent
or lease-to-region assignment exists before graph geometry is known. Conservatively
withhold the largest U currently free bus regions for U unadopted bus claims;
report the checked sum of the remaining actual regions. Stable equal-size ties
use slot index. Legacy bus admission chooses only among the available remainder
and retains existing per-slot required-memory checks. Song graph adoption still
checks actual individual contiguous fit across physical slots; no aggregate fit
substitutes for that check. Counting/sorting policy uses bounded scans of fixed
slots, without callback allocation. The existing song_bus_regions accessor remains
explicitly a physical-free descriptor view, not a claimed available-region view;
callers must use the claim-adjusted capacity report for admission. Do not advertise
withheld regions as assigned to any particular lease.

### src/dsp/engine.rs / song.rs — production call sites

- engine.rs Record::SampleBegin calls begin_legacy_sample rather than direct begin.
- Record::Graph calls relocated install_bytes: Inst invokes guarded Templates
  build; Bus/Master invokes guarded BusGraph install.
- Record::Native calls relocated install_native: Inst/Bus/Sample invoke matching
  guarded methods, preserving original Box/Arc failure and garbage backpressure.
- song.rs RequestCapacity invokes the same current adjusted report; ReserveResource
  invokes guarded reserve, BeginStaging invokes adjusted admission.
- Exact ownership is found in Templates slot.song, BusSlot song_key and SampleStore
  Entry.song; physical presence includes every retained state until actual removal.
- Existing collectors/cleanup keep their ownership semantics; refreshed calculations
  need no remembered snapshot or mutable provider guard counter.

### Native rejection ownership under garbage pressure

Current legacy install_native uses ring::push_garbage, which explicitly forgets
owners on missing/full garbage rather than returning them. New reservation-based
rejections must not inherit that leak and must not drop Box/Arc on the callback.
Add one construction-initialized Engine field:

```rust
pending_legacy_install: Option<Garbage>,
```

Relocated install_native hands each rejected Inst/Sample owner and every consumed
native Bus template to retain_legacy_owner. Try the actual garbage Producer push;
retain its exact Err owner in the one pending field on full/missing queue.
No owner may replace an occupied pending field. controls calls return_legacy_install
before consuming any new control record; if handoff still cannot succeed it stops.
After one record creates pending ownership, stop that consumption loop immediately.
Thus the one-owner-per-native-record bound suffices without callback allocation.
Song pending owner/critical receipt gates remain independently effective. This
ownership adjustment is scoped to legacy native install returns touched by this
child, not an assertion that unrelated historical retirement garbage handling is
already repaired. Keep generic fault/Installed receipt identity unchanged.
Meaningful tests use actual full/missing garbage and then exact drain/return,
verifying pointer/data preservation and no further install consumption meanwhile.

## Report and admission contract

Compute template/bus/sample remaining as checked physical-free minus unadopted
claims, never min(two independent counters). A broken overclaim yields honest
Capacity rather than underflow/wrap. Do not erase private control/analysis counts,
transport/branch/voice limits or existing measured PCM capacity. Stager snapshot
remaining is not authoritative for physical pool availability.

ReserveResource lacks per-sample PCM length and per-bus graph memory. It reserves
one role slot only. No PCM extent/byte promise is claimed until actual SampleBegin
or native validation supplies geometry and creates a real entry. Existing
BeginStaging aggregate requirements remain an admission check, not an invented
contiguous PCM reservation. Native pending validation already owns its entry and
charged length. Legacy PCM admission respects that actual entry/extent. This
child does not manufacture per-lease byte bounds from unrelated metadata.

## Module status and size budget

| Module | Current lines | Expected after relocation | Status |
|---|---:|---:|---|
| engine/song.rs | 911 | 880–930 | COMPLETED |
| engine/occupancy.rs | 375 | 300–550 | COMPLETED |
| arena/song.rs | 908 | 915–975 | COMPLETED |
| arena.rs | 953 | 920–985 | COMPLETED |
| bus.rs | 886 | 940–975 | COMPLETED |
| bus/occupancy.rs | 151 | 120–250 | COMPLETED |
| engine.rs | 909 | 880–920 | COMPLETED |
| tests/song_resource_capacity.rs | 638 | 350–650 | COMPLETED |

Keep every touched Rust below1000, including formatting. Cohesive relocations
must precede growth; request prior bounded manifest revision if estimates fail.

## Tasks

### TASK-001: Current ownership and capacity integration
**Status**: Completed
**Parallelizable**: No

- [x] Acquire stable source/fixture-owner holds and fresh baselines before mutations.
- [x] Mark In Progress and record numbered source intents.
- [x] Relocate Engine capacity/install helpers, register child and implement physical
      exact ownership probes and checked additive report/reservation admission.

### TASK-002: Legacy admission and meaningful regression fixtures
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No

```rust
fn unadopted_bus_and_template_claims_add_to_legacy_occupancy();
fn adopted_physical_owners_are_charged_once();
fn native_pending_pcm_and_browser_installing_entries_are_charged_once();
fn legacy_ingress_cannot_consume_last_reserved_slot();
fn sparse_bus_geometry_is_conservative_and_individual_fit_is_required();
fn cancellation_and_delayed_owner_return_keep_capacity_until_exact_release();
fn foreign_full_keys_and_multiple_epochs_never_alias_physical_owners();
fn rejected_legacy_native_owners_survive_full_and_missing_garbage();
```

- [x] Three actual free slots, one song claim, one legacy owner reports exactly one;
      second preparation requiring two rejects. Exercise instruments and buses.
- [x] Adoption before/after legacy installation reports the same correct count;
      cancellation restores capacity only after actual ownership/lease removal.
- [x] Actual one-slot legacy queue/install rejection preserves native Box/Arc owners
      and browser failure identity; no unsolicited successful receipt.
- [x] Native pending validation and browser Reserved/Installing PCM cases debit
      actual slots/bytes once even while staged=false. Unuploaded Sample claim
      debits one slot but does not invent PCM length.
- [x] Unequal actual BusSlot allocations (not fabricated region descriptors) demonstrate largest-U withholding and remaining
      checked sum; individually oversized graph rejects despite large aggregate.
- [x] Full-key epoch/generation/kind mismatches do not suppress outstanding claims.
- [x] Full/missing real garbage keeps original native Box/Arc pointers and payloads;
      callback stops consuming until exact off-thread handoff succeeds.
- [x] Existing zero-claim legacy behavior and configured silent Ready remain intact.

### TASK-003: Held mandatory verification
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No

- [x] Exact held SHA/line receipts and fresh nonempty qualified inventories precede
      root-coordinated native/host-wasm/strict Clippy/fmt/diff and affected tests.
- [x] New actual capacity tests plus staging positive graphs, query, carrier, wire,
      arena/bus/native legacy regressions pass; record original terminal handles.
- [x] Mandatory independent checker accepts unchanged cohort; retain all failures.
      No identical redundant nextest runs or stale fixture counts.

## Completion criteria

- [x] All modules/tasks and meaningful actual regression fixtures complete; ROOT0279 verifies the direct claimed-capacity browser rejection witness.
- [x] Report/reserve and every Engine legacy adoption path use additive occupancy.
- [x] Physical presence, not staged flag, governs double-charge avoidance.
- [x] Bus geometry and PCM declarations are honest, bounded and preserve individual fit.
- [x] Rust<1000, checked no-allocation callback work, independent evidence retained.
- [x] Parent09P/private audio/full song mode remain independently pending.

## Progress log

### 2026-10-02 — ROOT0255 plan-only

Read current Engine capacity/reserve/install dispatch, full-key SampleStore entries,
BusGraph region maintenance and TemplateSlots. Confirmed min-based undercount and
native validation's real Reserved entry before staged flag. Proposed eight-path
cohesive Engine child to fit existing near-limit parents. No Rust, Cargo, existing
plan, archive/index or Git changes. Await root contract review and source release.

### Plan-only eighth-module/owner-return refinement

Root review requested bus/occupancy.rs as the exact eighth Rust path, keeping
bus.rs below1000. Selection is a bounded fixed-slot scan with stable index ties;
real unequal BusSlot allocations must drive the fit tests. Read actual legacy
ring::push_garbage leak behavior before relocation; declared a single retained
legacy return owner and consumption gate so reservation-induced rejection cannot
leak or drop native ownership under garbage pressure. No source/Cargo mutation.

### Current BeginStaging refinement before source

Root review identified redundant constructor-snapshot physical-role checks in
stager.begin. Its only Engine caller must pass the fresh actual available report
to begin_with_available; preserve table/private-cell/analysis checks, and use
that report for fits rather than constructor physical counters. Add actual legacy
owner installed before provider construction, then retired/released after:
reported restored capacity must also be admitted. Template physical free count
requires the tier-compatible preallocated Some/None representation.

### ROOT0262 coherent source/fixture checkpoint held

Relocated capacity/reserve/begin and legacy install helpers into Engine occupancy
child, and guarded bus selection into bus occupancy child. Current reports use
actual tier-compatible template slots, bus/sample physical ownership and checked
unadopted full-key counts. Pending NativePCM and browser Installing ownership is
counted by real Entry presence. The stager begin helper receives actual available
capacity; stale constructor physical counters no longer reject restored storage.
Private-cell/analysis/protocol-table constraints remain. Legacy ingress preserves
actual graph checks and rejects before consuming outstanding role claims.
A retained one-owner return slot and intake gate preserve rejected native owners
under full/missing garbage while rendering continues.
Eight public fixtures and two private fixtures are written, not executed. These
cover additive legacy occupancy, adoption double-charge, actual Native pending
and browser Installing PCM, reserved-slot legacy refusal, delayed cancellation,
foreign generation/multiple epochs, audible legacy render during blocked return,
exact Arc return and actual unequal BusSlot memory. The restored-constructor
admission test uses a real private pre-processing install because public provider
configuration correctly forbids configuring after process blocks.
Scoped rustfmt of exactly eight Rust paths passed; every file remains below1000.
No author Cargo ran. All eight Rust files and own plan HELD for root-coordinated
mandatory checker; task completion awaits actual execution/independent findings.
Full parent09P/private audio/full song mode remain pending.

### ROOT0267 extraction documentation repair

Joined checker original40829 terminal101: native and host-wasm passed; strict
Clippy stopped before fixtures on four orphan doc/attribute blocks left by helper
relocation. Preserved /tmp/vactr-song09p-occupancy-audible-independent-001/clippy.log.
Moved original begin/install_arc/build/adopt documentation onto actual relocated
public facades and original bus install docs/argument attribute onto its facade.
No behavior change or lint suppression was added. Scoped formatting only; no
author Cargo. All source/plan bytes held again for fresh mandatory execution.

### ROOT0269 unused fixture producer repair

Retry24467 terminal101: native/host-wasm passed; strict Clippy stopped on an unused
copied Rig.events_tx field before behavior tests. Removed only that producer
field/binding; actual EventConsumer and LiveNoteOn audible ownership assertions
remain. Full diagnostic retained in
/tmp/vactr-song09p-occupancy-audible-independent-002/clippy.log.
No production change, allowance or Cargo execution; all bytes held for retry.

### ROOT0271 actual required-memory fixture correction

Retry19447 terminal101: native/host-wasm/strict Clippy passed; private restored
admission fixture passed, unequal bus fixture failed because ordinary Convolution
intentionally permits legacy clamped memory. Corrected only private input to
SpringReverb, with explicit requires_full_memory and actual mem_len greater than
selected real region after room allowance. Preserved384/256 available sums and
withheld512 region, plain-install positive and previous3Delay staging aggregate
fit evidence. Diagnostic retained in
/tmp/vactr-song09p-occupancy-audible-independent-003/occupancy-private-tests.log.
No production change, Cargo or semantics waiver; held for mandatory retry.

### ROOT0273 independent evidence and literal completion audit

Independent004 original46165 terminal0: 36 commands/gates and150 distinct actual
qualified tests passed, all48 cohort hashes and raw logs audited by root. Result
SHA0b3ccf0fafd56feee458a66029a2b65c455df7e7f1b2ad1592e228703226c2f0,
/tmp/vactr-song09p-occupancy-audible-independent-004/final-results.json.
The 8 public occupancy fixtures and2 private fixtures are part of150, not added
again. Native/host-wasm/strict all-target Clippy/scoped fmt/diff are green.
All8 own Rust hashes remain exactly those independently accepted; no author Cargo.

| Requirement | Actual evidence / source |
|---|---|
| Additive report/reserve, legacy bus/template occupancy | unadopted_bus_and_template_claims_add_to_legacy_occupancy |
| Adopted single charge | adopted_physical_owners_are_charged_once |
| Reserved/Installing PCM and unknown-geometry slot claim | native_pending_pcm_and_browser_installing_entries_are_charged_once |
| Guarded native legacy role admission | legacy_ingress_cannot_consume_last_reserved_slot |
| Exact Box return, nonzero audio continues under missing/full garbage | rejected_legacy_native_owners_survive_full_and_missing_garbage |
| Exact Arc return and slot claim protection | sample_claims_block_legacy_pcm_and_retain_arc_until_garbage_handoff |
| Delayed cancellation keeps claim until handoff | cancellation_and_delayed_owner_return_keep_capacity_until_exact_release |
| Epoch/generation full-key distinction | foreign_full_keys_and_multiple_epochs_never_alias_physical_owners |
| Kind equality and actual malformed kind rejection | graph_bank_native_epoch_and_kind_rules_reject_invalid_ownership; wrong_native_kind_is_returned_as_malformed_without_adoption; exact slot key comparisons |
| Genuine unequal allocated bus geometry and required individual memory | bus::occupancy private actual_unequal_slot_memory_is_withheld_and_individual_fit_still_applies |
| Legacy release after provider construction restores current admission | engine::occupancy private legacy_release_restores_current_admission_after_provider_construction |
| Native/Arena complete closures, silent Ready, malformed ownership | staging20, carrier19 and banks4 unfiltered fixtures |
| Actual audible preservation / callback memory | paired live Native/Arena probes and callback_retained_rejection_allocates_and_destroys_nothing |
| Compatibility / current capacity provider | query13, wire21, routes32, arena5, bus7, analysis4, render7, native-adapter8 |

The remaining literal TASK002 browser failure-identity clause has source proof
(guarded legacy decode/begin preserves fault/resource), but no direct actual
legacy ByteInbox install rejected specifically by outstanding last-slot claims
was identified in the fresh inventories. Keep that checkbox and overall plan
In Progress pending a genuine witness, rather than infer it from successful byte
closure or native failure tests. All other proven criteria are marked accurately.
Public rendered bus/master equality is an operational detector witness, not direct
KeySnapshot access. Combined ACK+garbage retirement pressure remains parent09P
work; full09 private activation/playback, full08D geometry and joint staged/runtime
quotas remain unfinished integration. No archive/index/Git changes.

### ROOT0275 actual browser claimed-role fixture declarations

```rust
fn byte_legacy_record(kind: SongResourceKind, resource: u32) -> Vec<u8>;
fn process_byte_capacity(rig: &mut Rig, record: &[u8], epoch: SnapshotEpoch)
    -> Vec<HostMsg>;
fn byte_legacy_claim_rejections_preserve_fault_identity_and_following_record();
```

Parameterize valid legacy Inst graph, neutral Bus graph and SampleBegin using
actual encode_inst/encode_bus/encode_graph_record/encode_sample_begin. Pair each
with a zero-claim Arena success on the same bytes, then a constructor-configured
adequate lease table and all real available role slots reserved by genuine keys.
Rejected byte ingress must preserve exact BadResource/resource, current capacity
and outstanding claims, emit no Installed, and process a following encoded full
u64 capacity query. Collect complete HostMsg receipts, not song-only filtering.
No production changes or synthetic physical counters. Written acceptance awaits
mandatory joined execution after both authors hold.

### ROOT0279 final child completion audit

Independent pressure/browser003 original4235 terminal0: all36 gates and153
qualified actual tests passed; root verified all48 current cohort hashes and log
SHAs. Evidence /tmp/vactr-song09p-pressure-browser-independent-003/final-results.json
SHA2e495169f21789dc625e8a9b45468ad35384c4dab7293887f3bfcc015fba0511.
Actual9 public capacity fixtures and2 private occupancy fixtures are included
in153; they are not added to the earlier150 scope. Previous failures remain
retained in independent001/002/003 and own numbered receipts.

The outstanding browser requirement now has direct execution evidence:
byte_legacy_claim_rejections_preserve_fault_identity_and_following_record uses
valid encoded legacy Instrument/Bus/SampleBegin records, establishes zero-claim
validity, then exhausts each actual role with outstanding claims. Exact BadResource
and resource identity, no false Installed, unchanged capacity, and following valid
RequestCapacity recovery all passed. This supersedes the historical pending
completion assessment above without changing its evidence history.

All eight declared modules, three tasks and child completion criteria are now
fulfilled under ROOT0273 and ROOT0279 evidence. No remaining child criterion is
unproven. Public bus/master equality remains an operational detector witness,
not direct private KeySnapshot access. Physical resource safety does not establish
full song audio: full09 runtime activation/rendering, full08B/08D integration and
shared staged/runtime admission remain separate obligations. Root audits parent09P
combined pressure independently; this child does not mark that parent complete.
This completion changes only this plan; all Rust hashes stay unchanged and no
Cargo, archive, index or Git mutation is performed.
