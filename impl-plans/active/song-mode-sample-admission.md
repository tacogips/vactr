# Checked song sample sender admission implementation plan

**Plan ID**: SONG-SAMPLE-ADMISSION
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Purpose and dependencies

Make every complete admitted PCM union either progress through actual bounded
transfers or fail truthfully with retained ownership. Legacy submit_song_sample
returns the same Arc for permanent invalid/capacity failures and queue pressure;
current owner retries them identically. Wasm retains at most INBOX_SLOTS sample
metadata entries until actual return, while raw DSP CapacityReport may advertise
more physical sample resources. Do not guess a platform cap or trim the music.

- **Previous / Depends on**: [Original consuming owner](song-mode-host-preparation.md), [Activation wire](song-mode-activation-wire.md), complete joint source hold and verification before shared edits.
- **Consumer**: [Transport core](song-mode-transport-core.md), [Streaming export](song-mode-export-core.md).
- **Independent runtime followup**: [Activation pipeline](song-mode-activation-pipeline.md); shared preparation source remains serial.

| Dependency | Required output | Current status |
|---|---|---|
| Original owner cohort | Verified original candidate, all accepted keys and cancellation | Original owner/playback/pressure independently verified; controller wave active |
| Raw DSP capacity | Actual epoch/serial/free physical dimensions | Existing genuine provider source |
| Sender bounds | Actual Native absence of retained metadata limit and Wasm remaining metadata/bytes | Source audited; checked API missing |

## Exact future Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/host/caps.rs` | Checked sample submission and actual sender-capacity trait hooks | SOURCE_WRITTEN |
| `src/host/caps/song.rs` | Original-Arc refusal and sender-capacity DTOs | SOURCE_WRITTEN |
| `src/host/native/audio.rs` | Thin native trait forwarding preserving legacy submission | SOURCE_WRITTEN |
| `src/host/native/audio/song.rs` | Native valid-install versus actual queue-pressure checks | SOURCE_WRITTEN |
| `src/host/wasm/messages.rs` | Thin Wasm trait/provider forwarding | SOURCE_WRITTEN |
| `src/host/wasm/messages/song.rs` | Actual retained metadata/byte bounds and checked admission | SOURCE_WRITTEN |
| `src/host/caps/song/preparation.rs` | Independent bound observation/admission and typed retry/failure cleanup | SOURCE_WRITTEN |
| `tests/song_sample_admission.rs` | Genuine Native/portable Wasm exact-boundary/pressure/Arc witnesses | SOURCE_WRITTEN |

Eight Rust paths plus own progress. Native parent969 and preparation875 currently
have limited headroom; use cohesive existing child helpers and thin forwarding.
Every touched Rust stays below1000. If source growth requires a split, amend the
manifest/plan before writes; do not silently add a ninth path or omit behavior.
Shared owner/wire/provider sources are edited only after their previous holds.

## Public declaration contract

```rust
pub struct SongSampleRefusal {
    pub lease: SongLeaseKey,
    pub data: Arc<SampleData>,
    pub error: SongSubmitError,
}
pub enum SongSampleSenderCapacity {
    Unbounded,
    Bounded { resources: u32, pcm_bytes: u64 },
}
pub trait AudioHost {
    // All existing methods remain, including Arc-only compatibility.
    fn try_song_sample(&mut self, lease: SongLeaseKey, data: Arc<SampleData>)
        -> Result<(), SongSampleRefusal>;
    fn song_sample_sender_capacity(&self) -> Result<SongSampleSenderCapacity, Failure>;
}
```

Unbounded describes absence of a sender-local retained metadata/PCM budget;
it never means unbounded DSP/store capacity. Default checked submission is
Unavailable retaining original lease/Arc, and default observation unavailable.
No inferred bound from capabilities, graph materialization or platform name.

## Required behavior

