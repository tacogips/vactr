> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-CONSUMERS: Canvas editor consumers implementation plan

**planId**: CE-CONSUMERS
**planPath**: impl-plans/completed/canvas-editor-224-consumers.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-INPUT, CE-CLOCK

## Intent and repository context
Migrate binding and parameter consumers to the headless surface without changing authority or write semantics.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No cleanup of unrelated DSP/parameter editors or new per-slot mix.

## Related plans
Previous/dependencies: CE-INPUT, CE-CLOCK. Next: CE-JOIN.

## Write paths
- editor/src/bind/mount.ts
- editor/src/bind/drag.ts
- editor/src/bind/write.ts
- editor/src/bind/routing.ts
- editor/src/params/mount.ts
- editor/src/params/roll.ts
- editor/test/bind
- editor/test/params
- impl-plans/completed/canvas-editor-224-consumers.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| B1 | bind/mount.ts and drag.ts | Not started |
| B2 | bind/write.ts and routing.ts | Not started |
| B3 | params/mount.ts and roll.ts plus tests | Not started |

### B1: bind/mount.ts and drag.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Replace appendConfig/updateListener/dom handlers/decorations with concrete subscriptions, pointer capture and GPU overlay annotation channels. Map eligible numeric sites using source revisions and coordinates; preserve ordinary selection and stale-site rejection. Inline badge shows overlay value but never edits source in overlay mode.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### B2: bind/write.ts and routing.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Read state/dispatch through CodeSurface, flush pending doc changes before writes/eval, enforce expected text/form_gen/edit_epoch and per-target16ms latest-wins. During composition defer conflicting source changes using frozen site/expected text, revalidate after commit; report stale instead of overwriting. Maintain directive learns and external-file persistence behavior.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### B3: params/mount.ts and roll.ts plus tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Replace head decorations/click DOM extension with GPU range annotations and surface pointer hit testing, preserve selectedSiteId and roll text reads. Parameter edits still route through BindApi; display grids/rolls never emit write messages. Port existing tests, preserving source/overlay/MIDI-learning/reconciliation semantics; test one revision per binding edit and selection isolation.

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
- `cd editor && npm run test -- test/bind test/params`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] No migrated consumer imports EditorView or decorations
- [ ] Binding, directives, numeric drag and params regression suites retain behavioral coverage
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
