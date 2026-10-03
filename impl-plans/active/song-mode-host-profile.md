# Song host capacity profile

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Song playback and capacities](../../design-docs/specs/design-song-mode.md)

## Purpose

Select actual bounded production Native/browser capacities after the measured
original generated-parts score, without changing generic Engine defaults or
reducing voice/granular/FX budgets. This adapter phase depends on the separately
planned [heterogeneous bus memory layout](song-mode-bus-memory-layout.md).
The later adapter source release implemented this profile; focused evidence is
recorded below. Future mutations require coordinated source authorization.

## Source-derived measurement

Original PreparedSong, real Session::new prelude, Native actual configured host
and actual CapacityReport/ResourceReady/SealReady were exercised independently:
`/tmp/vactr-bootstrap-handoff-checkpoint-002.log`, original60711 terminal0.
The genuine measurement test and separate handoff3 tests passed.

At8000Hz, measurement host has64 buses,128 templates,16 voices:

| Field | After actual bootstrap | Score required | After actual Ready |
|---|---:|---:|---:|
| templates |55|22|33|
| buses |63|25|38|
| voice slots |16|16|16|
| voice frames |512000|288|512000|
| bus frames |21672000|1527976|13072000|
| cells |1024|0|1024|
| sample slots |256|0|256|
| PCM bytes |67108864|0|67108864|
| ACK slots |8192|49|8192|

There are11 logical branches,22 physical pools,2 tracks,1 master and47 resource
leases. Every branch reserves2 generations for positive2second tails across
placements. Dependency families include Builtin and Instrument identities;
merging those based solely on equal resolved instruments is not proved.
No family/music/generation reduction is proposed.

Two unchanged examples require50 song buses plus the legacy master:51 physical
buses. Actual bootstrap occupies73 templates; two examples need44 more, so128
physical template slots leave11 after both. Control/analyzer/sample demand for
this score is zero; this is not a bound for arbitrary scores.

## Exact adapter manifest

Five Rust paths; parent-owned TS evidence is separate:

- `src/host/mod.rs`: register shared validated profile.
- NEW `src/host/song_profile.rs`: constructor profile helper.
- `src/host/native/audio.rs`: stereo/quad device opening selects actual profile.
- `src/host/wasm/worklet_half.rs`: init validates profile before replacing Worklet.
- `tests/song_host_profile.rs`: genuine bootstrap/owners/refusal/audio witnesses.

Parent968/worklet523/test227 lines currently. Stop for a prior split before1000.
Generic Engine bus6/template96 and compatibility headless constructors stay
unchanged; headless_with_config permits the identical production allocation
profile without claiming a device was opened.

## Exact declarations

```rust
pub const SONG_BUS_SLOTS: usize; //51
pub const SONG_TEMPLATE_SLOTS: usize; //128
pub fn song_engine_config(sample_rate: f32, max_block: usize,
    capabilities: CapabilitySet, store: StoreKind, output_channels: u8)
    -> Result<EngineConfig, ConfigError>;
```

Helper selects51/128 and six full-budget +45 bounded song regions through the
layout prerequisite. Preserve all other state budgets. Reject unsupported
integral8000..192000 rate/channels/block/config before allocation/replacement.
Actual capabilities remain explicit; Native uses actual CPAL rate and block512,
worklet actual rate/block128/Arena/voices. Wire/ABI and generic defaults unchanged.

## Memory proposal at48000Hz

The superseded uniform51 allocation adds349920000 Native /155736000 browser
state bytes over six slots. Six additional channel/send/snapshot slices per slot
add552960 Native /138240 browser bytes: known totals350472960 /155874240.
These are an **unapproved high-memory fallback**, not chosen architecture.

Proposed small region:4*Room +2*(4*rate+4) +rate floats. Room at48000 is7118
floats, so460480 floats/1841920 bytes per added slot.45 extra regions add82886400
state bytes on either tier; with six scratch slices known totals83439360 Native /
83024640 browser. Retain six original full regions. Metadata/FxState/template
arrays/allocator overhead excluded. Exact Rust effect calculations govern sizing;
new layout tests must assert actual sizes. Larger graphs use available full
regions or truthfully refuse before old ownership/fade changes.