- Keep raw Engine CapacityReport epoch/serial/dimensions exactly as measured.
  Observe actual sender constraint separately and combine independently remaining
  sample/PCM constraints before Reserve; do not subtract the same claim twice.
  Observations are non-atomic; actual transfer remains checked.
- Native has no separate retained metadata ceiling. Validate exact sample install
  kind/data/geometry before actual enqueue; invalid is permanent Invalid retaining
  original Arc, and actual control-ring pressure is Backpressure.
- Wasm reports checked INBOX_SLOTS minus actual retained song_reservations, and
  checked arena_bytes minus arena_used/song_bytes. No nominal MAX_RESOURCES-only
  observation can certify more retained samples than the sender supports.
- Kind/data/duplicate/metadata/byte exhaustion returns typed permanent Invalid;
  unsupported returns Unavailable. Accepted Wasm local ownership remains bounded
  and pumps through actual byte-ring pressure. Preserve original Arc identity
  on every refusal; do not copy PCM, silently reload live assets or recompile.
- Legacy Arc-only wrappers delegate to checked operations and return the same
  data on failure. Existing callers/traits continue compiling and behaving honestly.
- Owner retries only typed Backpressure with unchanged upload/cursor/work budget.
  Permanent failure preserves original candidate and cleans every accepted full
  key and outstanding upload obligation. No infinite retry on permanent refusal.
- Final SliceAccepted does not release metadata or PCM charge; actual exact
  LeaseReturned remains retirement authority. Provider-owned slice ACKs remain
  routed correctly without manufacturing ResourceReady.
- Full unique decoded PCM union and fixed IR/table dependencies remain intact.
  Accept arbitrary supported unions when both actual constraints admit them;
  otherwise issue an explicit capacity diagnostic and preserve old active music.

## Tasks

### TASK-001: Actual sender contracts and bounded ownership
**Status**: In Progress
**Parallelizable**: No; shared source follows verified prior holds.
- [ ] Fresh source/design/plan baselines and exact sender/store constraints.
- [ ] Define trait/DTO compatibility and preserve raw physical reports.

### TASK-002: Checked provider and consuming owner integration
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [ ] Complete all provider checks and independent observed-bound admission.
- [ ] Retry/fail/cleanup preserve exact Arc, original candidate and every full key.

### TASK-003: Genuine complete-admission evidence
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; mandatory joined checker after full source hold.
- [ ] Actual INBOX_SLOTS retained samples plus truthful one-over failure despite larger raw DSP report.
- [ ] Raw report serial/dimensions unchanged, refused Arc exact and repeat refusal stable.
- [ ] Native real queuefull then progress, permanent invalid no retry, unsupported explicit failure.
- [ ] Final slice retains charge, exact fullkey return releases it, observation race produces safe cleanup.
- [ ] Original host/codec/sample regression suites and Native/wasm/strictClippy/line/format gates.

## Completion criteria

- [ ] Complete union either progresses or fails truthfully under actual bounds.
- [ ] Every success/failure/cleanup invariant proved with real provider receipts.
- [ ] Original source hashes/process logs preserved; no unexpected dirty-work changes.
- [ ] Full progression, exact activation, Apply/mute, browser playback and export remain required.

## Progress log

### 2026-10-02 — ROOT0542 actual metadata ceiling preflight

Owner author found the actual Wasm retained metadata limit. Specialized audit
confirmed Arc-only permanent/transient ambiguity in both providers and raw DSP
report forwarding. Separate sender observation preserves authentic report data
and avoids guessed caps. Planning only; current small owner fixtures are useful
scoped witnesses, not full arbitrary-asset clearance. No source release yet.

### 2026-10-03 — Source release and current declaration refinement

