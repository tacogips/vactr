# Genuine controller acknowledgment pressure

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Live Apply transaction](../../design-docs/specs/design-song-mode.md#focused-design-and-implementation-reassessment-2026-10-03)
**Dependency**: [Controller integration](song-mode-atomic-apply-controller.md)

## Intent and exact source scope

Prove controller progress and ownership under a genuinely full critical host
acknowledgment ring, in addition to the already passing command pressure fixture.
Retain actual Session, provider, DSP and original ticket behavior. No fake ACK,
capacity override, ignored foreign outcome or production authority change.

| Path | Deliverable |
|---|---|
| `tests/song_apply_controller.rs` | Cohesive extraction of pressure fixtures; real callback/drain controls in existing harness |
| `tests/song_apply_controller/ack_pressure.rs` | Existing command pressure fixture and new actual acknowledgment saturation witness |

The parent is currently 910 lines. Extract its existing command pressure case
before adding harness controls. Both files must remain below 1000 lines. This
followup owns the two pressure paths serially after the controller checkpoint;
it does not enlarge the production controller plan's source manifest.

## Required declarations and evidence

The child uses the original parent Controller/Rig and introduces a real test
`actual_full_ack_ring_preserves_pending_mute_and_replacement`. Harness helpers
may withhold Session/provider receipt draining while rendering genuine callbacks,
then resume actual delivery. Native and Arena rings must both reach their actual
configured critical capacity, established by read-only ring state or a genuine
retained producer outcome. Count every externally injected foreign rejection
under its explicit fixture owner. Forward every Session-owned receipt unchanged.

While pressure is held, mute remains unacknowledged and replacement publication
must not occur. After pressure clears, actual mute acknowledgment precedes overlay
freezing, replacement applies on the exact old clock, audio respects the carried
gate, cleanup completes, and a further Apply remains usable. Preserve the callback
allocation/deallocation probes and all existing command pressure assertions.

## Tasks and completion

1. **Written**: Declare harness/drain helper signatures and capture source baseline.
2. **Written; execution pending**: Specialized author extracts pressure case and implements both real
   backend saturation paths with finite bounds derived from actual capacities.
3. **Pending**: Independent checker executes the whole controller binary and
   existing runtime/handoff/transport regressions after a coherent source hold.

- [ ] Both actual critical rings fill; temporary observations are not called full.
- [ ] All actual outcomes retain their proper owner and exact-once accounting.
- [ ] Pending mute, replacement, audio and subsequent Apply progress are verified.
- [ ] Whole affected suites, formatting and under-1000 line checks pass.

## Progress log

### 2026-10-03 — Root followup declaration

Seven controller fixtures passed. Command-ring pressure is proven; critical
acknowledgment saturation remains a distinct missing witness. This plan permits
a cohesive test split rather than exceeding the parent file limit. No new source
or verification has run for this followup.

## Exact before-source helper declarations

`Rig::tick_with_delivery(&mut self,deliver:bool)` renders the same callback;
false withholds Arena received-ring delivery, not garbage ownership cleanup.
`RecordedHost` retains shared `held:Rc<Cell<bool>>` and
`replay:Rc<RefCell<VecDeque<HostMsg>>>`; held drain returns no receipts.
Released drain forwards original replay+provider records through existing exact
foreign rejection demultiplexing and full ledger, never generates an ACK.
`Controller::snapshot_full_ring(&mut self)->usize` transfers actual Arena ring
records through HostState (or drains Native provider directly), measures the
actual returned count, and retains those exact records for normal dispatcher
replay. Native actual ACK_CAPACITY and Arena actual producer.capacity()/consumer
len prove full before draining. Foreign filler is the same invalid epoch9999
Mute POD with exact once-only Rejected accounting. No clock nonce mutation.
`actual_full_ack_ring_preserves_pending_mute_and_replacement` fills real rings
with 8192 actual bounded admissions/pumped outcomes; issues pending mute and
candidate while receipts are withheld, asserts no Prime/Replace/Muted, then
releases authentic outcomes and verifies mute→overlay→Apply→finite cleanup and
another Apply. Callback probes remain armed only around real render/process.

### Source hold — genuine saturation fixture written

Existing command-pressure body moved intact to the declared child. Both actual
rings are filled by admitted foreign Mute commands and genuine DSP rejections;
Arena measured consumer length equals producer capacity before extraction;
Native public drain returns exactly its configured ACK_CAPACITY. Exact actual
records are retained for normal dispatcher replay. Pending mute/candidate cannot
publish before replay, and full-key reserve/return ledger verifies exactly-once
normal retirement. No passing execution claim yet. All production except the
separately root-authorized shared rounding correction remains held.

### Actual first saturation failure and precise fixture repair

17786 terminal101: old seven tests PASS; new saturation witness failed because
Native public drain filters an internally owned capacity report triggered by
mutating foreign Mute. Physical ring capacity remains8192. No cap-1 inference.
Before refill, render/drain original provider records into preserved replay with
no Session delivery; bounded settling has no new producer mutations. New filler
is nonmutating public RequestCapacity(epoch9999), whose actual CapacityReports
are consumed only by this explicit foreign producer and counted exactly8192.
Existing command-pressure Mute/rejection fixture is unchanged. Public Native
snapshot must still measure8192, Arena still checks actual ring len==capacity.

### Actual corrected saturation execution

Whole controller eight tests PASS in original79547; joined command exit101 came
from two separate sample fixtures, not this binary. Raw
`/tmp/vactr-controller-sample-checkpoint-002.log`
SHA8c4ab534caf4b4977a8bac3969c209bd3bdac618eb8f3f7878efa6820a4a97eb.
Actual full8192 rings, pending mute waiting, actual overlay/replacement, complete
full-key returns, and subsequent Apply all passed. New partial-admission and
permanent-refusal controller regressions are now source-written in this cohesive
child under the separate controller reassessment release; their execution is
pending. No full strict-lint/combined-current-controller claim yet.

### 2026-10-03 — Scoped delegation lint declaration

Actual all-target Clippy original93963 exited101 at helper
admit_controller_command returning inline SongCommandRefusal (520 bytes).
AudioHost::try_song_command intentionally allows result_large_err at caps.rs39
to preserve exact inline refused POD ownership. Apply that documented allowance
only to this fixture delegation helper; no boxing, cloning, reminted command,
assertion or behavioral changes. Source remains the declared child manifest;
independent strict lint and whole controller verification pending.
