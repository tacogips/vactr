# Live command isolation during finite song ownership

**Status**: In Progress
**Created**: 2026-10-03
**Design Reference**: [Timing and randomness](../../design-docs/specs/design-song-mode.md#timing-and-randomness), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)

## Purpose

Reject live tempo/clock changes and independently running legacy slots or
one-shots while finite song resources are owned. Preserve legacy behavior once
all song ownership is released. Existing candidate validation checks mixing in
submitted code, but does not guard later incremental evaluation.

## Manifest and ownership

Four Rust paths; no dependencies, protocol changes or guessed capacity limits:

- `src/sched/runtime/song.rs`: controller owner adds a read-only ownership guard.
- `src/sched/runtime.rs`: coding owner rejects legacy binding at dispatch/bind.
- `src/sched/oneshot.rs`: coding owner rejects tempo/clock changes and one-shots.
- `tests/song_live_isolation.rs`: genuine Session/Native ownership fixtures.

The controller author retains exclusive ownership of song.rs. The guard is
provided serially before the coding owner writes its consumers. All production
sources must be held before independent checks. Keep each Rust file below 1000
lines; register additional split paths in this plan before editing if necessary.

## Deliverable contracts

Runtime exposes a crate-private boolean ownership query that includes actual
preparation, Ready, active/replacement owners and outstanding retirement. A
historical Ended/Failed display value alone is not current resource ownership.

Before mutating clocks, adding legacy slots, or scheduling immediate/deferred
legacy work, return an explicit capability diagnostic while the guard is true.
Rejections preserve actual song PCM, frozen settings/catalog and owned keys.
Existing Apply and instrument mute remain available through their song APIs.
After cleanup, the same legacy commands retain their existing semantics.

## Tasks

### TASK-001: Read-only ownership guard

**Status**: In Progress
**Parallelizable**: No; controller author alone writes song.rs.

- [x] Guard uses actual owned state, not stale published transport state.
- [x] Include preparing, Ready, replacement and retirement phases.

### TASK-002: Guard command consumers

**Status**: In Progress
**Parallelizable**: No; depends on TASK-001 declaration.

- [x] Reject Bpm/Cycle/Clock changes while song owns resources.
- [x] Reject reverse legacy slot/one-shot/scheduled thunk mixing.
- [ ] Preserve outside-song legacy behavior and song Apply/mute.

### TASK-003: Independent acceptance

**Status**: In Progress
**Parallelizable**: No; requires held sources.

- [ ] Real Session/Native fixture observes Applied before forbidden live edits.
- [ ] Rejections leave frozen timing, playback/catalog and exact ownership intact.
- [ ] Same commands work after authoritative finite cleanup.
- [ ] Existing legacy and all song tests plus native/WASM strict gates pass.

## Progress log

### 2026-10-03 — read-only completion audit

Source review finds Runtime.apply unconditionally dispatches Tempo and SlotBind;
tempo and one_shot accept changes without song ownership checks. Starting a Song
rejects existing legacy slots, so the missing guard is the reverse direction.
This plan closes explicit accepted design requirements; no implementation or
passing execution is claimed by plan creation.

### Consumer declarations before source implementation

Actual guard supplied by controller author: crate-private
`Runtime::owns_song_resources(&self)->bool`, true for actual preparing/Ready/
nonempty owners, excluding stale display state. song.rs remains exclusively
owned by that author. Consumer guard runs at bind entry and one_shot entry,
before dry-run/sample requests/slot/AtQueue or clock mutations. tempo refuses
Bpm/Cycle/Clock through FailCode::BeyondCapability; MidiClockOut changes only
outgoing synchronization preference and stays allowed. No live input/follow
clock admission is thereby granted. Tests use genuine ApplySong/Applied/PCM,
ordinary Eval commands, actual natural full-key cleanup, then legacy commands.

### Source-ready genuine fixture checkpoint

Controller author supplied song.rs890 immutable guard. Coding consumer sources
runtime850/oneshot235 are held and compiled in independent controller10/sample6/
privateapply4 checkpoint; those existing20 tests passed with these exact sources.
New `actual_song_ownership_rejects_live_clock_and_legacy_work_until_retirement`
uses two actual Session/Native pairs and original ApplySong candidates. It
checks preparing refusal, actual Applied and frozen catalog, Bpm/Cycle/Clock,
SlotBind, immediate once and deferred at refusal, exact twin PCM through Playing/
Draining, retained original epoch/catalog, output-only MidiClockOut permission,
actual Ended full-key cleanup, and restored ordinary commands while historical
Ended remains displayed. No fake ownership state or ACK. This new fixture is
written but not yet executed. Scoped formatting passes; every file below1000.
All production consumer hashes remain unchanged while the fixture was written;
no author Cargo, dependencies or unrelated source edits.
