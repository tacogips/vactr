# CE-CONTRACT: Canvas editor contracts implementation plan

**planId**: CE-CONTRACT
**planPath**: impl-plans/active/canvas-editor-224-contracts.md
**Status**: In Progress — implementation verified; formal workflow acceptance pending
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
| C1 | app/apis.ts and app/deps.ts | Implemented and verified |
| C2 | protocol/types.ts and envelope.ts | Implemented and verified |
| C3 | client.ts and store.ts | Implemented and verified |

### C1: app/apis.ts and app/deps.ts
**Status**: Implemented and verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Specify CodeSurface state/dispatch, change+selection subscription, focus, posAtCoords/coordsAtPos, range annotation and pointer subscription methods. State uses existing EditorState/Text/ChangeSet. Supply composition-range guard and deferred source-write callback; preserve samples and revision helpers. Add optional VisualApi background canvas source subscription and shared ResourceBudget reference. Keep legacy view temporarily for compilation; CE-JOIN removes it. No generalized adapter framework.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### C2: protocol/types.ts and envelope.ts
**Status**: Implemented and verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add optional playing epoch and end_time; preserve time/dur/src. Add optional TempoBody transport sample with epoch (opaque string), sample_time, cycle ratio, BPM, beats_per_cycle, running, latency_seconds nullable, latency_kind measured/estimate/unavailable, uncertainty_seconds nullable. Sample_time means processing onset time, not receipt. Probe contract echoes page_send plus engine_receive/engine_send, epoch and host correlation. Validate finite numeric bounds, rational denominators, positive BPM and 1 MiB telemetry envelope/4096 event ceilings; preserve legacy messages but mark missing timing unsynchronized.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### C3: client.ts and store.ts
**Status**: Implemented and verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Store latest timed sample; bound telemetry queue to64 messages coalescing snapshots/levels. Keep control replies outside telemetry dropping; bound pending requests64 with visible busy errors. Define disposal and all new listener cleanup. Test old envelopes, new timing, invalid values, overflow and no loss of eval/ack/error replies.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

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
- [x] Wire v1 compatibility and bounded queues proven
- [x] Contract descriptors consumed without runtime EditorView requirement
- [x] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved. Formal workflow acceptance remains pending.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.

### Session: 2026-09-30 15:30 JST — Step6 CE-CONTRACT implementation
**Tasks Completed**: C1, C2, C3; assigned implementation and required behavioral verification complete.
**Status boundary**: Formal test-integrity/adversarial/integration review, accepted completion, shared indexes/archive, staging, commit and push remain downstream. No worker Git mutations performed.
**Dependency readiness**: dependsOn is empty; runtime reviewFeedback contains no findings. Accepted design15.3 and committed plan at checkpoint8ee36f23e2311455126f6486db992d33227ff3e2 agree on additive contracts and staged legacy compatibility.
**C1 evidence**: app/apis.ts exports headless EditorState/ChangeSet dispatch, subscription, focus, coordinate, range, pointer and composition/deferred-write descriptors; deps.ts shares ResourceBudget. Existing samples/revision helpers and legacy view retained for CE-JOIN. contracts.test.ts consumes headless state/range descriptors without constructing EditorView. Concrete controller/rendering and cutover remain CE-STATE/CE-GPU/CE-JOIN ownership.
**C2 evidence**: Optional playing epoch/end_time and TempoBody.transport preserve v1 payloads. clock-probe echoes page_send and engine_receive/engine_send with opaque epoch, optional engine_time/output_time correlation and nullable latency/uncertainty provenance. Decoder validates finite/nonnegative times, ordered end time, positive BPM, safe rational denominators, UTF-8 source span bounds,4096 events and1MiB UTF-8 telemetry size. Legacy tempo clears synchronized sample; control envelopes do not use telemetry dropping.
**C3 evidence**: Client and Store cap reentrant telemetry64 and coalesce tempo/levels, with observable drop/coalesce counters. Client rejects request65 with visible busy error and preserves synchronous correlation. Correlated replies are excluded from dropping; control batches stay atomic. Store retains latest monotonic same-epoch sample, accepts epoch changes, clears legacy timing, and disposes state/listeners. Client close clears pending requests, document/tweak work and subscriptions.
**Immutable intents**: tmp/canvas-editor-224/CE-CONTRACT/attempt-1/edit-*/ contains read-only before bytes, SHA256/intent, pre-diff and intended-after bytes; /private/tmp/vactr-224-implementation/CE-CONTRACT/attempt-1-edit-*/ contains immutable corresponding pre-edit snapshots. Every contextual patch checked the fresh preimage and reread postimage. No detected task-source overwrite.
**Baseline**: `cd editor && npm run check` exit0, baseline-check.log; `cd editor && npm run test -- test/protocol` exit0,46 passed,0 failed, baseline-test.log. Paths below share tmp/canvas-editor-224/CE-CONTRACT/attempt-1/.
**Prior implementation checks**: check-1.log exit0; test-1.log exit0,77 passed,0 failed. After review fixes check-final.log exit0; test-final.log exit0,83 passed,0 failed. Final minor descriptor/fixture refinements were rerun below; earlier logs retained.
**Final required gates**:
- `cd editor && npm run check`: exit0; complete log check-final-2.log; metadata check-final-2.json.
- `cd editor && npm run test -- test/canvas/contracts.test.ts test/protocol`: exit0;9 files,83 tests passed,0 failed; complete log test-final-2.log; metadata test-final-2.json.
Both foreground processes reached terminal exit. verified-editor-source.json records SHA256 identities for editor source/tests/configuration/locks; identity remained unchanged across both final commands. This is stable local task evidence on the shared tree; CE-JOIN/CE-FINAL own combined final verification.
**Independent review and improve**: /root/contract_review read-only review found four mid issues: client stale snapshot coalescing, unsafe latency_kind coercion, disposal callbacks and subscriber exceptions stranding control replies. Fixed each with regression coverage. Improve self-review also removed unsafe raw version coercion and preserved statically typed probe fixture. Fresh independent reread found no unresolved high/mid findings; see independent-review.json. Formal workflow acceptance is not claimed.
**Preservation**: protocol/types.ts pre-existing ParamUnit 'm' remains intact. No Rust, Swift, dependency locks, shared manifest/index, MusicDSP or rumble files edited by this worker; other ongoing changes remain untouched.
**Remaining gates**: Downstream formal reviews and serial Git finalization; concrete GPU/input/clock/native implementation belongs dependent plans. No iPad, IME, GPU performance or physical synchronization evidence is claimed by this contract-only plan.

### Session: 2026-09-30 — Step4 session 227 author self-review correction
Corrected a checked formal-review criterion contradicted by this plan's own progress log.
Supporting independent review and author improve evidence remain historical; required
workflow decisions remain pending. No implementation behavior or evidence changed.
