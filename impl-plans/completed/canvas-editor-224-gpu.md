> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-GPU: Canvas editor gpu implementation plan

**planId**: CE-GPU
**planPath**: impl-plans/completed/canvas-editor-224-gpu.md
**Status**: In Progress — G1–G4 implementation and required verification complete; native/formal review pending
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
- impl-plans/completed/canvas-editor-224-gpu.md

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

## Session 235 bounded recovery: G4 first-frame background cache repair
**Status**: Implementation verified in recovery-235; renewed native/formal review pending.
**Design trace**: accepted 15.3.3, 15.3.5 and 15.3.7 require persistent text reuse separated from animation.
**Parallelizable**: Yes with CE-TELEMETRY and CE-PACKAGE after checkpoint; never with a dependent renderer consumer.
Fresh-read renderer.ts, atlas.ts, resources.ts and gpu.test.ts, with execution-contract pre/post hashes and immutable intentions for every edit. Change only existing owned paths. Diagnose shared geometry/staging admission: uploadBackground currently reserves staging through the atlas, so near-cap copies can evict resident text. Preserve text residency by accounting temporary backdrop staging independently of persistent text metadata or downscaling backdrop admission to leave required text headroom. Choose the smallest repair after reading current admission code. No new cache framework, cap increase, skipped glyphs or softened assertions.

Read complete preserved evidence:
- `tmp/canvas-editor-224/root-observations/background-cache-repro/finding.json` and `test.log`: isolated production renderer regression exit 1, 0/1; uploads 80,146,212,278.
- `tmp/canvas-editor-224/root-observations/background-cache-browser/finding.json`, `result.json`, `test.log`: production Chromium WebGL2 exit 1, 5/6; first-frame background uploads 80,144,208,272.
- `result-preloaded-cache.json` in that browser directory: 6/6 control, uploads remain 80. Passing control does not excuse first-frame failure.

Add official `editor/test/canvas/gpu.test.ts` regression using unchanged text with animated near-staging-cap background from the first frame. Require no extra text uploads after first render, reusable backdrop allocation, caps at every frame, no omitted text and zero live resources on disposal. Preserve original isolated reproduction and assertion sensitivity; do not rewrite operator tests to hide failure. Retain 16 MiB atlas, 8 MiB geometry/staging, 8 MiB layout, 4 million canvas/backdrop pixels and 96 MiB total caps.

Run foreground to exit, recording unique new log paths and source identities:
- `cd editor && npm run check`: whole-editor typing.
- `cd editor && npm run test -- test/canvas/gpu.test.ts`: original suite plus official first-frame regression.
- `cd editor && ./node_modules/.bin/vitest run --config ../tmp/canvas-editor-224/root-observations/background-cache-repro/vitest.config.mjs`: original isolated case must pass.
- `node tmp/canvas-editor-224/CE-GPU/recovery-235/background-cache-browser.mjs`: repaired first-frame case must pass all six assertions and exit 0. Prepare this finite evidence runner by copying `root-observations/background-cache-browser/run.mjs` into that equally deep attempt directory and changing ONLY its output `dir` constant to `tmp/canvas-editor-224/CE-GPU/recovery-235`. Preserve imports, production workload, all six assertions, cleanup and exit checks byte-for-byte; record original/copy hashes and exact one-line diff for independent integrity review. Use a new attempt name if the directory already exists. This avoids overwriting historical result/source/log records; execution-contract evidence paths permit probe artifacts, not additional production ownership. Capture stdout/stderr to the new complete test.log.
SwiftShader is software. These checks establish API/cache behavior, not Apple GPU performance, full editor integration or physical iPad evidence. CE-FINAL retains those gates.

- [x] G4 official regression, original isolated reproduction and six-assertion real-browser scenario pass on identical repaired source.
- [x] No cap increase; text/cache residency, admission refusal and disposal invariants retained.
- [ ] Runner-owned native progress, test-integrity, adversarial and integration decisions accept fresh evidence. Provisional historical GPU acceptance is invalid for final acceptance while G4 is open.

### Session: 2026-09-30 — Step4 session235
G4 allocated within existing GPU paths; failed evidence retained. No production source modified or implementation tests executed. CE-STATE/CE-INPUT accepted behavior is preserved; fresh reconciled-tree verification follows GPU repair under their retention contract.

### Session: 2026-09-30 — Step6 G4 recovery-235

Assigned runtime mode: issue-resolution; dependsOn and acceptedPlanIds are empty in the
remaining-plan projection. External CE-CONTRACT proof matches all seven current and
committed source/test identities (`recovery-235/external-contract-proof.json`). Read
checkpoint 3ce293e66d9cd1b436cf3a6018e2d581501d1a51 plan and accepted design15.3;
both match current bytes before edits. G4 aligns with persistent text/animation separation.

