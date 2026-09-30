# CE-GPU: Canvas editor gpu implementation plan

**planId**: CE-GPU
**planPath**: impl-plans/active/canvas-editor-224-gpu.md
**Status**: In Progress
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT

## Intent and repository context
Draw all visible source and feedback on WebGL2 with bounded reusable text geometry.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No DOM source renderer, new language operators or consumer migration.

## Related plans
Previous/dependencies: CE-CONTRACT. Next: CE-INPUT, CE-VISUAL.

## Write paths
- editor/src/code/layout.ts
- editor/src/code/atlas.ts
- editor/src/code/renderer.ts
- editor/src/code/resources.ts
- editor/src/code/code.css
- editor/test/canvas/gpu.test.ts
- impl-plans/active/canvas-editor-224-gpu.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| G1 | code/layout.ts and atlas.ts | Implemented; verified |
| G2 | code/resources.ts and renderer.ts | Implemented; verified |
| G3 | code/code.css and gpu tests | Implemented; verified |

### G1: code/layout.ts and atlas.ts
**Status**: Implemented; verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Shape visible unwrapped runs via offscreen2D browser fonts, tile within texture limits; tabs four stops and CRLF one visual break. Grapheme navigation maps measured advances to UTF16; never split surrogate/combining clusters. Key atlas by text/font/fallback/DPR/style, invalidate font/DPR. Bound atlas16MiB, layout8MiB; LRU offscreen eviction, bounded batches for visible overflow and long lines without missing text. Report unsupported RTL, test Japanese and ligature caret geometry against shaped output.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### G2: code/resources.ts and renderer.ts
**Status**: Implemented; verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Concrete allocation ledger shared with visual code: GPU total96MiB, geometry/staging8MiB, canvas/backdrop4M pixels, visual text8MiB, eight1024-square RGBA8 targets, video720p. Reserve transient replacement before allocation; release on resize/dispose/context loss and expose counters. Render background, scrim, selections/playing/eval, syntax text+line numbers, diagnostics, cursor, badges and touch handles in that order. Buffer reuse/dirty uploads only; idle frames do not rebuild text.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted tests prove behavior and complete logs show final exits.

