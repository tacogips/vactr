# Configured Native song preparation implementation plan

**Plan ID**: SONG-NATIVE-CONFIGURED-PREPARATION
**Status**: In Progress
**Created**: 2026-10-02
**Last Updated**: 2026-10-02
**Design Reference**: [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export), [Live controls and snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Purpose and dependencies

Provide genuine explicitly configured Native capacity for concurrent old/new song
preparation. Default Engine bus slots6 include one live legacy Master: free5.
Each complete PLAIN candidate retains four song buses, so two require eight free
slots. Retiring old music or narrowing the next music would invalidate refused
Apply isolation. A caller-supplied EngineConfig must allocate actual capacity;
capacity observations and all staging receipts remain measured by the real Engine.

- **Depends on**: [Host preparation](song-mode-host-preparation.md).
- **Related**: [Placement overlap](song-mode-placement-overlap.md).
- **Consumers**: [Transport integration](song-mode-transport-integration.md), [Export](song-mode-export-core.md).

## Exact Rust manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/host/native/audio.rs` | Thin validated configured headless constructor using existing pair | NOT_STARTED |
| `tests/song_hosts.rs` | Genuine configured capacity and invalid configuration witnesses | NOT_STARTED |
| `tests/song_host_preparation.rs` | Native Rig explicitly allocates12 real bus slots matching Arena | NOT_STARTED |

Three Rust files plus this plan and owned host-preparation progress only. Current
line counts941/505/603; every touched source remains below1000. Additional paths
or a split need Root amendment. No other owner/source/plan writes.

## Public declaration contract

```rust
impl NativeAudioHost {
    pub fn headless_with_config(config: EngineConfig, cells: usize)
        -> Result<(Self, AudioSide), Failure>;
}
```

## Required behavior

- Validate actual EngineConfig before allocations and use existing Native pair.
- Require NativeArc storage, exact integral supported sample rate for FrameClock,
  and native fixed render quantum compatibility. Use existing Failure diagnostics.
- Preserve existing headless/headless_quad signatures and behavior. No new
  dependency, callback allocation, synthetic capacity report or bypass of staging.
- Actual custom bus slots are allocated before callback and measured free slots
  exclude real live legacy Master; report epoch/serial/dimensions stay authentic.
- The owner Native Rig uses12 actual bus slots and otherwise preserves capabilities,
  voice limits, clocks, callback memory probe and complete old/new PLAIN music.
- Refused activation still uses genuine filled control/outbox pressure and retained
  exact command; old audio remains audible and old ownership retained.
- Invalid configuration must fail before constructing a host; actual tests cover
  storage, rate and quantum rejection as well as measured custom capacity.

## Tasks

### TASK-001: Native configuration contract
**Status**: In Progress
**Parallelizable**: Yes; disjoint from routing author paths.
- [x] Read actual pair/render geometry and configuration validation contracts.
- [x] Record immutable exact3Rust/plan baselines.

### TASK-002: Constructor and genuine capacity fixture
**Status**: NOT_STARTED
**Depends On**: TASK-001
**Parallelizable**: No within this manifest.
- [x] Implement validated thin constructor and actual error-case fixtures (execution pending).
- [x] Configure complete Native old/new owner fixture without narrowing music.
- [ ] Preserve callback allocation/deallocation and capacity authority.

### TASK-003: Mandatory joined acceptance
**Status**: NOT_STARTED
**Depends On**: TASK-002 and routing source hold.
**Parallelizable**: No; complete source hold through original terminal.
- [ ] Native/wasm/strictClippy, whole song_hosts and owner pressure tests pass.
- [ ] All cohort hashes, actual inventories, full logs and line/format checks reviewed.

## Completion criteria

- [ ] Real configured Native capacity with truthful validation/refusals.
- [ ] Complete old/new Native/Arena owner pressure witnesses pass.
- [ ] Legacy host behavior preserved and callback memory invariant retained.
- [ ] Scheduling, Apply/mute integration, browser playback and export remain required.

## Progress log

### 2026-10-02 — ROOT0558 actual concurrent preparation capacity

Read-only author and Root source review proves default5 free versus8 needed for
complete old/new candidates. This prerequisite allocates capacity genuinely
before callback. No source written or behavioral acceptance by Root.

### Exact constructor/test refinement (before source)
```rust
#[test] fn configured_native_reports_actual_free_capacity_and_renders_existing_quantum();
#[test] fn configured_native_rejects_storage_rate_and_incompatible_quantum();
```
Native render splits into MAX_BLOCK512 chunks, so configured Engine max_block
must be at least512 and already within EngineConfig validated bounds. Rate must
be exact integral and storage NativeArc. Invalid configurations return Type
before pair; never silently replace caller capacities or storage. Public actual
CapacityReport must show bus_slots12 minus real legacy Master1 =11.

### ROOT0558 source-ready held checkpoint
Constructor implemented using genuine caller EngineConfig and existing pair;
legacy headless/headless_quad behavior unchanged. Native owner Rig allocates12
real bus slots with unchanged other Native configuration, capabilities and PCM
probe. New actual capacity and invalid configuration fixtures are written.
Scoped rustfmt/check0; source sizes968/570/610, all below1000. No author Cargo,
behavior acceptance or full playback claim. Both plans and all source held for
mandatory joined routing/provider/owner checker.
