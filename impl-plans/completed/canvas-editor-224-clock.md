> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-CLOCK: Canvas editor clock implementation plan

**planId**: CE-CLOCK
**planPath**: impl-plans/completed/canvas-editor-224-clock.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-TELEMETRY

## Intent and repository context
Share one audible absolute transport time across highlights, beat visuals and effects.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No receipt-time synchronization claims, protocol schema edits or native bootstrap.

## Related plans
Previous/dependencies: CE-TELEMETRY. Next: CE-CONSUMERS, CE-VISUAL, CE-SHELL.

## Write paths
- editor/src/app/clock.ts
- editor/src/code/highlight.ts
- editor/src/code/transport.ts
- editor/src/visual/frame.ts
- editor/test/canvas/clock.test.ts
- editor/test/code/highlight.test.ts
- editor/test/code/transport.test.ts
- editor/test/visual/frame.test.ts
- impl-plans/completed/canvas-editor-224-clock.md

## Shared paths and intended edits
- editor/test/code/highlight.test.ts: Successive ownership with CE-JOIN. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.
- editor/test/code/transport.test.ts: Successive ownership with CE-JOIN. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.
- editor/test/visual/frame.test.ts: Successive ownership with CE-VISUAL. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| K1 | app/clock.ts | Not started |
| K2 | code/highlight.ts and transport.ts | Not started |
| K3 | visual/frame.ts and tests | Not started |

### K1: app/clock.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Implement browser output timestamp correlation with page performance time. Do not subtract delay twice; fallback processing-minus-declared-total-delay marked estimate/unavailable. Native probe fit keeps8 samples, refresh1Hz, rejects RTT>100ms, reports halfRTT+host uncertainty. Stale>2sec/epoch mismatch invalidates synchronization. Extend existing Clock deliberately; retain processing-time accessor for core scheduling separate from audible display time.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### K2: code/highlight.ts and transport.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Accept new timed snapshots/end_time/epoch. Pure range outputs instead of decorations; map revision-aware spans, drop touched/old sources, cap4096/future2sec/batch4096, count loss. Late250ms frame recomputes now and skips expired ranges. Hold paused/lost MIDI, reanchor BPM/epoch, clear on hush/stop. Legacy receipt anchor labeled unsynchronized. Transport beat ring reads sample timestamp rather than arrival.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### K3: visual/frame.ts and tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Sample one display time per frame for musical visuals and ranges, while core session_frame receives its appropriate processing clock; do not feed delayed display time back into audio scheduling. Tests output-delay double count, synthetic varying RTT, stale correlation, epochs, BPM changes and5min drift simulation. Update highlight/transport tests to range data, preserving past behavior assertions.

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
- `cd editor && npm run check`
- `cd editor && npm run test -- test/canvas/clock.test.ts test/code/highlight.test.ts test/code/transport.test.ts test/visual/frame.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Absolute-time late recovery and invalid correlation behavior pass
- [ ] Processing versus audible display time is explicit
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
