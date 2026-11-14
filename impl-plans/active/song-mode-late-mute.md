# Interactive song mute at the actual callback frame

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Live song controls and routing](../../design-docs/specs/design-song-mode.md)

## Purpose

Interactive Mute arriving after its requested frame applies at the next actual
callback frame. This differs from exact musical scheduling: Event, Activate,
Release, Rebind and Endpoints retain their existing strict late rejection.
No JavaScript timestamp becomes audio authority. The queued original Mute POD
is retained unchanged; only its effective queue position is max(requested,now).
Existing mute execution acknowledges its actual frame and uses a64-frame gate.
Existing complete command validity, capacity, full epoch and family validation
remain authoritative. Stale epochs and malformed musical commands still reject.

## Exact future/write manifest

1. `src/dsp/song.rs`: existing `SongRuntime::queue` signature unchanged;
   only Mute's effective frame changes. Add direct production queue unit proof.
2. `tests/song_dsp.rs`: register new test child only; preserve original assertions.
3. NEW `tests/song_dsp/late_mute.rs`: reuse original actual Rig and allocator probe.

```rust
fn late_mute_preserves_original_command_and_other_timed_commands_stay_strict();
fn late_mute_acknowledges_actual_frame_and_has_exact_pcm_ramp();
fn late_unmute_does_not_resume_old_notes_and_stale_epoch_is_refused();
fn activate(rig: &mut Rig, branches: &[u32]);
fn mute(rig: &mut Rig, epoch: SnapshotEpoch, muted: bool, frame: u64);
```

The new public test child is a module of existing song_dsp binary. Native records
and real encoded ByteInbox records both pass Engine. No synthetic successful ACK
or replacement DSP effects. Original prepared three-family fixture provides PCM
sibling reference, reset oscillator reference and actual private bank ownership.
Each actual callback inherits existing zero-allocation/zero-deallocation probe.

## Tasks

| Task | Status | Dependencies |
|---|---|---|
| TASK-001: bounded effective Mute frame | Written, held | Existing Engine timed queue |
| TASK-002: actual late Mute/unmute and unchanged strict commands | Written, held | TASK-001 |
| TASK-003: focused independent check and fresh browser fourth fixture | Pending | Source hold |

## Completion criteria

- [ ] Original queued Mute unchanged; effective actual frame advances only when late.
- [ ] Other timed commands reject past frames and invalid controls unchanged.
- [ ] Native/Arena actual Muted ACK contains actual callback frame.
- [ ] Actual64-frame gate PCM oracle and untouched sibling remain exact.
- [ ] Late unmute cannot resume old notes; foreign epoch rejects with unchanged audio.
- [ ] Callback zero allocation/deallocation and existing fixtures preserved.
- [ ] Independent focused tests, native/wasm build and actual browser mute pass.

## Progress

2026-10-03: actual clock-repaired browser playback3 fixtures pass; fourth actual
liveMute returns Malformed when cached clock/transport latency places requested
frame before callback now. Source queue rejects all past timed commands before
existing actual-frame mute execution. Bounded requested-versus-effective frame
correction leaves exact musical scheduling unchanged. Fresh intent0001 recorded
before source; no author Cargo.

Source checkpoint: original Mute POD retained in SongTimedCommand; effective
frame alone uses max(requested,now). Direct queue unit tests all five strict
musical variants and invalid nonfinite Event unchanged. Two new actual Rig
fixtures run both Native/Arena upload/ACK/callback paths with original allocator
probe, exact ramp/sibling PCM oracle, actual ACK frame, late unmute stop and
foreign epoch refusal. Scoped rustfmt check0. Tests written, not author executed.

Independent first checkpoint: direct queue unit1 PASS; actual late mute64-frame
ramp PASS; late-unmute twin test failed at first fresh note comparison. Raw
/tmp/vactr-late-mute-public-001.log original78802/101. Its sibling-only reference
had never sounded the initial oscillator, leaving different shared track/master
histories. Fixture corrected to three same-initial-note/mute-history twins:
remaining-muted baseline proves no resumed notes; on-time same-frame reference
proves exact late-unmute/new-onset/stale-refusal PCM. No production change or
epsilon relaxation; genuine new voice count and extra audible contribution remain.
