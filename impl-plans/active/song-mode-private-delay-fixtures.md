# Private delay positive fixture migration implementation plan

**Status**: Completed
**Plan ID**: SONG-PRIVATE-DELAY-FIXTURES
**Created / Last Updated**: 2026-10-02
**Session target**: 1–2 sessions
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Intent and actual audited footprint

Migrate genuine positive PrivateFx staging rigs to actual full Room+FX+stereo4s
physical regions before the new production strict adoption contract is checked.
Preserve all original audio/ownership/partition/pressure expectations. This is a
serial companion of[song-mode-private-delay.md](song-mode-private-delay.md), not a
new zero-send compatibility mode or weakening of genuine short-memory rejections.

Historical READ-ONLY audit source051 and intent052 incorrectly reported the only existing explicit positive
PrivateFx graph adoption rig in tests/song_dsp.rs97: bus_seconds=.25. It actually
installs PrivateFx keys3/4 and optional13 at276–289 through Native and ByteInbox;
all lifecycle fixtures reuse that parent rig. Actual two-channel4-second delay alone
requires8sr+8 floats per private slot; .25 seconds cannot meet that contract.

|Observed path/configuration|Actual role|Write migration required?|
|---|---|---|
|tests/song_dsp.rs97 .25s|positive actual PrivateFx Native/Arena adoption|Yes|
|tests/song_dsp/lifecycle.rs|uses parent rig, no own geometry allocation|No; run all original assertions|
|tests/song_resource_staging.rs93 .1s|actual graph fixtures use Track|No|
|tests/song_resource_staging/acceptance.rs and live.rs|Track/Master actual graph ownership|No|
|tests/song_resource_capacity.rs81 .1s|adoption loops Instrument/Track|No|
|src/dsp/bus/song_runtime.rs295 BusGraph8192|reuse fixture Track/Master keys301–302,328|No|
|src/dsp/bus/occupancy.rs128 BusGraph1024|legacy occupancy/claims; no positive PrivateFx graph|No|
|tests/song_resource_carriers.rs655 PrivateFx|payload validity/tag coverage; actual graph adoptionTrack|No|
|src/host/native/audio/song.rs339 PrivateFx|host enqueue/full queue Box retention, no Engine adoption|No|
|tests/song_graph_banks.rs44/101/114|POD roundtrip/parity, no PrivateFx installation|No|
|tests/song_hosts.rs and song_host_clock.rs small slots|actual sample/Track transport/clock, noPrivateFx graph|No|

Do not bulk increase every .1/.25 configuration found by text search. Keep Track/
Master/legacy behavior unchanged in production so unrelated limited-memory tests
remain real negative/positive witnesses. Inventory is an inspected current snapshot;
root must reconcile any new neutral author source before release. If a genuine
additional PrivateFx positive appears, report exact path first and obtain bounded
prior manifest amendment rather than broad implicit fixture authority.

Engine.allocate134 defines each slot's mem as secs(bus_seconds)+granular effect
memory; BusGraph::new420 allocates the complete value PER slot, not an aggregate
split. Current default10 seconds is per slot and is not changed by this plan. Never
substitute bus_slots*seconds or96 illustrative seconds for actual measured proof.

**Historical proposal restriction**: the initial Ready document did not authorize
Rust/Cargo. Subsequent exact serial releases authorized the migration; ROOT0385
accepted its combined held evidence. This completion update authorizes no source work.

## Manifest and module status

```json
{
  "planId": "SONG-PRIVATE-DELAY-FIXTURES",
  "planPath": "impl-plans/active/song-mode-private-delay-fixtures.md",
  "dependsOn": ["SONG-PRIVATE-DELAY"],
  "writePaths": [
    "tests/song_dsp.rs",
    "tests/song_dsp/delay_geometry.rs",
    "src/dsp/song.rs",
    "impl-plans/active/song-mode-private-delay-fixtures.md"
  ]
}
```

|Module|Deliverable|Status|
|---|---|---|
|tests/song_dsp.rs|explicit child registration and actual positive Rig geometry|VERIFIED|
|tests/song_dsp/delay_geometry.rs|checked sizing/inspection helper and migration tests|VERIFIED|
|src/dsp/song.rs CFGTEST only|actual private analyzer positive geometry/layout|VERIFIED|

Three Rust modules after ROOT0382 prior amendment; all touched files<1000. Parent currently799 must retain all
14 existing audio fixtures and registrations. Put cohesive sizing/inspection into
the child before parent growth; no edits to lifecycle.rs or production here. This
manifest is disjoint from the production eight paths. Serial dependency is required
because the helper consumes the new actual inspected delay layout contract.

## Exact declarations and contracts

Imports are existing EngineConfig, Engine, CapabilitySet, BusTemplate, EffectKind,
SongLeaseKey and SongPrivateDelayLayout from the production plan.

