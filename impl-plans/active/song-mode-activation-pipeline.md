# Activation pipeline and exact empty-song lifecycle implementation plan

**Plan ID**: SONG-ACTIVATION-PIPELINE
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export), [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Scope and related plans

Implement every actual production activation-failure carrier, authenticated
initial-event pipeline and exact empty/subframe endpoints. All requirements of
[activation correlation](song-mode-activation-correlation.md) remain mandatory.

- **Previous / Depends on**: [Activation wire](song-mode-activation-wire.md), [Host preparation](song-mode-host-preparation.md).
- **Next**: [Transport core](song-mode-transport-core.md), [Runtime/session integration](song-mode-transport-integration.md).

| Dependency | Required output | Current status |
|---|---|---|
| Activation wire | Exact carrier, old-tag compatibility and forwarding | Four codec and19 carrier tests passed in focused verification |
| Host preparation | Verified original owner and real cleanup ledger | All10 owner tests passed, including actual pressure and cancellation |
| Actual ranked queue | Same-frame admission/execution and lifecycle identity | Source reviewed; corrections pending |

## Exact future Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/dsp/song.rs` | Actual queue ordering for initial Activate and same-epoch endpoint | In Progress |
| `src/dsp/engine/song_queue.rs` | Projected order consistency and correlated intake failure | In Progress |
| `src/dsp/engine/song_runtime.rs` | Correlated executed/retained outcome and exact empty endpoint handling | In Progress |
| `src/host/caps/song/preparation.rs` | Once-only activation, authenticated pipeline and exact receipt ownership | In Progress |
| `tests/song_host_preparation.rs` | Update genuine owner correlation and cleanup obligations | In Progress |
| `tests/song_dsp.rs` | Preserve/update actual timing, rollback and ACK pressure witnesses | In Progress |
| `tests/song_activation_correlation.rs` | Genuine Native/Arena zero-onset, empty/subframe and failure evidence | In Progress |
| `tests/song_dsp/lifecycle.rs` | Existing overdue activation regression matches exact typed request | In Progress |

Eight Rust paths plus own plan progress. All touched sources below1000; any new
exhaustive match or split needs root amendment. Shared preparation/test ownership
is serial after the original host owner cohort. No unlisted ingress reordering
workaround or generic lifecycle framework.

## Existing public declaration contract

```rust
impl SongReadyBundle {
    pub fn submit_activation(&mut self, host: &mut dyn AudioHost,
        activation: SongActivation) -> Result<(), SongCommandRefusal>;
    pub fn receive_activation(&mut self, ack: SongHostAck) -> Result<(), SongHostAck>;
}
```

Keep signatures and original private candidate/resource ownership. One activation
outcome stays pending; authenticated timed events may be submitted after accepted
activation enqueue. Only actual Applied or exact ActivationRejected confirms
that outcome; generic event rejection remains the transport's obligation.

## Required actual behavior

- Cover public Native and ByteInbox intake via Engine::record runtime routing,
  timed activation failure and retained failed outcome under receipt pressure.
  Preserve original requested frame for rejection and actual frame for Applied.
- Private untimed song_record Activate bypass in engine/song.rs cfg(test) is not
  reachable through production Activate intake. Keep its staging-helper generic
  rejection unchanged and do not count it as public correlation evidence.
- Authenticate queued initial Event against genuine Ready branch/full keys;
  render its activation-frame onset exactly. Refusals retain their actual POD.
  No successful enqueue is falsely published Applied; no musical preroll or lost note.
- Wrong epoch/requested frame receipts remain unconsumed. Generic same-epoch
  rejection cannot imply activation never happened or authorize unsafe cancellation.
- Actual ordering lives in dsp/song.rs, not only song_queue's projection helper.
  Initial Activate must precede its same-frame endpoint. Retain established
  Release/Rebind/Event order, end-exclusive onset suppression and old-epoch Apply behavior.
- Empty sequence, Repeat0 and positive rational duration rounding0 are valid
  public cases. Handle nonzero tail and zero tail at exact boundaries; no inactive
  endpoint rejection followed by an indefinitely active empty epoch.
- Distinguish valid endpoint equality at requested application frame from genuinely
  overdue/past-cap activation. Preserve truthful failed Apply and old-active isolation.
- Normal completion retains Master and all exact full-key safe returns through
  actual deadline and callback cleanup. ACK/garbage pressure must retain genuine
  correlated outcomes and cleanup; no frame-only or generic-rejection proof.
- If receipt pressure delays actual activation and makes the first event expired,
  report truthful late/failure outcome, preserving cleanup; do not claim exact music.
- Callback remains allocation/deallocation/VM free and all queues/storage bounded.

## Tasks

### TASK-001: Fresh actual failure/order authority
**Status**: In Progress
**Parallelizable**: No; requires verified wire/owner.
- [x] Record fresh baselines and prove production intake/fallback separation.
- [x] Confirm every actual queue/retained failure site and ranked ordering consumer.

### TASK-002: Pipeline and exact lifecycle integration
**Status**: In Progress
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [ ] Complete all source deliverables and preserve original once-only ownership.
- [ ] Keep projected admission and actual execution order coherent for all boundaries.

### TASK-003: Genuine audio and failure witnesses
**Status**: In Progress
**Depends On**: TASK-002
**Parallelizable**: No; full source hold before mandatory checker.
- [ ] Actual Native/Arena initial onset at musical zero and partition invariance.
- [ ] Admission/execution failures correlated; generic event rejection and stale frame cannot spoof outcome.
- [ ] Authentic owner empty/zero-repeat/subframe songs with zero/nonzero tail complete exactly.
- [ ] Physical ACK/garbage pressure, delayed outcome, failed Apply and safe full-key cleanup.
- [ ] Existing DSP/generation/mute/host regressions, native/wasm/strictClippy and scoped line/format checks.

## Completion criteria

- [ ] Every parent behavior verified in actual production paths with original logs/hashes.
- [ ] Existing old-active rollback and full-key ownership preserved under all outcomes.
- [ ] Full transport now has coherent exact initial onset and finite endpoint authority.
- [ ] Complete progression, Apply/mute, browser playback and streaming export remain required.

## Progress log

### 2026-10-02 — ROOT0538 owning-module split

Read-only audit confirmed actual queue insertion ranks Endpoints before Activate,
so empty/subframe arrangements can reject their endpoint then remain active.
Added the actual sorting owner to the bounded runtime phase. Private fallback
and exhaustive ACK audits identified no further required production source paths.
Planning only; no Rust edits, checker execution or source release.

### 2026-10-02 — ROOT0566 implementation release after focused acceptance

Original focused session43079 exited0: all10 host preparation tests, one private
resource test, four activation codec and19 carrier tests, six privacy cases,
strictClippy and scoped formatting passed. Separate34-file line/format/diff
closeout passed. All931 inputs matched their frozen source; broad018 results
remain valid for unchanged production and successful test paths.

Implement this complete seven-path phase now, then connect finite transport.
The user requested a design/implementation review: repeated broad checks delayed
usable playback. Use focused tests for actual changes and failures; run broader
integration at the complete playback milestone. Preserve the full original scope.
No further planning prerequisite is introduced by this release. Exact first
onset, empty completion, correlation and ownership remain required; sampled
asset permanent-refusal handling remains assigned to its existing follow-up.

### ROOT0566 production declaration before implementation

`SongReadyBundle::submit_initial_command(&mut self, host: &mut dyn AudioHost,
command: SongCommand) -> Result<(), SongCommandRefusal>` permits only exact
same-epoch initial Event/Endpoints after accepted Activate, pending or Applied.
It validates original retained pool generation and full resource keys; refusal
retains the POD. Generic Rejected never establishes activation failure.
`SongRuntime::command_rank` becomes crate-visible shared ordering authority used
by queue projection: Activate0, Release/Endpoints1, Rebind2, Mute3, Event4.
Endpoint equality at actual activation frame is valid; an expired earlier tail
cap rejects before mutation. Original requested activation remains in typed
intake/execution/retained failure receipts. No tests added or executed in this
production turn, per Root's explicit instruction; independent review follows.

Root authorized authentic initial-generation Release in the same pending-command
method: same retained pool/full-key validation, frame>=activation and valid tail
deadline. This preserves short initial components without waiting for Applied.
Root subsequently authorized focused genuine fixtures in released test paths;
execution remains delegated to the independent checker.

Focused fixture declarations before writing: owner
`pending_initial_commands_preserve_onset_and_exact_activation_correlation` and
`empty_repeat_zero_and_subframe_owners_retire_at_exact_initial_endpoint`; DSP
`duplicate_activation_rejection_retains_requested_identity_and_active_audio`.
Reuse genuine existing Native/Arena owner/Engine rigs and callback allocators.
Existing exact initial frame/partition/ACK-pressure fixtures remain unchanged.

New standalone `actual_native_and_byte_intake_preserve_activation_failure_under_ack_pressure`
uses real configured Engine and real Native/ByteInbox RequestCapacity pressure,
then authentic absent-preparation Activate intake. It checks original fullwidth
request correlation after guarded intake resumes; no fabricated Ready or ACK.

### ROOT0566 coherent production and focused fixtures held

Implemented shared ranked ordering and all production activation rejection
carriers in the owned queue/runtime; original requested frames survive intake
and execution errors. Ready consumes exact typed activation failure, returns
generic and foreign-request outcomes untouched, and admits authentic initial
Event/Release/Endpoints while activation is pending. Strictly expired endpoint
cap refuses activation; equal cap applies then closes at the same exact frame.
Existing keyed retirement remains under its authoritative guarded staging pump.

Written focused fixtures cover real Native/Arena pending onset and short release,
empty sequence/Repeat0/subframe endpoint with zero and positive frame tail,
exact returned Master/full keys, duplicate activation preserving old audio, and
actual Native/ByteInbox intake rejection under real capacity-report ACK pressure.
Existing exact onset/partition and callback allocator probes remain in use.
Scoped rustfmt and scoped --check exited0; every owned Rust file is below1000.
No Cargo/nextest executed; independent compilation/behavior review is pending.
Full progression, Apply/mute, dynamic source/geometry, browser and streaming
export obligations remain incomplete. No full-goal acceptance is inferred.

### Focused check: activation receipt regression

The first focused run compiled: correlation passed (1); DSP passed 16 and failed
1 at tests/song_dsp/lifecycle.rs:308. This existing overdue-activation fixture
still expects the old generic Rejected instead of exact ActivationRejected.
Root adds tests/song_dsp/lifecycle.rs as the eighth Rust module in this phase
for the contract update. Preserve actual ACK pressure, absence of Applied and
subsequent preparation reuse assertions; match the original requested frame.
Owner tests were not reached. Log: /tmp/vactr-activation-focused-001.log.

### Focused activation behavior verified

The final owner run passed all 12 tests, including authentic same-callback
onset and zero/empty/subframe normal retirement: /tmp/vactr-activation-owner-drain-001.log.
Correlation passed 1; the 17 DSP tests passed across the initial run (16) and
the exact repaired legacy expectation (1): /tmp/vactr-activation-dsp-repaired-001.log.
The owner fixture now drains actual physical receipts through AudioHost::drain,
which preserves Applied after an internally consumed capacity report without
rendering another callback. This is scoped evidence, not complete song playback.

Consumer integration additionally needs immutable captured sample geometry for
speed-fit. Root releases a small SongReadyBundle::sample_data accessor in the
already owned preparation.rs, returning only its retained closed Upload::Sample
data by authenticated source/lease binding. No loader, new authority or copy.
