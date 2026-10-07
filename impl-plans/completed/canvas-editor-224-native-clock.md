> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-AUDIO: Canvas editor clock implementation plan

**planId**: CE-AUDIO
**planPath**: impl-plans/completed/canvas-editor-224-native-clock.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-TELEMETRY

## Intent and repository context
Expose truthful native processing/output clock correlation through existing NativeAudioHost.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No replacement engine, audio DSP refactor or hard real-time display claim.

## Related plans
Previous/dependencies: CE-TELEMETRY. Next: CE-SHELL.

## Write paths
- src/host/native/audio.rs
- src/host/native/clock.rs
- src/host/native/mod.rs
- src/host/native/tests/audio.rs
- src/host/native/tests/clock.rs
- src/host/native/tests/mod.rs
- impl-plans/completed/canvas-editor-224-native-clock.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| A1 | native/clock.rs and audio.rs | Not started |
| A2 | native/mod.rs | Not started |
| A3 | native tests | Not started |

### A1: native/clock.rs and audio.rs
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add fixed-size lock-free timestamp publication linked to FrameClock and CPAL callback presentation metadata where actually supported. Export snapshot processing time, monotonic correlation, epoch, output delay and uncertainty/provenance. Metadata unavailable must remain unavailable, never derived from receipt time. Use CPAL output callback timestamp to pair stream playback/callback times with frame counter; only bounded atomic publication in callback, no logging/locks/IPC. New clock module keeps audio.rs below1000 lines; if threshold reached split existing cohesive clock portion, preserving user changes.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### A2: native/mod.rs
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Expose clock snapshot to native session owner with existing NativeHosts/cells/FrameClock. Route re-open/reset invalidates epoch; preserve loader, instrument registry, no-input default and named-bus behavior.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### A3: native tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Headless tests for coherent snapshot reads, no unavailable-delay fabrication, timestamp discontinuity/reset and callback metadata accounting. Hardware route/delay evidence belongs CE-VERIFY; unit tests cannot prove physical compensation.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

## Invariants
- Single editing authority; visible source/feedbackGPU, DOM input/accessibility only.
- Source authority, revision/epoch, UTF conversion and flush-before-write preserved.
- Audio never waits for UI; processing/display clocks remain separate.
- Preserve user dirty hunks, latest predecessor contract and resource/queue ceilings.
- Device and measurement evidence remains incomplete unless actually captured.

## Verification commands and required evidence
- `CARGO_TERM_QUIET=true cargo check`
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run host::native::tests::`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Native output metadata has explicit provenance
- [ ] Audio callback remains isolated from UI/control paths
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
