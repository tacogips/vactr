# Heterogeneous constructor-owned song bus regions

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Song routing and measured capacities](../../design-docs/specs/design-song-mode.md)
**Dependency**: [Adapter profile](song-mode-host-profile.md) consumes this prerequisite.

## Contract

Decouple added physical song slots from maximum granular state replication using
heterogeneous **constructor-owned** regions. Preserve the original six full
legacy-budget regions, callback no allocation/deallocation, exact effect budgets,
full-key lifetime, actual stage ACKs and safe original-owner refusal. No shared
arena rewrite, dynamic callback allocation or family/generation reduction.

Measured unchanged generated-parts has25 buses,22 pools,47 resources and zero
bus effect-chain memory (all branch effect_template None, neutral tracks/master).
LPF is frozen event/instrument control; it does not introduce a named FX chain.
Positive tail requires2 generations per branch. Two originals plus legacy master
need51 slots; bootstrap73 plus44 song templates requires at least117 templates.
Adapter selects128 separately; this layout does not alter template pricing.

## Exact future Rust manifest

| Path | Current lines | Purpose |
|---|---:|---|
| `src/dsp/ring.rs` |856| Typed optional constructor profile, checked validation; defaultNone |
| `src/dsp/engine.rs` |858| Compute actual heterogeneous allocation before callbacks |
| `src/dsp/bus.rs` |788| Cohesive heterogeneous constructor, legacy eligibility flag |
| `src/dsp/bus/song_runtime.rs` |424| Shared deterministic smallest-fitting Native/Arena stage selection |
| `src/dsp/bus/occupancy.rs` |151| Legacy selection restricted to full regions, preserve claim shielding |
| NEW `tests/song_bus_memory_layout.rs` |0| Real measured geometry/Native/Arena/adoption/pressure/owner/audio probes |

Six paths, no implicit ninth caller. Engine parent is shared with ongoing DSP
work: serialize ownership/allocate block before source release, do not overwrite
other author changes. All source <1000; stop for prior amendment if needed.
Root authorized the six-path source wave; adapters remain separately held.

## Exact proposed declarations (no bodies)

```rust
// ring.rs; additive EngineConfig field song_bus_memory: Option<SongBusMemoryProfile>
#[derive(Clone, Copy, Debug)]
pub struct SongBusMemoryProfile {
    pub full_slots: usize,
    pub small_chain_seconds: f32,
}
// bus.rs: additive private BusSlot field legacy_eligible: bool
pub fn new_with_memory_profile<C: CellRead + ?Sized>(
    n_slots: usize, max_block: usize, full_mem: usize,
    profile: Option<SongBusMemoryProfile>, cells: &C, sr: f32,
    caps: &CapabilitySet) -> Result<BusGraph, ConfigError>;
pub(crate) fn small_song_region_frames(sr: f32, caps: &CapabilitySet,
    chain_seconds: f32) -> Result<usize, ConfigError>;
// song_runtime.rs; sibling helper visibility stays within bus module
fn song_bus_need(key: SongLeaseKey, template: &BusTemplate,
    sr: f32, caps: &CapabilitySet) -> Result<usize, SongRejectCode>;
fn best_song_slot(&self, need: usize) -> Option<usize>;
// private unit/helper and real integration tests
fn default_layout_keeps_every_legacy_region_and_profile_is_checked();
fn native_small_stage_preserves_full_region_and_exact_box_owner();
fn arena_small_stage_preserves_full_region_and_exact_actual_ready();
fn mixed_graphs_and_unadopted_claims_refuse_without_old_mutation();
fn generated_two_bootstrapped_originals_fit_measured_heterogeneous_regions();
fn callback_stage_render_reset_cancel_has_zero_alloc_and_dealloc();
```

Existing BusGraph::new signature/behavior remains compatible and invokes uniform
allocation. EngineConfig::new field defaultsNone. Config profile validation
checks full_slots>=2 and <=bus_slots, finite positive chain seconds and supported
integer rate, checked per-region/aggregate sizes. Preserve old config validation
and actual state ceiling; no unchecked float cast/wrap. Allocation failure obeys
existing constructor boundary; no callback fallbacks.

## Region sizing and selection

Selected adapter profile:full_slots6,small_chain_seconds1.0, total51. Generic
constructor unchanged. With D=4*rate+4, R=actual effects::mem_len(Room,rate,caps),
C=checked one-second float allowance, small=4R+2D+C. This contains full Room,
mandatory independent two private delay lines, protects neutral quarter-room
configuration, and gives a nonzero extra chain allowance. Unchanged example
needs C=0, but this chosen bounded C supports positive-memory small FX. Validate
via actual compiled graph rather than inferring keyword IDs. At48000,R7118,
small460480 floats. Arithmetic must call actual helper, not duplicate rounding.

Private need is exact R+sumFX+2D (existing private_need); neutral track/master
uses full R+sumFX and region>=4R to avoid partial Room semantics. Both Native Box
and Arena POD stage use the SAME computed fit predicate and deterministic
smallest Free region with tie-break slot index. Never consume a large legacy
region for a fitting smaller graph. Region actual mem.len remains authority.
Invalid graph/malformed kind returns old exact owner without mutation.