```rust
fn configure_private_fixture_regions(
    config: &mut EngineConfig, caps: &CapabilitySet, sample_rate: u32,
) -> Result<(), String>;
fn assert_actual_private_fixture_layout(
    engine: &Engine, key: SongLeaseKey, template: &BusTemplate,
) -> SongPrivateDelayLayout;
fn migrated_original_private_rig_uses_real_full_regions();
fn original_unrelated_small_track_and_master_contract_stays_supported();
```

Size the rig from actual full Room/its largest actual private FX definition and
2*(4sr+4), using checked floats/bytes conversions. Configure an honestly allocated
per-slot amount before Engine construction. Fractional f32 EngineConfig seconds
must round up safely; do not claim nominal seconds equal exact region floats.
The new copied actual layout and Engine.song_remaining_capacities are authoritative:
assert total storage contains all full disjoint Room/FX/delay regions; assert exact
left/right4sr+4 lengths and slot identity. Track/Master slots remain distinct and
never borrow a private region. Scratch allocations are constructor-owned, not
manufactured free capacity. All staged leases/Ready/Applied receipts are real.

Capture actual measured capacities before reserve, after silent adoption, during
retirement pressure and after full-key return. Derive any changed numerical capacity
assertions from those real snapshots/regions and the new fixed memory requirement;
retain inequalities, exact charged leases and failure codes. Never replace an
assertion with merely finite/silent output or a greater artificial availability.

Parent Rig positive PrivateFx resources all receive full memory, even zero delay
send events; the test is not an alternate old mode. Nonzero execution belongs to
the production plan's actual new fixtures, and must pass before combined completion.
The original hardmute/tail/partition/oracle tests keep their exact original audio
expectations, Room histories and meaningful nonzero output. Independent larger
physical storage must not transfer effect state or change normalization/gains.

The original zeroalloc/dealloc callback probe remains around real processing.
Constructor allocations occur outside it. Existing full ACK/garbage, frame0 rejection,
voice/sample ownership and reused foreign-slot proofs remain intact.

## Tasks

### TASK-001: Exact positive-rig sizing and serial baseline
**Status**: Completed
**Parallelizable**: No; production layout API must be held first.
**Deliverables**: parent registration and child sizing helper.
- [x] Reconcile genuine adoption footprint and immutable exact amended three-path baseline.
- [x] Compute checked full per-slot requirement, configure before constructor.
- [x] No default-engine change, arbitrary nominal budget or unrelated fixture mutation.

### TASK-002: Preserve original proofs and inspect actual regions
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No; same parent rig.
**Deliverables**: actual inspected layout helper and migration fixture declarations.
- [x] Original14 audio/lifecycle tests unchanged in semantic assertions and pass.
- [x] Real Native/Arena PrivateFx full-key layout and measured capacity proof.
- [x] Original unrelated Track/Master/small-memory/negative cases remain unmodified.

### TASK-003: Combined independent verification
**Status**: Completed
**Depends On**: TASK-002 and production held source.
**Parallelizable**: No; final joined cohort.
- [x] Native/host-wasm and strictalltargetClippy green, actual qualified nonempty
  original audio/private delay/staging/capacity/bus/legacy inventories and tests.
- [x] Preserve genuine one-short/fragmentation/rejection cases; no failure waiver.
- [x] Actual nonzero private delay tests from production and original callback
  allocation/deallocation probe pass; unique supplemental nextest if required.
- [x] Scopedfmt/diff/maxlines and exact combined pre/post/current source hashes;
  original process handles polled terminal and every failure retained.

## Dependencies and related plans

|Dependency|Output|State|
|---|---|---|
|production private delay|exact physical layout inspection and strictadoption|scoped independently verified|
|original DSP baseline|14 genuine audio/lifecycle fixtures|verified current baseline|
|neutral stage author|new genuinePrivateFx fixtures use measured full geometry|accepted joined dependency; wider owner remains unfinished|

**Previous**: [Private branch delay](song-mode-private-delay.md).
**Next**: wider[DSP routing](song-mode-dsp-routing.md) acceptance.
Implementation prerequisites do not require claiming full parent09 complete; final
parent remains subject to independent real delay, detectors, seeds and transport.

## Completion criteria

- [ ] Exactly audited positive PrivateFx rig migrates to actual full regions.
- [ ] All original positive and negative assertions remain meaningful and green.
- [ ] Production genuine nonzero delay and private tail proofs independently pass.
- [ ] No zero-send compatibility bypass/defaultbudgetbump/guessed capacity.
- [ ] Two bounded modules/three tasks, no unowned source expansion or dependencies.

## Progress log

### 2026-10-02 — document-only Ready
Read actual PrivateFx occurrences and inspected true adoption versus transport,
Track/Master and payload-only contexts. .1s and8192 occurrences are not automatically
private-delay migrations. Only parent actualPrivateFx rig needs currentsource edits;
new child prevents1000-line growth. Intent052 and after053 retain audit. No Rust,
Cargo, existingplan/index/Git/archive mutation. Future Cargo uses mise with
CARGO_TERM_QUIET=true; nextest uses prescribed fail-onlyvariables. Exact root source
release and joined checker remain mandatory; no current delay acceptance claim.