### G3: code/code.css and gpu tests
**Status**: Implemented; verified
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Style transparent caret bridge without visible duplicate text. Handle context loss/restore from CPU state and empty feedback targets, allocation-failure GPU status with accessible save, effective DPR reduction status. Verify ledger refusal/rollback, retained text after failure, coordinates, clipping, dirty upload counts and resource deletion with recordingGL; realGPU proof is CE-VERIFY.

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
- `cd editor && npm run test -- test/canvas/gpu.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [x] All required annotations have GPU drawing paths
- [x] Allocation ledger enforces caps and disposal returns live owned bytes to zero
- [x] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.

### Session: 2026-09-30 — CE-GPU implementation

Implementation-phase tasks G1–G3 complete; formal workflow review remains downstream.
Dependency admission: runtime acceptedPlanIds contains CE-CONTRACT; no reinterpretation of
predecessor acceptance. Committed plan and design15.3.3/15.3.5 align with this implementation.
Only the seven assigned paths changed. No worker Git mutations, shared manifest/index edits,
Rust changes, detached commands or broad formatting were performed.

Deliverables: TextLayout exposes measured UTF16 grapheme coordinates, CRLF, four-space tabs,
visible ranges and bounded LRU with unchanged-line reuse; RTL hit testing reports unsupported.
GlyphAtlas rasterizes whole shaped runs into cropped reusable tiles, accounts persistent metadata
and temporary staging, evicts under pressure and draws batches without losing visible text.
CanvasRenderer accepts CPU layout, annotations, cursor/handles, status/save callbacks and a
revision-tagged background canvas; draws all specified layers on WebGL2 with one static quad.
ResourceLedger implements ResourceBudget and typed reservations for96MiB total,16MiB atlas,
8MiB geometry/staging,4M canvas/backdrop pixels,8MiB visual text, eight1024-square visual
targets and one720p video allocation. Canvas reserves conservatively at most2M pixels to leave
backdrop headroom. Layout cache is8MiB; atlas weak identities do not retain historical source.
GPU failure/context loss retains CPU document and accessible-save seam; restore allocates fresh
resources; dispose deletes owned GL objects and returns owned ledger bytes to zero.
Scoped transparent input-bridge CSS preserves legacy/sample styles for the CE-JOIN cutover.

Final source-matched evidence (complete foreground logs with terminal exits):
- `cd editor && npm run check`: exit0; `tmp/canvas-editor-224/CE-GPU/attempt-1/check-4.log`
  and `check-4.json` (command metadata and all editor source/test SHA256 identities).
- `cd editor && npm run test -- test/canvas/gpu.test.ts`: exit0;28 passed,0 failed;
  `tmp/canvas-editor-224/CE-GPU/attempt-1/test-4.log` and `test-4.json`.
- Exact-path `git diff --check`: exit0; final record in `diff-check.log`/`diff-check.json`.
- Immutable fresh-read per-edit intent, prehash/bytes, diff and posthash records:
  `tmp/canvas-editor-224/CE-GPU/attempt-1/edit-*/`, mirrored before editing under
  `/private/tmp/vactr-224-implementation/CE-GPU/attempt-1/edit-*/`.
  Final source snapshots and drift comparison: `handoff-source.json`/`self-check.json`.

Prior attempts preserved: check-initial exit1 (two literal-parameter type errors); test-1 exit1
(23 tests,21 passed,2 failures: staging eviction); test-2 exit1 (26 tests,23 passed,3 failures:
DPR2 tile test budget and ephemeral label identities). check-2/check-3 passed; test-3 passed26.
All failures were corrected before source-matched check-4/test-4; earlier logs remain immutable.

Supporting independent read-only reviewer `/root/gpu_review` identified six material issues,
all fixed: equal-backing-size DPR refresh; background source identity; initial backdrop staging
bound; historical source retention; subsequent backdrop staging after metadata growth; dense
long-line syntax metadata bounded to visible tiles. Supporting decision accepted with no findings:
`supporting-review.json`. Improve skill self-review also corrected staging admission/eviction
and stable bounded label identities; regressions cover all fixes. Formal test-integrity/adversarial
review is not claimed and the corresponding completion checkbox remains unchecked for that gate.

Downstream ownership: CE-INPUT handles interactions/IME/accessibility; CE-VISUAL consumes the
ledger/background seam; CE-JOIN wires the renderer/status/save into mount and removes EditorView;
CE-FINAL owns shared indexes/archive, combined-tree checks, real GPU/physical iPad and measured
performance evidence, plus coordinator Git finalization. RecordingGL/font metrics are behavioral
unit proof only; real browser Japanese/ligature caret validation and hardware measurements remain
with those accepted downstream gates. No GPU/iPad/performance acceptance is inferred here.

### Session: 2026-09-30 — CE-GPU runtime retry retry-20260930-180816

Runtime dependency admission: CE-CONTRACT is in acceptedPlanIds. Assigned reviewFeedback
has no findings; supplied CE-STATE/CE-TELEMETRY/CE-PACKAGE findings remain with their owners.
Committed plan at 4fc37414e2f2704f6737f1545071bf506bd9c0f8 and accepted design15.3.3/15.3.5
were read; current G1–G3 implementation remains aligned. No source correction was necessary.
All seven owned files match the supplied pre-node snapshot; snapshot-comparison.json records
the exact comparison. All163 captured source/configuration identities remained unchanged
across fresh foreground verification (source-before.json/source-after.json).

Fresh required gates on this source:
- `cd editor && npm run check`: exit0; complete `tmp/canvas-editor-224/CE-GPU/retry-20260930-180816/check.log` and check.json.
- `cd editor && npm run test -- test/canvas/gpu.test.ts`: exit0;28 run,28 passed,0 failed;
  complete `tmp/canvas-editor-224/CE-GPU/retry-20260930-180816/test.log` and test.json.

Supporting independent read-only agent `/root/gpu_scope_review` found no high/mid issue;
review decision retained in supporting-review.json. Improve author self-check inspected
GPU drawing paths, clipping, admission/rollback, idle upload reuse, context-loss recovery,
CPU save retention, disposal and current tests; no actionable material finding or required
verification gap. No Rust/Swift changes or Git mutations. This retry edits only this log;
immutable fresh-read prebytes, intent, diff and posthash live in edit-progress/ here and
/private/tmp/vactr-224-implementation/CE-GPU/retry-20260930-180816-edit-progress/.

Implementation-phase G1–G3 and required behavioral gates are complete. Formal integrity,
adversarial/integration review and review-dependent completion record remain downstream;
the formal-review checkbox stays unchecked. CE-INPUT/CE-VISUAL/CE-JOIN/CE-FINAL retain
interaction, consumer integration, real GPU/device and measured hardware ownership as
already recorded above. Unit tests do not establish actual Japanese font shaping or iPad
performance. No native progress-gate result or formal acceptance is manufactured here.