Fixed the reproduced material cache failure: backdrop copies reserve directly through the
ledger rather than evicting glyph metadata. Downsample staging to at most half of the
unchanged 8MiB geometry cap/current free geometry, leaving first-frame text headroom.
A reused backdrop must still fit current staging capacity; pressure triggers a smaller
replacement. Replacement admission budgets both texture and temporary copy before
allocation. Removed the obsolete background-only atlas eviction method. All reviewed
resource caps and glyph rendering/admission behavior remain unchanged.

Official tests add first-frame four-revision animation with 80 resident source/line-number
tiles, stable texture draw identities and uploads, no evictions, reusable backdrop allocation,
per-reservation caps and zero live resources on disposal. A second regression adds geometry
pressure and verifies backdrop shrink without text eviction. Existing 28 tests are retained.

Final-source foreground terminal evidence under `tmp/canvas-editor-224/CE-GPU/recovery-235/`:
- `cd editor && npm run check`: exit0; complete check.log and check.json.
- `cd editor && npm run test -- test/canvas/gpu.test.ts`: exit0;30 run/30 passed/0 failed;
  complete gpu-test.log and gpu-test.json.
- `cd editor && ./node_modules/.bin/vitest run --config ../tmp/canvas-editor-224/root-observations/background-cache-repro/vitest.config.mjs`:
  exit0;1 run/1 passed/0 failed; isolated-test.log and isolated-test.json.
- `node tmp/canvas-editor-224/CE-GPU/recovery-235/background-cache-browser.mjs`:
  exit0;6 run/6 passed/0 failed; browser-test.log (identical test.log), browser-test.json,
  result.json. Animated Chromium uploads remain80/80/80/80, evictions0, refusals0,
  owned bytes zero after disposal; backend Chromium151 SwiftShader.
- All157 captured editor source/test/configuration identities are identical across command
  terminal records and current bytes: source-suite-comparison.json. Browser source identities
  also independently match. Original operator failure logs/results remain unchanged.
- Finite browser runner copies original run.mjs with ONLY the output-dir line changed;
  original/copy SHA256 and exact one-line diff in runner-provenance.json. Browser/server
  lifecycle exits through original cleanup. No detached process or unowned service.

Immutable per-edit fresh bytes, intent, prehash/HEAD diff and posthash/change diff:
recovery-235/edit-1-renderer, edit-2-atlas, edit-3-tests and edit-4-progress; mirrored
pre-edit records in /private/tmp/vactr-224-implementation/CE-GPU/recovery-235/.
Owned source/test hunks only; no shared manifest/index, Rust/Swift, Git mutations or formatter.
Supporting read-only /root/gpu_investigation confirmed root cause and admission repair;
/root/gpu_repair_review accepted the bounded fix with no findings (supporting-review.json).
Improve self-review checked changed admission/cleanup paths, tests, cap preservation and
source-matched logs: no unresolved high/mid finding or implementation-phase verification gap.

G4 implementation-phase work is complete. Native progress, independent test-integrity,
adversarial/integration acceptance and review-dependent completion remain downstream;
formal acceptance is not claimed and the relevant checkbox remains unchecked. Serial
CE-FINAL retains renewed STATE/INPUT/combined integration gates, shared documentation,
archive and Git finalization. SwiftShader establishes API/cache behavior only: Apple GPU,
physical iPad, full app/IME/accessibility and measured hardware budgets remain their
assigned downstream gates, not inferred from these results.


### Session: 2026-10-01 — Step6 CE-GPU recovery-237-20261001-075142

Issue-resolution on main at e0d4fa30fa7c56911a4938e957c56d3bbdb3a430.
Runtime dependsOn/acceptedPlanIds are empty under the accepted remaining-plan projection;
external CE-CONTRACT committed/current hashes match admitted session237 proof
(external-contract-proof.json). Committed plan G4 and accepted design15.3 align.
All seven assigned files match native pre-node snapshot (snapshot-comparison.json).
Retained renderer/atlas repair and two official regressions were preserved without source edits.

Fresh terminal foreground evidence in `tmp/canvas-editor-224/CE-GPU/recovery-237-20261001-075142/`:
- `cd editor && npm run check`: exit1; complete check.log/check.json. Twelve diagnostics:
  web-tree-sitter missing in node_modules, resulting missing-module and implicit-any errors
  in syntax-core.ts and syntax tests. This is a failed required gate, not an approved baseline.
- `cd editor && npm run test -- test/canvas/gpu.test.ts`: exit0;30 run/30 passed/0 failed;
  complete gpu-test.log/gpu-test.json.