### 2026-10-02 — companion ordering decision frozen
ROOT0362 freezes legacy parallel-send branch-owned ordering in the production plan.
Migration retains original dry audio assertions and must not route delay through
private FX or bypass finalgate/track/master. Genuine gain0/send1 delayedreturn and
hardmute oracles remain mandatory production fixtures. Ready/incomplete status is
unchanged; no source/Cargo execution. New immutable056/057 receipts cover only these
two owned plans; neutral repair and all other plans/Rust remain separately owned.

### 2026-10-02 — constructor/rate companion contract frozen
ROOT0366 reconciles the production constructor with the existing infallible
BusSlot boundary and fixes strict PrivateFx carving to full R+sumFi+2D while
retaining conservative admission pricing. Companion positives use supported exact
integer actual rates. Production negatives must include a real fractional-rate
configuration with unchanged measured slots and exact rejected Native Box pointer;
no fixture may hide rejection with zero sends or weaken existing dry assertions.
The general fractional host clock/report gap remains a parent obligation.
Receipts058/059 preserve exact two-document scope; companion two Rust paths remain
unchanged, Ready and implementation unchecked. No Rust or Cargo execution.

### 2026-10-02 — truthful copied physical slot witness required
ROOT0368 requires the production internal layout helper to receive the checked
actual physical slot index from a full-key enumerated lookup. Companion inspection
must compare every copied region index against the genuine physical inventory,
including a nonzero slot and foreign-key reuse; no made-up zero/sentinel index.
The public Engine getter and exact8+2 manifests are unchanged. Receipts060/061
hold these two documents only. Implementation remains unchecked; no Rust/Cargo.

### ROOT0373 source author checkpoint — verification pending
Original song_dsp test/assertion bodies are preserved. Only positive Rig bus_seconds
setup now derives actual empty-private full Room+2D demand at 48k, with constructor
headroom; Track/Master alone continue fitting genuine 8192-frame regions.
Child registration and two actual Native/Arena layout/unchanged unrelated sizing
fixtures are written. Public copied indexes compare genuine nonzero physical slots;
full delay lines are each192004 frames and sends16 frames/channel.
No Cargo/test execution in this author wave; scoped rustfmt completed. Mandatory
joined checker must prove original fixtures and new assertions before completion.

### ROOT0382 prior companion manifest amendment — actual omitted analyzer fixture

Independent003 original65564 terminal101: native/wasm/strictClippy0; new delay10 and
capture6 passed. FullDSP648PASS/1FAIL at actual_private_analyzer adoption.
The initial audit omitted `src/dsp/song.rs::tests` actual PrivateFx key2 graph.
This is an authentic original audit omission; do not backdate discovery or authority.
ROOT0382 authorizes only its CFGTEST memory setup/assertions and this companion plan.
Existing original analyzer/private-cell/owner-bank/no-legacy-alias assertions remain
unchanged. Source prefix before `#[cfg(test)] mod tests` must remain byte-identical
SHA290fd40468dac6916e64f69c557d63e238dad2183ff4398968dd7ee20d7bc557.
Existing test signature (no new helper or production API):
```rust
fn actual_private_analyzer_writes_owner_bank_without_legacy_alias();
```
Within this fixture at real48000/caps: checked D=4*rate+4, need=Room+2D;
actual constructor memory includes granular::effect_mem_len. Configure only the
remaining positive demand plus one-frame rounding headroom via bus_seconds; prove
an actual free physical region fits before adopting the empty PrivateFx. Afterwards
copied owner/physicalslot/Room/exactstereo lines/scratch are compared to real storage.
Only src/dsp/song.rs CFGTEST may change in this wave; existing companion tests and
all delay/capture production remain held. No Cargo or behavior claim yet.

### ROOT0382 analyzer fixture source ready — independent checks pending
Checked full Room+2D demand is derived at real48000/caps; only uncovered demand above
actual granular constructor frames, plus one rounding frame, becomes bus_seconds.
Before empty-private adoption, maintained individual free regions must fit. Copied
layout owner and actual nonzero physical slot match, Room/chain0/exactD/offsets/sends16
and complete storage match the original actual free region snapshot. No assumed
floating seconds roundtrip storage value is used as the oracle.
All original analyzer bank/control/private-cell/noLegacyalias/payload/audio assertions
are intact; production prefix SHA remains290fd404... unchanged. Companion originals,
green publicdelay/capture and every other source/plan remain held. Scoped rustfmt/check0;
no author Cargo or behavior execution. Initial source051 audit omission is preserved
historically; fresh mandatory full checker must prove the repair.

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

All companion migration criteria are proven; this plan is Completed in-place under
doc-only authorization. No index/archive operation or parent completion is implied.
