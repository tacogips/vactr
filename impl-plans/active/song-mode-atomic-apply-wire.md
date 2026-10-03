# Atomic Song Apply wire contract

**Status**: In Progress
**Created**: 2026-10-03
**Updated**: 2026-10-03

## Design references

- [Implementation review](../../design-docs/specs/design-song-mode.md#implementation-review-2026-10-03)
- [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Scope

This additive wire phase carries an explicit old epoch, original new activation
and staged initial mute overlay identity. It implements no DSP transaction,
resource adoption, overlay remapping, fade, rollback or successful acknowledgment.
Those remain mandatory serial implementation phases. Existing initial Activate,
all command tags 0–19, and all acknowledgment bytes remain unchanged.

## Exact write manifest

| Path | Ownership | Baseline |
|---|---|---|
| `src/song/routing.rs` | PODs, command variants, epoch extraction, structural validation | 614 lines; `6a336c5d070e807d8a7c3898e2f01e5d91856d0f1b2f6b35ed2c4152d89218d1` |
| `src/host/caps/song.rs` | Existing streaming codec | 632 lines; `596239aa6b93e904b16ba7272c80da59ee5bcef9a472ffb6619cf38f164edc25` |
| `tests/song_atomic_apply_codec.rs` | Actual public CtlMsg codec fixtures | New |

Only this plan may also change. Three Rust modules; each must remain below 1000
lines. No runtime, scheduler, owner, protocol ACK or dependency changes.

## Declarations

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongReplacement {
    pub activation: SongActivation,
    pub previous: SnapshotEpoch,
    pub overlay_nonce: u64,
    pub overlay_count: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongInitialMute {
    pub epoch: SnapshotEpoch,
    pub overlay_nonce: u64,
    pub instrument: u32,
    pub muted: bool,
}
// Additive SongCommand variants:
// Replace(SongReplacement)
// PrimeMute(SongInitialMute)
// Existing signatures retained:
// SongCommand::epoch(self) -> SnapshotEpoch
// SongCommand::valid(self) -> bool
```

Replacement epoch is activation.epoch; previous is the exact caller-carried old
snapshot epoch, never inferred from active state. Epoch zero remains valid as in
existing Activate. Replacement requires different old/new epochs, nonzero nonce,
activation frame at least 64 and representable activation.frame + 64. This
reserves the future old closing/new opening ramps without implementing them.
Count spans full u32; zero count with a nonzero nonce supports an empty overlay.
PrimeMute requires a nonzero nonce and an exact boolean byte. Instrument spans
full u32, interpreted only after authentic family resolution in a future phase.

## Codec contract

Append command tags 20 (Replace) and 21 (PrimeMute) under existing 0x1B carrier.
Common prefix is tag plus kind plus little-endian u64 new epoch.
Replace payload: u64 activation frame, u64 previous epoch, u64 overlay nonce,
u32 overlay count; total 38 bytes. PrimeMute payload: u64 overlay nonce,
u32 instrument, u8 muted; total 23 bytes. Existing maximum record storage fits
both; no maximum or ring enlargement. Decode returns the prefix consumed length
and leaves concatenated bytes available for the next message. Truncation,
unknown tags, malformed booleans and structurally invalid values reject.
Native validation and byte decoding use the same SongCommand::valid contract.

Duplicate families, missing staged records, foreign nonce tables, exact physical
owners, Ready status and resource limits require future bounded DSP preflight.
Portable validation must not pretend to prove those cross-record properties.
Engine's existing wildcard NotReady rejection remains until that phase.

## Exhaustive caller audit

`SongCommand::epoch` in routing and the codec's command tag/payload matches are
exhaustive and updated here. Audited command sites: engine.rs, dsp/song.rs,
engine/song.rs, engine/song_queue.rs and its tests, engine/song_runtime.rs,
sched/song.rs, sched/runtime/song.rs, native/audio.rs, native/audio/song.rs,
native/audio/song_capacity.rs, wasm/messages/song.rs, preparation.rs and
preparation/ledger.rs. Their command matches have fallback arms, selective
patterns or let destructuring. No additional source is required for compilation.
Unimplemented new records reach the actual staging wildcard NotReady response;
no successful transaction or overlay receipt is introduced.

## Tasks

### TASK-001: Typed additive commands

**Status**: Completed
**Parallelizable**: No
**Deliverables**: routing.rs

- [x] Exact PODs, epoch extraction and structural refusal implemented.
- [x] No old POD/wire change; no heap-owned command payload.

### TASK-002: Streaming codec

**Status**: Completed
**Parallelizable**: No (depends on TASK-001)
**Deliverables**: host/caps/song.rs

- [x] New tags encode/decode exact full-width values and prefix lengths.
- [x] Invalid native POD and malformed bytes refuse identically.
- [x] Old tags and acknowledgment encoding remain unchanged.

### TASK-003: Genuine codec witnesses

**Status**: In Progress
**Parallelizable**: No (depends on TASK-002)
**Deliverables**: tests/song_atomic_apply_codec.rs

- [x] Replacement/overlay byte oracles, full-width boundaries and epoch zero.
- [x] Invalid frames, same epochs, zero nonce, bool, tag, truncation and short output.
- [x] Old command tags 0–19 byte fixtures and concatenated decode consumption.
- [x] Scoped formatting and independent focused verification pass.
- [ ] Independent strict all-target Clippy and fresh wasm build complete.

## Progress log

### 2026-10-03: Audited phase authorization

Root authorized the plan and audited three-file source scope. Immutable prior
intent: `tmp/song-mode-riela/SONG-ATOMIC-APPLY-WIRE/0001-before.json`.
No Cargo or tests executed by author. Full atomic DSP transaction, muted overlay
certification/remapping, exact cycle selection, rollback, fade, Browser playback
and complete goal verification remain pending.

## Verification handoff

Dedicated checker: native check plus strict all-target Clippy and
`cargo test --test song_atomic_apply_codec` and existing
`cargo test --test song_activation_codec` / `cargo test --test song_host_clock`.
Use mise and CARGO_TERM_QUIET=true. Author only performs scoped rustfmt.

### 2026-10-03: Source held for independent verification

Implemented the three declared Rust paths. New record lengths are 38 and 23;
existing maximum capacity is unchanged. Engine, host owner, scheduler and ACK
encoding were not edited. Scoped rustfmt and scoped rustfmt --check exited 0.
No Cargo, Clippy or test execution occurred in the author wave. Fixture bodies
are written, but behavioral acceptance remains unchecked until dedicated review.

Written fixture names:

- replacement_and_initial_overlay_have_exact_full_width_byte_layouts
- malformed_intents_refuse_native_and_byte_admission
- new_records_preserve_prefix_consumption_and_refuse_all_short_buffers
- original_twenty_command_tags_keep_golden_payloads

The codec's empty-overlay count remains a structural intent, not proof of an
installed overlay table. No new acknowledgment or runtime acceptance was added.

### 2026-10-03: Independent focused wire acceptance

Root independently reports native check exit 0 and terminal codec checkpoint
45088 exit 0, chunk 4911e8, `/tmp/vactr-clock-wire-codecs-001.log`:
all four new atomic wire fixtures, all four existing activation codec fixtures
and all seven existing host clock fixtures passed. The separate actual clock
child's six fixtures also passed in that checkpoint. These are wire/clock
results; they establish no DSP replacement behavior. Fresh pure wasm build
10656 remains live at this entry and is not recorded as passed. No author Cargo.
The prepared-owner/DSP/controller transactions remain separately incomplete.