- `cd editor && ./node_modules/.bin/vitest run --config ../tmp/canvas-editor-224/root-observations/background-cache-repro/vitest.config.mjs`:
  exit0;1 run/1 passed/0 failed; complete isolated-test.log/isolated-test.json.
- `node tmp/canvas-editor-224/CE-GPU/recovery-237-20261001-075142/background-cache-browser.mjs`:
  exit0;6 run/6 passed/0 failed; complete browser-test.log/browser-test.json/result.json.
  Chromium151 SwiftShader: uploads80/80/80/80, zero evictions/refusals, zero owned bytes
  after disposal and GL error0. No hardware budget/device acceptance inferred.
- All captured editor source/test/configuration identities match before/after and each
  command (source-suite-comparison.json). Runner provenance preserves original assertions,
  workload, imports and cleanup; only output-dir line differs (runner-provenance.json).

Implementation is blocked/incomplete solely at the required typing gate. CE-PACKAGE or
serial coordinator must restore the already declared web-tree-sitter dependency; package
installation/lock edits are outside CE-GPU write scope. Redispatch after readiness and rerun
whole-editor check on fresh source. No narrower check substitutes for this gate.
Previously checked G4 behavioral/resource criteria remain proven; renewed typing and formal
native/test-integrity/adversarial/integration acceptance stay pending. Historical failure and
recovery evidence is preserved, not relabeled. Supporting read-only /root/gpu_retained_review
found no high/mid source issue (supporting-review.json). Improve self-check found this concrete
verification gap and records it in blocker.json/self-check.json; no speculative cleanup.
Plan-only fresh-read preimage, intended contextual status/log hunk and postdiff/hash are
recorded under edit-progress and mirrored under /private/tmp/vactr-224-implementation/CE-GPU/.
No Git mutations, shared manifest/index edits, Rust/Swift changes or detached commands.


### Session: 2026-10-01 — Step6 CE-GPU recovery-native-20261001-082614

Evidence-only native redispatch in issue-resolution. Runtime dependsOn=[] and
acceptedPlanIds=[CE-PACKAGE] govern current admission; committed e0d4fa30 G4 and
accepted design15.3.3/15.3.5 align. External CE-CONTRACT current/committed admitted
hashes match (external-contract-proof.json). All seven owned pre-node files match
(snapshot-comparison.json); no source edits, cap changes or dependency installs.
The installed web-tree-sitter0.27.0 readiness repair resolves the previous typing
blocker. Preserve recovery-237 failure/fingerprint and original operator failures.

Fresh foreground terminal verification under
`tmp/canvas-editor-224/CE-GPU/recovery-native-20261001-082614/`:
- `cd editor && npm run check`: exit0; complete check.log/check.json.
- `cd editor && npm run test -- test/canvas/gpu.test.ts`: exit0;30/30,0 failures;
  complete gpu-test.log/gpu-test.json.
- `cd editor && ./node_modules/.bin/vitest run --config ../tmp/canvas-editor-224/root-observations/background-cache-repro/vitest.config.mjs`:
  exit0;1/1,0 failures; complete isolated-test.log/isolated-test.json.
- `node tmp/canvas-editor-224/CE-GPU/recovery-native-20261001-082614/background-cache-browser.mjs`:
  exit0;6/6,0 failures; complete browser-test.log/browser-test.json/result.json.
  Chromium151 SwiftShader animated uploads80/80/80/80,evictions0,refusals0,
  GL error0 and owned bytes0 after disposal. No hardware-budget inference.
All210 captured source/test/configuration/operator-test identities match before,
after and every command (source-suite-comparison.json). Browser runner changes
only its output-directory constant; exact diff/hashes in runner-provenance.json.
Finite browser/server lifecycle closed before terminal return.

Supporting read-only `/root/gpu_recovery_review` found no actionable high/mid issue;
supporting-review.json records its limited decision. Improve author review inspected
retained repair/test hunks, admission/disposal/caps and complete logs: no findings or
implementation-phase verification gap (self-check.json). Immutable fresh plan preimage,
intent, HEAD diff and postimage/diff/hash are under edit-progress and mirrored under
/private/tmp/vactr-224-implementation/CE-GPU/recovery-native-20261001-082614/.
No Rust/Swift changes, shared manifest/index edits, Git mutations or broad formatting.

G1–G4 implementation-phase work and all required gates are complete; no blockers.
Native progress, test-integrity, adversarial/integration decisions and review-dependent
completion remain downstream. Their checkboxes stay unchecked; this Step6 does not
manufacture acceptance or route around native review. Assigned integration finding is
addressed by fresh source-matched evidence and readiness resolution; formal decisions
must now be obtained by runner-owned later steps. Shared reconciliation, device/manual
acceptance, measured hardware budgets and Git finalization retain existing downstream
ownership. SwiftShader establishes cache/API behavior only, not physical iPad evidence.