Parent releases the exact eight Rust paths after pressure12 acceptance. Native
parent969 leaves30 lines, thin trait forwarding only; existing child handles
checks. Actual `remaining_song_sample_limits` is not called and omits the16
retained metadata ceiling. Generic Rejected currently releases epoch sender
charges before actual key returns; retain these charges until exact LeaseReturned.
Use already charged `demand::from_assembly` actual unique sample count/bytes for
independent sender-bound verification before ledger/Reserve, without rewriting
raw report serial/dimensions or double-subtracting reservations. Private owner
prepare now accepts `host:&dyn AudioHost` to observe sender bounds; no other
caller/API changes. No sender observation needed for a zero-PCM union, preserving
source-free unsupported-host compatibility. Native data validation preserves its
actual usize geometry; browser retains its actual u32 byte-transfer limits.
`SongSampleRefusal` derives Debug and preserves lease plus Arc; checked default
Unavailable, legacy wrappers return its original data. Controller7 paths are
disjoint and remain exclusively owned by their author. No Cargo by author.

Fresh baselines recorded before source writes:
- `src/host/caps.rs`: 457 lines; SHA256 `970766e764e792e22c824c275f7645a0dcc42853820f87d6c5ba5f1136784d96`.
- `src/host/caps/song.rs`: 660 lines; SHA256 `83051a0e463012d1f0a462f18771fe5eb0dd2680f7896877f5335bfe06976690`.
- `src/host/native/audio.rs`: 969 lines; SHA256 `3f2f22d03096c5e90974b1efba275d5c30592c466804cc5cb032bb9631afeafc`.
- `src/host/native/audio/song.rs`: 780 lines; SHA256 `e3be3c9c09b1b41dc087732da350894b0e0458779ad1f8639971ae4bb55ff056`.
- `src/host/wasm/messages.rs`: 543 lines; SHA256 `834863b38d83d88f841a3f6c7188cae3e6c77092e5a16f0f5126fd20d32f242a`.
- `src/host/wasm/messages/song.rs`: 338 lines; SHA256 `d61ba3211aac07008b8c9eef6b7f7445e69d0f01cfba1fc813dd099f033850a5`.
- `src/host/caps/song/preparation.rs`: 875 lines; SHA256 `d99559129bd290f8cd426b41ac343cf2bc96b901275afa4275ac60d03f7b23d5`.
- `tests/song_sample_admission.rs`: 0 lines; SHA256 `new`.

### Coherent initial source checkpoint (full fixtures pending)

Typed original-Arc refusal, independent actual Native/Wasm sender observations,
legacy delegation and owner Backpressure-only retry are written. Already charged
complete demand is checked against sender limits before ledger/Reserve. Generic
epoch rejection stops Wasm transfers but retains charge until exact key return.
Native child validates actual geometry/data; parent thin forwarding remains981
lines. Owner903 lines. Initial unsupported/native-permanent fixtures written;
full actual sixteen/one-over/real pressure/slice-charge/race cleanup witnesses
remain pending after this compiler checkpoint. Scoped formatting0; no author
Cargo or behavioral success claim. All eight source paths now held temporarily
for parent controller compile feedback; resume remaining fixtures only after
parent/checker confirms terminal.

### Initial independent fixture feedback

Joined controller/sample compilation succeeded original94323 terminal101, stopped
at controller fixture before sample runs. Separate sample run foregrounddab150
terminated101: unsupported checked/legacy original Arc witness passed; Native
setup failed before admission because quantum16 is below Native MAX_BLOCK512.
Corrected actual Native config to MAX_BLOCK and renamed fixture precisely to
`native_permanent_geometry_refusal_preserves_exact_arc_and_sender_observation`.
Queue pressure proof remains a forthcoming distinct fixture, not claimed here.
Raw `/tmp/vactr-sample-admission-initial-001.log` preserved. Seven production
paths stay fixed. Three external legacy-only wrappers need the parent-owned
Ready sample-forwarding companion; no compatibility fallback hides Unavailable.

### Full fixture release and declarations

