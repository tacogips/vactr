# Private song branch delay implementation plan

**Status**: In Progress
**Plan ID**: SONG-PRIVATE-DELAY
**Created / Last Updated**: 2026-10-02
**Session target**: 1–3 sessions
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Intent and exact prerequisite

Provide real branch-owned delay for the already-priced song delay controls.
prepare.rs121–145 reserves stereo2*(4*sample_rate+4) floats per live generation,
but current BusSlot has no such region. engine/song_runtime.rs331 rejects every
nonzero delay send; engine/render.rs112 mixes only the dry song voice.
Legacy OrbitDelay allocates boxes and belongs to legacy orbits; it cannot be
constructed on the callback or borrowed by a song branch.

Carve full Room, all private FX and four-second stereo delay from the actual
PrivateFx lease's BusSlot.mem. Distinct generations retain separate full-key regions
through tails and acknowledgment pressure. Delay returns join that private branch
before the post-FX gain gate and before track sum. This plan supplies delay storage
and execution; detector causality, event seed, host preparation and full playback
remain separate parent requirements.

**Source authorized** by ROOT0373; independent verification remains pending.
Historical Ready proposal restriction:
Root must authorize an exact held manifest and coordinate the serial fixture plan
before compilation. No new compatibility mode, zero-send bypass, guessed memory,
legacy orbit reuse, wire tag, dependency or mutable public memory alias is permitted.

## Manifest and module status

```json
{
  "planId": "SONG-PRIVATE-DELAY",
  "planPath": "impl-plans/active/song-mode-private-delay.md",
  "dependsOn": ["SONG-09P", "SONG-09"],
  "writePaths": [
    "src/dsp/bus.rs",
    "src/dsp/bus/song_runtime.rs",
    "src/dsp/bus/song_delay.rs",
    "src/dsp/engine/render.rs",
    "src/dsp/engine/song_runtime.rs",
    "src/dsp/engine/song_runtime/delay.rs",
    "tests/song_private_delay.rs",
    "tests/song_private_delay/rig.rs",
    "impl-plans/active/song-mode-private-delay.md"
  ]
}
```

| Module | Deliverable | Status |
|---|---|---|
|bus.rs|constructor-owned state and scratch, child declaration|SCOPED_VERIFIED|
|bus/song_runtime.rs|identical Native/Arena strict adoption, reset/return integration|SCOPED_VERIFIED|
|bus/song_delay.rs|checked physical layout and allocation-free delay kernel|SCOPED_VERIFIED|
|engine/render.rs|offset-correct private sends|SCOPED_VERIFIED|
|engine/song_runtime.rs|child declaration and event/render callsites|SCOPED_VERIFIED|
|engine/song_runtime/delay.rs|owner-checked controls/layout facade|SCOPED_VERIFIED|
|tests/song_private_delay.rs|real nonzero audio, fit, overlap and pressure proof|SCOPED_VERIFIED|
|tests/song_private_delay/rig.rs|bounded actual Native/ByteInbox fixture|SCOPED_VERIFIED|

Eight Rust paths; each must stay below1000 lines. engine/song_runtime.rs currently911
requires cohesive delay helper extraction before growth. Do not implicitly edit
prepare.rs, carriers, arenas or existing fixture files. Their public interfaces
already convey full-key PrivateFx leases, control IDs38/39/40 and measured capacities.
New testbinary registers its helper by explicit path. New helper methods are
implemented in the declared child, keeping parent growth bounded.

## Exact declaration contracts