## Tasks and completion

TASK-001 profile construction depends on verified layout; TASK-002 Native/Arena
original-owner evidence; TASK-003 fresh artifact/browser actual capacities.

- [x] Real production profile51 buses/128 templates with validated heterogeneous layout.
- [x] Generic defaults, capabilities and full legacy region budgets unchanged.
- [x] Authentic bootstrap leaves55 templates and50 free buses.
- [x] Two unchanged original generated scores each reach actual Ready:25 buses/22 templates each.
- [x] One-below measured capacity refuses next while old nonzero audio/ownership survives.
- [x] Exact cleanup restores measured original capacity/owners.
- [ ] Fresh actual browser ABI proves full capacity vectors and genuine overlap preparation.
- [ ] Independent native/wasm/format/strict checks pass.

## Progress

Historical initial13/free12/six-demand estimates are superseded by actual25-bus
measurement; none remain executable completion criteria. Original standalone
measurement1 passed; bootstrap full-vector measurement1 and handoff3 passed.
Historical measurement checkpoint was SONG-HOST-PROFILE/0008. Current adapter
Rust is held at SONG-HOST-PROFILE/0017; production profile is implemented.
Full AtomicApply, generic score capacity, scheduler and overall Song completion
remain separate, unwaived scopes.

Layout evidence: independent public4 PASS original10179 terminal0 and private1
PASS40329 terminal0, raw bus-layout logs retained. This is scoped evidence, not
strict/full regression or overall completion. Adapter source wave authorized.
Tests may retain a self-contained portable Engine/ByteInbox fixture in existing
song_host_profile.rs (no extra path), using the production song_engine_config
for both actual Native and Arena owners. Existing original source/music, full
bootstrap, capacity/cleanup/PCM assertions retained. New exact test declarations:
fn profile_preserves_default_memory_and_validates_before_construction();
fn configured_native_profile_is_actual_and_headless_compatibility_is_unchanged();
Prior measured uniform64 fixture is superseded by actual production51 profile
measurement; original generated24cycles/25buses/22templates remains asserted.

Fixture implementation refinement before writes: self-contained original-owner
portable Rig (same verified ByteInbox/Native render pattern) in the existing test
path uses actual song_engine_config for both transports, without extra paths.
Retain original generated candidate/duration/demand assertions; replace nominal
uniform64 measurement with production51 actual before/after/cleanup measurement,
including both original epochs and real prelude. Exact public test names:
measure_original_generated_parts_ready_peak_bus_demand();
profile_preserves_default_memory_and_validates_before_construction();
configured_native_profile_is_actual_and_headless_compatibility_is_unchanged();
profile_small_stage_preserves_full_region_and_exact_box_owner();
profile_one_below_refuses_next_and_preserves_old_audio();
Existing verified raw uniform64 measurement remains historical immutable evidence.

Adapter fixture checkpoint: five public fixtures written in existing test path,
including actual production helper Native/Arena two-original bootstrap/Ready,
one-bus-short old-audio preservation, original generated duration24cycles,
exact50/55 ->25/33 ->0/11 observed bus/template vectors and original full-key
restoration, constructor invalid inputs and generic Native headless5free/96template
compatibility. TLS armed callbacks assert both zero allocation and deallocation.
Native parent969, worklet530, sharedhelper38; scopedformatcheck0. Production
coherent checkpoint compiled in focused Native DSP retry6247 (4pass) and fresh
purewasm20596 terminal0. Independent profile public5 PASS original86152/0,
all0017 source hashes unchanged; raw /tmp/vactr-host-profile-complete-001.log
SHA84de672b31e2d87222b7314119b1c74966acea93352452ba0c0f8a89483fa3d8.
Root actual browser5 PASS against fresh artifact fc5c9ee0, including real
73-template bootstrap/free55 templates/free50 buses, full-width request nonce and
invalid fractional reinitialization retaining pending capacity report/currentclock.
No physical Native device opening claim. StrictClippy/fulljoinedgate remains pending;
browser two-replacement/controller acceptance remains distinct and incomplete.