Initial2 independently passed foreground7eee17 exit0, raw
`/tmp/vactr-sample-admission-initial-002.log` SHA256
`228e611ded08fd7fc4d92d8482ef119c2c508f1bdda25ceec3e46f81103d8ea5`;
controller7 also passed against these providers. Parent releases full fixture
work and the separately declared forwarding3 plan. Existing sample8 remains
unchanged. New test module embeds actual portable Wasm provider and ABI, a
real ByteInbox/Engine/sample reservation/garbage fixture, and real Native host
queue-fill/drain fixture. A real decoded-factory candidate/owner path will prove
complete-union pre-Reserve refusal and observed-bound race cleanup; no artificial
ResourceReady/LeaseReturned or nominal capability substitutes. Fresh source
baselines are initial0003 held hashes, verified before renewed source writes.

### Full fixture source checkpoint — independent behavior pending

Seven production inputs remain byte-identical to initial0003 during the
controller/libunit run. The sample fixture adds actual portable Wasm ABI /
ByteInbox / configured Engine / garbage-return orchestration, with a mutex
serializing its shared ABI. Real Native owns its ordinary control ring.

Written scenarios:
- Exact16 sender sample claims and repeated original-Arc one-over refusal while
  raw physical capacity advertises more; actual final SliceAccepted and
  ResourceReady retain all16 metadata and32 bytes/sample. An actual unknown
  Mute rejection retains charge; only actual full-key LeaseReturned releases it.
- Fill the real Native control ring with valid Clock requests, observe typed
  Backpressure retaining original lease/Arc, render/drain and retry that PCM to
  actual ResourceReady. Automatic internal capacity queries share this ring,
  so admitted public request count is measured rather than guessed equal1024.
- Original decoded sample-path union16 reaches actual Ready and cancels with
  full-key returns; union17 fails HostUnavailable before resource uploads,
  retaining the original complete PreparedSong.
- Observe capacity16, then accept a separately reserved foreign sample with
  its actual provider; owner16 receives permanent checked refusal, fails once,
  and drains original accepted keys. Foreign resource id200 is distinct from
  the candidate's remapped ids; its independent cleanup restores sender16.

Four new tests plus the two already verified initial tests are written. Scoped
formatting passes; no Cargo executed by author. Complete boundary/race behavior
and affected full wrapper suites remain unchecked until independent checker.

### Independent full fixture feedback / exact catalog repair

Original49818 exited101. Forwarding parent suites passed layout4/owner12/profile5;
sample4 passed actual metadata16/fullkey charge, Native ringpressure, geometry,
and unsupported originalArc checks. Complete union setup failed before intended
owner assertions: declared score paths `./p{i}.wav` differed from factory keys
`p{i}.wav`. Correct only factory keys to the exact declared closed paths. Race
mutex poison was secondary to that panic and remains strict; no recovery masks
failures. Seven production inputs and three forwarding inputs stay unchanged.
No author Cargo; complete union and race require fresh independent six-test run.

### Actual graph capacity isolation repair

Original79547 exited101: controller8 passed, sample4 passed; complete union
failed genuine route admission at measured bus_slots before sample admission.
The original fixture allocated physical12 buses (legacy master leaves11),
templates16, branch slots16. Inventory captures each distinct declared sample
path family; one-shot tail0 reserves one generation each. prepare.rs183–193
requires private count plus track plus master and the same private template
count. Candidate16 therefore needs16 branches/templates and18 free buses;
candidate17 needs17 and19. Arena fixture now allocates physical20 buses
(legacy1+19), templates17 and branches17. Independent limits remain
leases128/cells64/analysis128/ACK128/PCM1000000 and sender INBOX_SLOTS16.
Add strict actual Ready16 branch/pool/template/bus demand assertions;
candidate17 still must fail HostUnavailable at sender admission. Secondary
race mutex poison remains strict. All seven production hashes unchanged;
no author Cargo. A stopped author edit assertion produced no source changes;
receipt0006 records that unchanged state and is superseded by0007 containing
these actual fixture modifications. No behavior claimed before checker.