Existing imports: BusSlot, BusTemplate, BusGraph, DelayLine, SongFrameRegion,
SongLeaseKey, SongRejectCode, SongLimits, Engine, Ctl, CellRead and CapabilitySet.
No mutable sample/region ownership is exported.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongPrivateDelayLayout {
    pub owner: SongLeaseKey,
    pub room: SongFrameRegion,
    pub chain_frames: u64,
    pub left: SongFrameRegion,
    pub right: SongFrameRegion,
    pub storage_frames: u64,
    pub send_frames_per_channel: u32,
}
#[derive(Debug)]
pub(crate) struct SongPrivateDelay {
    pub(crate) owner: Option<SongLeaseKey>,
    pub(crate) left: DelayLine,
    pub(crate) right: DelayLine,
    pub(crate) time: f32,
    pub(crate) feedback: f32,
    pub(crate) send_l: Box<[f32]>,
    pub(crate) send_r: Box<[f32]>,
}
```

BusSlot owns this state, constructed with its fixed max_block. Scratch buffers are
real constructor allocations, initialized before any processing. They are configured
scratch (2*max_block floats per bus slot), not part of the declared delay region or
invented free bus_frames. Preserve actual configured region reporting; do not add
scratch to reported allocatable bus memory. Existing output/FX scratch likewise
remains separate from the physical mem pool. Record the new exact constructor-byte
cost in tests and plan evidence, including checked scratch length multiplication before allocation. The infallible
constructor follows the existing BusSlot allocation boundary and receives the
already validated fixed max_block; it does not introduce fallible propagation
through unowned Engine::allocate or change allocation-failure semantics.

```rust
impl SongPrivateDelay {
    pub(crate) fn new(max_block: usize) -> Self;
    pub(crate) fn reset(&mut self, owner: SongLeaseKey, mem: &mut [f32])
        -> Result<(), SongRejectCode>;
}
impl BusSlot {
    pub(crate) fn song_private_layout(
        &self, owner: SongLeaseKey, slot_index: u32,
    ) -> Result<SongPrivateDelayLayout, SongRejectCode>;
    pub(crate) fn set_song_delay(
        &mut self, owner: SongLeaseKey, time: Option<f32>, feedback: Option<f32>,
    ) -> Result<(), SongRejectCode>;
    pub(crate) fn run_song_delay(
        &mut self, owner: SongLeaseKey, first: usize, frames: usize, sr: f32,
    ) -> Result<(), SongRejectCode>;
}
impl Engine {
    pub fn song_private_delay_layout(
        &self, owner: SongLeaseKey,
    ) -> Result<SongPrivateDelayLayout, SongRejectCode>;
}
```

The public layout is a copied inspection DTO, checked against the actual full key
and physical slot. It cannot reset, mutate or expose PCM buffers. Reexport its type
through the already-owned bus facade. Engine child supplies the getter without a
new parent engine path. ROOT0368 requires Engine/BusGraph to enumerate the actual
full-key physical slot, check its index conversion to u32, and pass that slot_index
to BusSlot::song_private_layout. Every copied SongFrameRegion must carry this
authentic physical index and match the real region inventory. No invented zero,
sentinel index or new BusSlot constructor field is permitted. Native/Arena tests
compare copied Room/left/right regions against actual slot positions, including
a nonzero physical slot and reuse under a distinct full key.
Checked full-key lookups reject foreign kinds, stale epoch,
generation and re-used physical slots; no resource.id-only authority.

### Physical adoption and region invariants

Engine.allocate134 constructs bus_mem=secs(cfg.bus_seconds)+granular::effect_mem_len,
and BusGraph::new420 allocates that complete amount to EACH BusSlot. EngineConfig's
current10-second default is per physical slot, not divided among bus_slots. Do not
increase default budgets from an aggregate misunderstanding. A plain default slot
may already fit; prove its actual returned region and full Room+FX+delay requirement.
No assumed96-second or other guessed nominal is acceptable.

At sample rate sr, each channel needs D=4*sr+4 floats with checked integer sizing.
ROOT0366 requires the shared strict PrivateFx layout helper to reject nonfinite,
nonpositive, unsupported-range, fractional or nonrepresentable actual sr BEFORE
any slot mutation. The actual rate must equal exactly its supported u32
representation; checked conversion and arithmetic precede carving. Native returns
the exact original Box; Arena/reset return Capacity on this geometry rejection.
A real fractional EngineConfig fixture must prove unchanged measured slots and
Native Box pointer identity. This private guard does not repair the general
fractional host clock/capacity-report mismatch; that remains a parent obligation.
PrivateFx adoption requires fullRoom+sum(full effect memory)+2D in one actual slot;
no aggregate free-frame substitution, clamped carve, shortened line or empty-delay
success. DelayLine::carve clamps, so validate checked extents first and then verify
each returned offset/length is exact. Room, each effect and both delay lines are
disjoint and contained in the same owned slot. Native Box and Arena template paths
share one sizing/layout contract before mutation; all validation failures preserve
original graph ownership and physical capacity. No extra leased slot is selected.

Preserve route pricing max(4Room,Room+chain)+2D as a conservative admission bound;
dedicated strict private configuration carves exactly R+sumFi+2D, where R is the
full Room state and Fi is each full chain effect. It does not invoke the legacy
total/4 Room clipping configuration for PrivateFx. Native, Arena and full-key reset
share this sizing and rejection-before-mutation helper. Track/Master and legacy configure paths retain
their established sizing/state behavior. Only PrivateFx acquires branch delay.
Native/Arena graph preparation and reset must use the same strict private layout,
not legacy configure's clipped Room total/4 or clipped chain cursor.

An actual admitted host route must fit both its measured total and each required
private slot. Preparation can never promise a nonzero delay using only aggregate
bus_frames. Current measured capacity DTO layout remains unchanged. Any discovered
missing actual per-slot proof is reported before expansion to another source path.

### Controls, render and lifetime

Resolve delay send38 through the existing voice control path; it retains the
existing0..1 clamp. Resolve delaytime39 and delayfeedback40 against the owned graph
cell view at actual admitted event time. Preserve legacy OrbitDelay defaults .25s
and .5 feedback, existing0..4s and0..0.95 feedback kernel clamp; finite invalid inputs
receive explicit diagnostics consistent with the current song event contract.
One generation owns time/feedback; event ordering at equal frames remains the
established control FIFO. No signal/cell from another epoch is consulted.

Dry and send gains follow the current stereo/aux/pan behavior, including event off
within the callback. Clear only current send range; accumulate all generation voice
sends before delay execution. Run once per sample at the private post-FX return
junction, before its final branch gate and track sum; dry FX order stays unchanged.
Delay state continues with zero sends through tail until the exact deadline. Never
reset on users==0, voice end or caller partition. Legacy orbit inputs/output and
legacy seed sequence remain unchanged.

Hard mute clears only the matching generation's Room/FX/delay and voices before
future onsets, including line cursors and sends; unmute admits future events only.
Preserve the already-tested post-FX64-frame gate. Rebind/reset cannot clear a foreign
slot re-used after partial retirement. Cancellation/garbage/ACK pressure retains
actual leases/templates until existing exact retirement evidence permits reuse.
No callback allocation, deallocation, VM, graph compilation, pool growth or locks.


**Ordering frozen by ROOT0362; Rust release remains separate.** Accepted design
routing/tails131–135 orders the final branch gate before track/master and requires
private Room/delay state, but does not specify whether declared private FX process
the delayed return. Actual legacy render.rs209–225 sends panned voice output into
both bus and OrbitDelay, then149–151 adds OrbitDelay return to the master slot before
legacy bus rendering. Thus legacy return bypasses the source bus Room/FX; master FX
still process it. Proposed restricted-branch position is: voice output splits to
dry Room/private FX and private delay input; sum delayed return with processed dry
AFTER private Room/FX, then apply final branch gain gate, then track FX and master FX.
This preserves source-private-FX bypass while moving ownership/gating into the branch.
ROOT0362 approves this exact legacy parallel-send branch-owned junction as the
authorized implementation contract. This document update does not release source.
Add a genuine private-gain0/delay-send1 delayed-impulse fixture proving return bypass,
a nonzero track-gain/compressor twin proving track/master still process it, and a
hardmute witness suppressing both dry and delayed output. A finite/nonzero equality
alone does not prove this routing policy. This frozen ordering must remain
sample-exact under1/17/64 partitions and preserve independent full-key tails.

## Genuine test declarations

```rust
fn native_and_arena_nonzero_private_delay_is_audible_from_measured_regions();
fn private_delay_full_extent_one_short_and_fragmented_fit_are_honest();
fn overlapping_generations_keep_private_tail_and_legacy_orbit_isolation();
fn exact_frame_delay_cells_and_partitioned_audio_match();
fn delayed_return_runs_through_track_and_master_after_private_fx();
fn hard_mute_reset_and_partial_return_never_touch_foreign_full_keys();
fn real_delay_render_retirement_and_ack_pressure_allocate_and_deallocate_zero();
```

Use actual queried/routed mono events and real oscillator/impulse/sample instruments.
Native and actual graph bytes→ByteInbox→Engine must produce ResourceReady/Ready/
Applied/retirement receipts; no forged state. Assert silence before activation,
finite genuinely nonzero delayed audio at nonzero delaytime, compare dry-only twin
and sibling/legacy twins, and independently inspect returned physical layout.
Exercise max4s, feedback0/nonzero, off+k and1/17/64 callback partitions. Tail evidence
must extend past voice end, not merely contain dry oscillator output.

Memory exact/one-short and fragmented aggregate witnesses must keep their negative
geometry and original error/ownership assertions. Fixed real constructor geometry
is chosen from full requirement and measured outputs, not a fabricated capacity
report. Allocation/deallocation probes surround actual render/reset/retirement,
with meaningful nonzero output and real full ACK/garbage pressure.

## Dependencies and serial execution

|Dependency|Requirement|Status|
|---|---|---|
|09P provider|actual native/Arena graph ownership and physical pools|verified prerequisite|
|09 runtime baseline|private branch/full-key/post-FX gate/tail paths|verified increments; full parent incomplete|
|neutral stages|actual empty private stage is still strict and fully delayed|author-active; join only once held|
|fixture migration plan|existing genuine positive PrivateFx rigs need full regions|document companion; serial source handoff required|

Implementation may precede full parent09 acceptance as a bounded prerequisite;
final parent acceptance requires this and the other pending DSP contracts. Root must
release exact manifests serially (no duplicated bus/song_runtime ownership).
Fixture plan edits only its own tests after production declares the actual sizing
contract; combined stable checker validates both before any phase clearance.

## Tasks

### TASK-001: Actual physical layout and constructor ownership
**Status**: Completed
**Parallelizable**: No
**Deliverables**: bus.rs, bus/song_delay.rs, bus/song_runtime.rs.
- [x] Checked full extent and real fixed scratch allocated before processing.
- [x] Identical Native/Arena nonclipped private adoption and copied full-key layout.
- [x] Failure atomicity, region isolation, safe reset/return and all files<1000.

### TASK-002: Controls, private render and lifecycle integration
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No
**Deliverables**: engine/render.rs, engine/song_runtime.rs, child delay.rs.
- [x] Actual nonzero sends, owned controls, correct offsets and private return junction.
- [x] End/tail/mute/physical reset/ACK ownership correct; legacy behavior unchanged.
- [x] Narrow same-owner logical Release/Rebind integration verified by ROOT0400 public9, private physicalusers and companion queue7 witnesses.
- [ ] Broader distinct-pool A→B→A, simultaneous positive-tail overlap and final-generation adversarial reuse remain in the separate In Progress child.
- [x] No allocation/deallocation/VM in callback and no new capability bypass.

### TASK-003: Genuine test witness and joined independent seal
**Status**: Completed
**Depends On**: TASK-002 and fixture plan held migration
**Parallelizable**: No
**Deliverables**: new public testbinary and fixture child.
- [x] Actual Native/Arena positive and negative memory/audio/ownership fixtures pass.
- [x] Legacy unaffected scopes and migrated original assertions remain green.
- [x] Mandatory native/host-wasm/strictClippy, fresh exact inventories/runs and unique
  supplemental nextest, scopedfmt/diff/lines/prepost current hashes, terminal handles.

## Completion criteria

- [x] Every private generation owns actual full stereo4s memory, Room and full FX.
- [x] Real nonzero delayed audio and honest exact/one-short/fragmented physical fit.
- [x] Full-key tails/mute/physical reset/pressure and callbackzeroalloc/dealloc verified.
- [x] Actual narrow same-owner logical transitions preserve delay controls, clear buffers and retain family/resource identity.
- [ ] Broader pooled lifecycle criteria remain dependent on the separate In Progress reuse plan.
- [x] No legacy orbit changes, guessed capacity, zero-delay bypass or new dependencies.
- [x] Eight Rust modules/three tasks bounded; combined fixture review independently green.
- [x] Scoped completion does not imply detector/seed/full song playback completion.

## Related plans and progress

**Previous**: [Resource staging](song-mode-resource-staging.md).
**Next**: [Private-delay fixture migration](song-mode-private-delay-fixtures.md), then
remaining parent[DSP routing](song-mode-dsp-routing.md).

### 2026-10-02 — document-only Ready
Read accepted private state/tail design and actual bus adoption/event/render paths.
Eight future source paths declared; companion migration audit distinguishes genuine
PrivateFx adoption from Track/Master/transport-only appearances. New two plans only,
immutable intent052 and after053; no Rust or Cargo executed. Root review and exact
subsequent source release required. Cargo quiet via CARGO_TERM_QUIET=true and mise;
nextest fail-only variables apply. No Git/index/archive or existing plan changes.

### 2026-10-02 — root ordering decision frozen, implementation pending
ROOT0362 SHAa5a57a432e5d559d69678382d7a92e3f77312c9a6daba43a74f8383b464eaf12
approves parallel dry Room/privateFX and private delay send, summing return after
private FX and before finalgate/track/master. Gain0/send1 delayedreturn, downstream
processing and hardmute oracles are mandatory. Ready remains a bounded document
status; every implementation criterion unchecked. Intent056/after057 retain exact
two document hashes. No Rust/Cargo/otherplan mutation or source authorization.

### 2026-10-02 — constructor and actual-rate decision frozen
ROOT0366 freezes infallible checked constructor sizing, dedicated strict
R+sumFi+2D physical carving and pre-mutation supported integer-rate rejection.
Conservative route pricing and ROOT0362 ordering are unchanged. Exact Native Box
retention/fractional geometry negative witnesses are required. The general
fractional host clock/report gap remains pending outside this manifest. Receipts
058/059 cover these two documents only; eight production and two companion Rust
paths remain unchanged, implementation unchecked and Ready. No Rust/Cargo ran.

### 2026-10-02 — physical slot identity declaration frozen
ROOT0368 supplies the checked actual slot_index to the internal BusSlot layout
helper; the public Engine getter stays unchanged. Copied region indices must match
the real physical inventory, with no invented zero/sentinel or constructor field
expansion. Receipts060/061 preserve exact two-document scope and unchanged8+2
manifests. Ready/unchecked implementation and separate source release remain.
The fractional Native environment guard is being authored separately; its parent
verification remains pending, without a claim that this document repairs it.

## Author implementation declarations — 2026-10-02

Exact additional private helpers before Rust, within ROOT0373 eight-path release:

```rust
impl BusSlot {
    pub(super) fn private_need(
        template: &BusTemplate, sr: f32, caps: &CapabilitySet,
    ) -> Result<(usize, usize, usize), SongRejectCode>;
    pub(super) fn configure_song_private<C: CellRead + ?Sized>(
        &mut self, owner: SongLeaseKey, template: &BusTemplate, cells: &C,
        sr: f32, caps: &CapabilitySet,
    ) -> Result<(), SongRejectCode>;
}
impl Engine {
    pub(in crate::dsp::engine) fn validate_private_delay_controls<C: CellRead + ?Sized>(
        audio: &crate::host::wire::AudioEvent, cells: &C,
    ) -> Result<(Option<f32>, Option<f32>), SongRejectCode>;
}
```

private_need returns full Room, summed full FX, and one channel delay length, all
checked and u32-addressable before mutation. Scratch clearing integrates existing
BusGraph::clear range; native/Arena/reset call same strict configuration. Control
validation occurs before voice/state mutation, even with a full voice pool; actual
set occurs after finding a free voice and before the infallible Voice.start call. Existing fullkey/reset semantics retained.
Source work In Progress; no tests/Cargo run. Exact receipts retain parent histories.

### Genuine private-delay fixture helper declarations

```rust
fn key(id: u32, kind: SongResourceKind) -> SongLeaseKey;
fn effect(kind: EffectKind, name: &str, value: f32) -> EffectSpec;
fn private_need(caps: &CapabilitySet, sr: f32, template: &BusTemplate) -> usize;
struct Probe;
unsafe impl std::alloc::GlobalAlloc for Probe;
impl Rig {
    fn new(bytes: bool, ack_slots: usize, sr: f32, bus_seconds: f32) -> Self;
    fn prepared(bytes: bool, private_gain: f32, track_gain: f32, master_gain: f32) -> Self;
    fn command(&mut self, command: SongCommand);
    fn process(&mut self, frames: usize, drain: bool) -> Vec<f32>;
    fn drain(&mut self);
    fn graph(&mut self, lease: SongLeaseKey, inst: Option<&InstDef>, bus: Option<&BusDef>);
    fn event(&mut self, branch: u32, frame: u64, send: f32, time: Ctl, feedback: Ctl);
    fn activate(&mut self, frame: u64);
    fn audio(&mut self, frames: usize, partition: usize) -> Vec<f32>;
}
```

Rig uses actual sample/graph Native or ByteInbox ingress, producer-owned receipts,
measured regions, sole PCM/Box owners, real seal/activation, callback TLS probe.
Every test keeps setup/allocation/result assertions outside the armed callback.

### Admission/reset and retained-owner fixture refinement

Exact existing signatures unchanged. Supported rates are integral 8000..192000.
All finite delay controls/full-key geometry/template/free-voice checks precede
delay mutation; fallible set completes before Voice.start, no error follows start.
Same-owner reset preserves time/feedback; new owner restores default controls.
Additional exact private test declarations (no production API expansion):
```rust
fn private_kernel_max_time_reset_and_foreign_owner_are_exact();
fn malformed_delay_controls_do_not_start_or_modify_a_voice();
```
Rig stores original native bus/sample allocation addresses as usize outside callback,
and records returned owner addresses outside callback; no extra retained Arc clone.

### Retained-owner and ingress fixture declarations
```rust
struct Rig {
    baseline: SongHostCapacities,
    native_owners: Vec<(SongLeaseKey, usize)>,
    returned_owners: Vec<(SongLeaseKey, usize)>,
}
fn arena_private_delay_rejects_short_actual_regions_without_adoption();
fn same_frame_delay_controls_obey_fifo_and_partition_invariance();
```
Fields extend the already declared Rig, not a second type. Addresses are copied
before transferring sole Box/Arc owners. Every returned owner is inspected only
after callback probe disarm; full-key + pointer exact-once and actual restoration
are asserted. Arena rejection uses actual ByteInbox graph ingress with measured
individual region shortage, no synthetic geometry.

### Source-ready author checkpoint (ROOT0373)
All eight production Rust paths and both companion fixture paths are written. No
Cargo/check/test/clippy commands ran in this author wave. Scoped rustfmt is the
only verification performed; mandatory joined checker must establish behavior.
Physical strict layout: R + sum(FX) + 2*(4*sr+4), supported integral 8000..192000.
No preparation pricing change; Track/Master retain established sizing. Constructor
scratch is 2*max_block*sizeof(f32) per physical bus slot, checked before allocation;
public layout reports actual per-channel scratch and physical slot index.
Ten genuine public fixtures + one private kernel fixture are written. They cover
Native/Arena PCM, gain0 return routing, track/master gains, finite controls/Cells,
FIFO exact-frame and partitioned rendering, strict physical shortage/fractional
rate, distinct branch/resource generations and tails, hardmute/reset, full-key
rejection, ACK/garbage pressure, sole owner pointer/receipt restoration and armed
callback zero allocations/deallocations. Original DSP assertions are preserved;
only positive PrivateFx fixture allocation is migrated to measured full memory.
These are written assertions, not passed results. No song playback completion claim.

Declaration process note: downstream Gain oracle was declared in ordering prose,
but its exact test fn signature was omitted before first Rust write. Receipt0008
records this deviation; current declarations now match written code, not a backdated
intent. Behavior/pass claims await independent checks.

### ROOT0379 fixture semantics repair — declarations before Rust

Original002/76862 terminal101: native/wasm/strictClippy0; newdelay10 had4PASS/6FAIL.
Actual log `/tmp/vactr-delay-capture-independent-002/delay-tests.log` remains retained.
Four audio failures are erroneous linear interpretation of Gain's dB parameter;
Arena shortage ignored actual granular pool/capture memory; malformed Arena encoder
correctly rejected nonfinite command before ingress. No production correction is
indicated by this cohort. No behavior result from repaired fixtures exists yet.
```rust
fn linear_gain_chain(gain: f32) -> Box<[EffectSpec]>;
impl Rig {
    pub fn malformed_event(&mut self, branch: u32, frame: u64,
        send: f32, time: f32, feedback: f32);
}
```
Exact linearzero uses actual Mute on1; positive factors use prim::gain_to_db.
All zero-dry/delayed-audibility/FIFO/sibling PCM/1/8 ratio oracles stay intact.
Arena physical shortage uses actual Delay FX extent in the unchanged configured
small Engine; all-slot-short/aggregate-fit/ResourceReady-absent assertions remain.
Malformed native POD must produce actual Malformed ACK. Arena first proves encoder0
and unchanged inbox, then finite genuine encoding has the identified used Const
word replaced by original nonfinite bits; standalone encoded AudioEvent proves
payload offset before mutation. Actual ByteInbox refusal increments by1, callback
PCM remains exactly the sibling reference; no fabricated Arena Song ACK.

### ROOT0379 repaired fixture cohort written and held
The declared two helpers are written; no production/companion/capture sources changed.
Zero private dry uses real Mute, unity Gain0dB, positive downstream factors converted
through the existing primitive. Original exact dry-zero, delayed audible, first-delay,
FIFO, 1/8 ratio, hardmute sibling and partition assertions are retained.
Arena short-region graph now includes actual Delay FX memory; all measured shortage,
aggregate fit, unchanged physical capacity/regions and no ResourceReady assertions
remain intact. Nonfinite Native POD gets actual Engine Malformed ACK; Arena proves
encoder0/unchanged queue, checks genuine finite AudioEvent payload before exact Const
bit mutation, then asserts actual ByteInbox refused+1 and identical sibling PCM.
Scoped rustfmt and rustfmt --check exit0; no Cargo commands. All behavior claims
remain pending fresh mandatory joined checker. Retained002 actual4PASS/6FAIL evidence
is not overwritten or reclassified as a passing gate.

### ROOT0385 independently accepted actual evidence
Original2957 terminal0 (poll429178):50 gates,906 distinct Cargo names plus4 privacy
names=910 distinct; inventories equal actual raw names and all94 hashes unchanged.
Unique supplemental nextest originalc3278d terminal0 repeated17 named fixtures.
Evidence: `/tmp/vactr-delay-capture-independent-004/final-results.json` SHA
bab20d86e601908c53cb16fcff9bda6c292c48157893109461bf3a1afe402394;
root acceptance `tmp/song-mode-riela/ROOT0385-delay-capture-scoped-acceptance.json`
records nextest final SHAfa369ad54b74e5ea61bf9e7f2a0e9c0a2f677b0f4e296aa157ee612fdef80575.
Native/host-wasm/strictalltargetClippy/scopedfmt/diff/maxlines passed. Actualdelay10,
originalaudio16 (14 original+2 migration), fullDSP649 including private kernel and
original owner-bank analyzer, plus all admitted regression scopes passed.
Failures001/002/003 and fixture/declaration audit deviations remain immutable history.
No author Cargo commands were used; mandatory specialized checker supplied evidence.
Parent detector causality, deterministic event seed, application/preparation ownership,
logical branch reuse, complete closure copy accounting, playback/export remain unchecked.

Scoped delay production and its ten fixtures are verified. Plan remains In Progress
only for its literal logical rebind integration criterion; same-owner physical reset
is proven, but ROOT0385 explicitly leaves pooled logical reuse unfinished. This is
not a waiver or claim that new Rebind commands exist.

### ROOT0402 logical same-owner evidence reconciliation
ROOT0400 independently accepts67 gates/1030 actual distinct names and18 nextest
selectedPASS/0selectedSKIP: nine public reuse fixtures, private physicalusers guard
and companion queue7. Actual logical same-owner Rebind now exists and passes
delayed-return/retainedcontrol/historyclear/noalloc/ownership/ordering witnesses.
The preceding ROOT0385 statement is historical: physical-only evidence then lacked
Rebind; this later cohort supplies its narrow proof. Broader distinct-pool A→B→A,
simultaneous positive-tail reuse and final-generation runtime audits stay unchecked
in the reuse child. This plan remains In Progress for that broader dependency; no
parent detector/seed/application/owner/scheduler/playback/export completion claim.
ROOT0400 matrix SHAf34310087d469cdf8de2edadf2d68000cc25ddc8c50e81b2ea40812855c8953e.
No Rust or author Cargo changes; exact docs only, held after reconciliation.
