> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-VISUAL: Canvas editor visual implementation plan

**planId**: CE-VISUAL
**planPath**: impl-plans/completed/canvas-editor-224-visual.md
**Status**: Ready for Step5 review; implementation not started
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-GPU, CE-CLOCK

## Intent and repository context
Reuse synth outputs/analyzer scopes and compose one bounded muted video background.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No microphone, camera, external streaming, new language syntax or cross-context texture sharing.

## Related plans
Previous/dependencies: CE-GPU, CE-CLOCK. Next: CE-JOIN.

## Write paths
- editor/src/visual/render-host.ts
- editor/src/visual/text-asset.ts
- editor/src/visual/mount.ts
- editor/src/visual/video.ts
- editor/src/visual/panes.ts
- editor/src/visual/scopes.ts
- editor/test/visual
- editor/test/canvas/visual.test.ts
- impl-plans/completed/canvas-editor-224-visual.md

## Shared paths and intended edits
- editor/test/visual: Successive ownership with CE-CLOCK. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| V1 | visual/render-host.ts and text-asset.ts | Not started |
| V2 | visual/video.ts and panes.ts | Not started |
| V3 | visual/mount.ts and scopes.ts plus tests | Not started |

### V1: visual/render-host.ts and text-asset.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Use CE-GPU shared ledger: eight1024-square targets max/text8MiB/global96MiB including transient resize replacements. Validate device limits, release textures/programs/targets on replace/dispose and restore empty feedback targets after context loss. Preserve last good shader on compile failure; no duplicate rendering engine.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### V2: visual/video.ts and panes.ts
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
One local file picker/user action, muted media element and one reusable upload texture. Max1280x720 at30Hz upload only on new decoded frames, pause hidden, dispose/revoke on replace. Reject excessive decoded resource usage with status. Publish latest visual canvas to compositor; transfer canvas via explicit copy at most once/frame, never a shared GL texture.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### V3: visual/mount.ts and scopes.ts plus tests
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Expose background canvas via contract subscription with lifetime cleanup. Native frontend may compose video/scopes while unsupported Hydra execution shows capability diagnostic. Oscilloscope reads existing analyzer cells; beat display uses clock. Test hidden/resume, decode failure, replacement/release, upload limits, allocation refusal and no microphone; retain render-host/scopes regressions.

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
- `cd editor && npm run test -- test/canvas/visual.test.ts test/visual`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Reusable bounded background/video lifecycle verified
- [ ] Existing synth feedback and analyzer behavior retained
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
