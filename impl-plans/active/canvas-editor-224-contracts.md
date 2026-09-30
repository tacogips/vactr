# CE-CONTRACT: Canvas editor contracts implementation plan

**planId**: CE-CONTRACT
**planPath**: impl-plans/active/canvas-editor-224-contracts.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: None

## Intent and repository context
Freeze the concrete headless surface and additive timing wire contracts before downstream work.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No editor cutover, rendering, native engine bootstrap or dependency updates.

## Related plans
Previous/dependencies: None. Next: CE-STATE, CE-GPU, CE-TELEMETRY, CE-PACKAGE.

## Write paths
- editor/src/app/apis.ts
- editor/src/app/deps.ts
- editor/src/protocol/types.ts
- editor/src/protocol/envelope.ts
- editor/src/protocol/client.ts
- editor/src/protocol/store.ts
- editor/test/canvas/contracts.test.ts
- impl-plans/active/canvas-editor-224-contracts.md

## Shared paths and intended edits
- editor/src/app/apis.ts: Successive ownership with CE-JOIN. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| C1 | app/apis.ts and app/deps.ts | Not started |
| C2 | protocol/types.ts and envelope.ts | Not started |
| C3 | client.ts and store.ts | Not started |

### C1: app/apis.ts and app/deps.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Specify CodeSurface state/dispatch, change+selection subscription, focus, posAtCoords/coordsAtPos, range annotation and pointer subscription methods. State uses existing EditorState/Text/ChangeSet. Supply composition-range guard and deferred source-write callback; preserve samples and revision helpers. Add optional VisualApi background canvas source subscription and shared ResourceBudget reference. Keep legacy view temporarily for compilation; CE-JOIN removes it. No generalized adapter framework.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### C2: protocol/types.ts and envelope.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add optional playing epoch and end_time; preserve time/dur/src. Add optional TempoBody transport sample with epoch (opaque string), sample_time, cycle ratio, BPM, beats_per_cycle, running, latency_seconds nullable, latency_kind measured/estimate/unavailable, uncertainty_seconds nullable. Sample_time means processing onset time, not receipt. Probe contract echoes page_send plus engine_receive/engine_send, epoch and host correlation. Validate finite numeric bounds, rational denominators, positive BPM and 1 MiB telemetry envelope/4096 event ceilings; preserve legacy messages but mark missing timing unsynchronized.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### C3: client.ts and store.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Store latest timed sample; bound telemetry queue to64 messages coalescing snapshots/levels. Keep control replies outside telemetry dropping; bound pending requests64 with visible busy errors. Define disposal and all new listener cleanup. Test old envelopes, new timing, invalid values, overflow and no loss of eval/ack/error replies.

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
- `cd editor && npm run test -- test/canvas/contracts.test.ts test/protocol`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Wire v1 compatibility and bounded queues proven
- [ ] Contract descriptors consumed without runtime EditorView requirement
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