Legacy install selects only legacy_eligible full-budget slots, including under
unadopted song claims; it must not silently clamp legacy state into added smaller
regions. Song may use a full region for a larger chain when one is Free, subject
to original claim/lifetime restrictions. Legacy full-budget capacity preserved,
not guaranteed simultaneous unlimited legacy and song graphs.

## Reservation/admission semantics

Existing unadopted shielding withholds largest actual Free regions by claim
count; retain conservative behavior. Count/scalar preparation acceptance does
NOT prove arbitrary heterogeneous graph placement. ResourceReady is emitted
only after actual per-graph stage succeeds. A large graph or mixed arrangement
can honestly fail geometry despite aggregateframes; owner retains all keys and
performs real cleanup, old Applied audio unchanged. No fake fit guarantee or
unsafe reuse of promised region. Best-fit adoption must not count staged slots
as Free or charge adopted reservations twice. Current physical regions and
CapacityReport must expose actual total remaining floats after every transition.

## Tasks

TASK-001 typed configuration/constructor (dependency Engine writer hold).
TASK-002 common best-fit and legacy eligibility (depends001).
TASK-003 genuine Native/Arena lifecycle and independent checks (depends002).

## Completion criteria

- [ ] Uniform generic defaults and original six full state regions unchanged.
- [ ] Profile invalid/overflow requests refuse before mutation/allocation.
- [ ] Actual51 geometry has6 original full +45 exact measured small regions.
- [ ] Both stage paths choose smallest fitting actual Free region deterministically.
- [ ] Small graphs preserve full-region capacity for real large effects; large one-short refusal retains exact Box/lease or Arena ownership.
- [ ] Legacy installation never uses added small regions or steals withheld claims.
- [ ] Two bootstrap-real unchanged examples fit actual vectors/Ready; no music/family/tail reduction.
- [ ] Mixed size/unadopted/ACK pressure refusal preserves old audible PCM, exact owners and guarded cleanup.
- [ ] Actual stage/reset/render/retire callbacks zero alloc and zero dealloc.
- [ ] Strict independent native/wasm/full regression/scopedformat pass.

## Progress

Document only. Measurement60711 terminal0 and full-vector raw evidence retained
in host-profile plan. Shared pool architecture is deferred; no memory code or
capability reduction written. Source release and Engine ownership coordination
remain required. This prerequisite does not complete full Song mode.

Source declaration refinement: BusGraph::best_song_slot(&self, need: usize) -> Option<usize>
is an inherent private method shared by both stage implementations.
BusGraph::song_bus_need(key: SongLeaseKey, template: &BusTemplate, sr: f32,
caps: &CapabilitySet) -> Result<usize, SongRejectCode> is an associated private
helper. small_song_region_frames is pub(crate) so checked EngineConfig validation
uses the identical actual effect sizing before allocating.

Exact fixture helper declarations before implementation: struct Rig { host:
Box<dyn AudioHost>, backend: Backend, caps: CapabilitySet, frame: u64, receipts:
Vec<SongHostAck>, last: [f32;32] }; enum Backend Native(Box<AudioSide>) or
Arena owning Engine/ByteInbox/event/ACK/garbage/cell state. Rig::new(bytes:bool,
bus_slots:usize)->Self; Rig::tick(&mut self)->Vec<HostMsg>;
Rig::complete(&mut self, owner:&mut SongHostPreparation)->SongReadyBundle;
Rig::cleanup(&mut self, owner:&mut SongHostPreparation)->();
Rig::capacity(&mut self,epoch:SnapshotEpoch)->SongHostCapacities;
Rig::bootstrap(&mut self)->Session; struct SharedAudio(Rc<RefCell<Box<dyn AudioHost>>>);
fn generated_original(epoch:SnapshotEpoch)->PreparedSong;
fn limits(capabilities:CapabilitySet)->SongPreparationLimits;
fn callback(run:impl FnOnce())->(); global TLS Probe implements GlobalAlloc.
All construction/encoding/ACK collection and destruction outside armed callbacks.
Fractional positive chain budgets round upward to a checked integer float extent;
only sample rate itself must be integral. No profile quota reset.

Compatibility refinement before fit-helper correction: existing uniform neutral
Track/Master slots retain actual quarter-room clamping. New profile small slots
are >=4R by construction and full profile slots retain old full allocation.
Exact private best_song_slot(&self, key: SongLeaseKey, need: usize, sr: f32,
caps: &CapabilitySet) -> Option<usize> evaluates neutral actual per-slot room
min(R, mem.len()/4) +sumFX; PrivateFx uses strict R+sumFX+2D. No universal4R
rejection for existing small uniform tests. Private test declaration:
fn heterogeneous_actual_box_reset_return_and_claim_shielding();

Source checkpoint: TASK001/002 implemented, TASK003 genuine fixtures written
but not executed by author. Profile defaultNone preserves generic6/template96.
Shared Engine allocate patch coordinated with rust_coding and both narrow edits
scoped-formatted at871 lines. Four public tests cover actual geometry/invalid
profiles, Native Box refusal and smallest-fit, two real Session-bootstrap original
Native/Arena Ready epochs and exact restored full-key capacities, and one-bus-short
next-owner refusal preserving real old audible transport before guarded retirement.
Every actual Native/Arena render is armed for zero alloc/zero dealloc. A private
production test exercises reset/Box identity return and actual unadopted claim
shielding. Independent verification is pending; no Cargo author execution.
