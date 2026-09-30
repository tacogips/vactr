# CE-PACKAGE: Canvas editor preparation implementation plan

**planId**: CE-PACKAGE
**planPath**: impl-plans/active/canvas-editor-224-package-preparation.md
**Status**: In Progress — implementation retry verified; runner progress and formal reviews pending
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT

## Intent and repository context
Serially prepare and lock only necessary browser automation and root-native shell dependencies before shell or final verification.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No Rust source edits, global installs, browser server, registry provenance discovery or unrelated dependency upgrades.

## Related plans
Previous/dependencies: CE-CONTRACT. Next: CE-SHELL.

## Write paths
- editor/test/canvas/package-browser-smoke.mjs
- editor/test/canvas/package-dependencies.py
- editor/package.json
- editor/package-lock.json
- editor/src-tauri/Cargo.toml
- editor/src-tauri/Cargo.lock
- impl-plans/active/canvas-editor-224-dependency-evidence.md
- impl-plans/active/canvas-editor-224-package-preparation.md

## Shared paths and intended edits
- editor/src-tauri/Cargo.toml: Successive ownership with CE-SHELL. Fresh-read/hash before each edit; predecessor finishes before dependent edit. CE-PACKAGE alone generates initial locks; finalization updates index/archive after join. Workers use locked checks.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| P1 | package.json and package-lock.json | Implemented |
| P2 | src-tauri/Cargo.toml and Cargo.lock | Implemented |
| P3 | dependency-evidence.md | Implemented |
| P4 | Portable browser/dependency retry and native/formal gates | Implementation verified; downstream gates pending |

### P1: package.json and package-lock.json
**Status**: Implemented
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add exact local playwright1.62.1 dev dependency using npm install --save-dev --save-exact playwright@1.62.1 --ignore-scripts. Record source/version/integrity and diff; do not upgrade other packages. Global playwright shim has no configured version, so consumers use the node runner with local import. Install Chromium via local CLI only as foreground bounded preparation; retain its logs and record unavailable browser/network prerequisites without passing evidence.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted preparation checks completed; full logs show final exits and conditional unavailable tooling.

### P2: src-tauri/Cargo.toml and Cargo.lock
**Status**: Implemented
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add local root vactr path dependency with host-native enabled. Keep locked Tauri/plugin versions; no new core Cargo dependencies. Supply-chain-secure-install skill required. Generate lock serially via CARGO_TERM_QUIET=true cargo generate-lockfile --manifest-path editor/src-tauri/Cargo.toml; inspect new native transitive dependency tree and ensure root Cargo.lock untouched. Downstream native shell changes only targets/entry/config and uses locked checks.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted preparation checks completed; full logs show final exits and conditional unavailable tooling.

### P3: dependency-evidence.md
**Status**: Implemented
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Record pinned artifacts/integrities, full install/generate logs and statuses. If exact dependency cannot resolve, report real dependency verification failure, not workflow-readiness failure; request serial plan amendment before changing pin. Include npm exec -- playwright install chromium output and actual browser install path/version for final runner.

**Completion criteria**:
- [x] File-level behavior above implemented with required invariants.
- [x] Targeted preparation checks completed; full logs show final exits and conditional unavailable tooling.

## Invariants
- Single editing authority; visible source/feedbackGPU, DOM input/accessibility only.
- Source authority, revision/epoch, UTF conversion and flush-before-write preserved.
- Audio never waits for UI; processing/display clocks remain separate.
- Preserve user dirty hunks, latest predecessor contract and resource/queue ceilings.
- Device and measurement evidence remains incomplete unless actually captured.

## Verification commands and required evidence
- `cd editor && npm install --save-dev --save-exact playwright@1.62.1 --ignore-scripts`
- `cd editor && npm exec -- playwright install chromium`
- `CARGO_TERM_QUIET=true cargo generate-lockfile --manifest-path editor/src-tauri/Cargo.toml`
- `CARGO_TERM_QUIET=true cargo metadata --locked --no-deps --manifest-path editor/src-tauri/Cargo.toml --format-version 1`
- `mise exec -- rustup target list --installed`
- `xcodebuild -version`
- `xcodebuild -showsdks`
- `CARGO_TERM_QUIET=true cargo tauri --version`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [x] Task-only pinned dependency diffs and complete logs
- [x] Browser binary available or verification gap explicitly recorded; no worker regenerates locks
- [x] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.

### Session: 2026-09-30 — CE-PACKAGE implementation
P1/P2/P3 implemented. Exact Playwright pin installed; Chromium 151.0.7922.34 launched with WebGL2 pixel smoke (3/3). Existing npm pins unchanged. Added local vactr host-native dependency and pinned existing direct Tauri versions. Initial lock generation upgraded unrelated transitives; improve self-review repaired graph preserving all 430 original identities and adding 31 native closure identities. Root manifest/lock unchanged; dependency checks 8/8. No Rust sources changed.

Command/status/log table and integrity records: [dependency evidence](canvas-editor-224-dependency-evidence.md); logs/metadata: `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/`. Fresh before-edit bytes, hashes, diffs and intents: `edit-001` through `edit-004`, mirrored under `/private/tmp/vactr-224-implementation/CE-PACKAGE/attempt-1/`. Full final locked metadata and native tree exit 0. Prior offline metadata exit 101 was resolved by online locked metadata exit 0. Conditional inventory: Xcode/iOS SDK available; iOS Rust targets and cargo-tauri absent (tauri-version exit 101), explicitly recorded for CE-SHELL/CE-FINAL; not a native build pass.

Implementation-phase requirements complete. Formal review criterion remains unchecked for later workflow steps; read-only package review and improve check do not substitute for those approvals. Next: formal review/integration, then CE-SHELL locked consumer. Shared indexes/archive, broad verification and task Git finalization are downstream. No Git mutations.

## Session 227 preparation retry contract (integration finding 3)
The native CE-PACKAGE progress result remains blocked. Attempt-1 smoke/dependency
assertions and metadata exist, but documentation saying implementation-phase complete
is not native gate acceptance or completed formal workflow review.

### P4: Portable implementation-exercising retry evidence
**Status**: Implementation verified; runner progress and formal reviews pending
**Parallelizable**: No; execute serially before CE-SHELL consumes accepted preparation.
- `editor/test/canvas/package-browser-smoke.mjs`: retain the existing attempt-1 smoke
  assertions in a portable checked-in runner importing local Playwright. Verify installed
  exact pin, executable existence, launch Chromium, create WebGL2, clear/read expected
  pixel and close browser in finally. Exit nonzero on failure; emit actual assertion counts,
  version and executable. This exercises installed preparation, not canvas application
  rendering; CE-FINAL still owns application/browser and device evidence.
- `editor/test/canvas/package-dependencies.py`: port attempt-1 dependency assertions
  to a read-only verifier with explicit --baseline and --output arguments. Compare
  current locks to preserved pre-preparation snapshots: retain all 430 original Cargo
  identities/checksums, unchanged existing npm records, exact Playwright/core 1.62.1,
  root path vactr host-native declaration, allowed registry sources and attributable
  native closure. Record current root manifest/lock hashes before/after checks; preserve
  any unrelated later root manifest drift rather than reverting it to attempt-1 bytes.
  Missing baseline cannot pass preservation checks. Write only new evidence output,
  never overwrite attempt-1 scripts/results/snapshots or regenerate dependency locks.
- `impl-plans/active/canvas-editor-224-dependency-evidence.md`: append retry command
  table, actual assertions/counts, source hashes and gate/review decisions. Preserve prior
  offline metadata exit 101 and absent cargo-tauri exit 101 as historical outcomes.
Run the following from repository root, saving complete logs under
`tmp/canvas-editor-224/CE-PACKAGE/attempt-2/`:
- `node editor/test/canvas/package-browser-smoke.mjs`
- `python3 editor/test/canvas/package-dependencies.py --baseline tmp/canvas-editor-224/CE-PACKAGE/attempt-1 --output tmp/canvas-editor-224/CE-PACKAGE/attempt-2/dependency-comparison.json`
- `CARGO_TERM_QUIET=true cargo metadata --locked --manifest-path editor/src-tauri/Cargo.toml --format-version 1`
- `CARGO_TERM_QUIET=true cargo tree --locked --manifest-path editor/src-tauri/Cargo.toml -p vactr`
No reinstall, browser download or lock generation on a normal retry. Only if a concrete
missing installed dependency prevents these checks, use the existing P1/P2 preparation
commands with captured fresh intents; preserve locked identities and inspect any diff.
Submit actual smoke/dependency assertion counts and failures via runner-supplied native
progress gate; metadata-only success is insufficient. Then obtain required test-integrity,
adversarial and integration decisions. No hand-written accepted flag substitutes for them.
- [x] Portable installed-browser smoke and dependency preservation assertions exit 0.
- [x] Full locked metadata/tree exits 0 and dependencies stay unchanged across retry.
- [ ] Native progress gate accepts implementation-exercising results/source identities.
- [ ] Formal workflow reviews accepted with no unresolved high/mid findings.

### Session: 2026-09-30 — Step4 session 227 amendment
P4 added with two preparation-specific verification scripts and exact commands. Native
blocked result remains authoritative; no command, formal review or native progress retry
was executed by this planning node. Existing browser/dependency assertions retained.

### Session: 2026-09-30 — Step 6 CE-PACKAGE preparation retry
P4 implementation-phase deliverables complete. Added portable installed-browser and
read-only authenticated-baseline dependency runners. Canonical bare Python required
a mise-installed Python fallback for tomllib; corrected before required gate execution.
All four exact assigned commands exit 0 with terminal status/full logs and eight
matching before/after source hashes under `tmp/canvas-editor-224/CE-PACKAGE/attempt-2/`.
Browser assertions 4/4, dependency assertions 10/10, failures 0. All 430 prior Cargo
identities/checksums retained; 31 additions attributable to local vactr closure;
existing npm records unchanged. Locked metadata/tree verified; no locks regenerated.

Commands and statuses: [dependency evidence](canvas-editor-224-dependency-evidence.md#session-227-step-6-retry--2026-09-30).
Immutable fresh-read intents: attempt-2/edit-*/ and
/private/tmp/vactr-224-implementation/CE-PACKAGE/attempt-2/edit-*/.
Read-only /root/package_review verified current identities and gate evidence;
improve author self-check found no unresolved high/mid issue. Historical blocked
native result and attempt-1 failed inventory/metadata logs are preserved, not rewritten.

Integration finding 3 implementation retry is addressed with new implementation-exercising
results. Native progress consumes this Step 6 JSON next; decision not yet returned.
Formal integrity/adversarial/integration approvals and their completion record are
later workflow work, not implementation gaps. Corresponding checkboxes remain unchecked.
CE-STATE and CE-TELEMETRY findings belong to their owners. No edits to the shared manifest,
indexes, dependency files, Rust/Swift/TypeScript or Git state. Dependent admission must
await runner-owned acceptedPlanIds; neither this log nor author review grants admission.

### Session: 2026-09-30 18:46 — CE-PACKAGE runner reconsideration handoff
Reconciled current assigned review feedback: evidence consumption needs runner-owned
reconsideration, not an unchanged implementation retry or dependency regeneration.
CE-CONTRACT admission is authoritative in runtime acceptedPlanIds; design 15.3.6 and
committed plan scope remain aligned. No source or dependency changes were warranted.

Fresh source audit: `python3 tmp/canvas-editor-224/CE-PACKAGE/reconsideration-20260930-184558/source-audit.py` exited 0;
complete log `tmp/canvas-editor-224/CE-PACKAGE/reconsideration-20260930-184558/source-audit.log`, status/source comparison
`tmp/canvas-editor-224/CE-PACKAGE/reconsideration-20260930-184558/source-audit-status.json` and `tmp/canvas-editor-224/CE-PACKAGE/reconsideration-20260930-184558/source-audit.json`.
All eight pre-node snapshot files matched bytes/modes; all four required command
identities still match current manifests, locks and runners. Existing complete
attempt-2 logs were inspected without rerunning: installed-browser 4/4 and dependency
10/10, zero failures; locked metadata/tree exit 0. Counts are submitted explicitly
in this Step 6 payload for runner-owned reconsideration. No lock regeneration.

Read-only `/root/package_evidence_review` independently confirmed all four logs,
source identities and positive counts, with no material findings. Improve author
self-review found no in-scope high/mid issue. Fresh preimage, intent, postimage and
exact append are retained in `tmp/canvas-editor-224/CE-PACKAGE/reconsideration-20260930-184558/edit-progress/` and the matching immutable
`/private/tmp/vactr-224-implementation/CE-PACKAGE/reconsideration-20260930-184558/edit-progress/`.

Historical native blocked classification is preserved until replaced by an
authoritative runner decision. This implementation-phase handoff claims neither
native acceptance nor formal review. Native reconsideration, test-integrity,
adversarial and integration review remain downstream; CE-SHELL admission awaits
runner-owned acceptance. Both gate checkboxes remain unchecked. Source implementation
and required behavioral verification are complete. No shared manifest, unrelated
files, Git state, Rust, Swift or TypeScript modifications.

### Session: 2026-09-30 19:22 — CE-PACKAGE fresh behavioral gate retry
CE-CONTRACT is admitted by runtime acceptedPlanIds. Committed P4 and accepted design
15.3.6 remain aligned; assigned reviewFeedback.findings is empty. Reconciled prior
integration finding 3 with fresh implementation-exercising results, rather than reusing
old logs or claiming a runner decision. No source or dependency repair was necessary.

Evidence directory: `tmp/canvas-editor-224/CE-PACKAGE/retry-20260930-192218/`.
All commands ran in the foreground from repository root; terminal metadata includes
start/end, cwd, final exit and complete log. Exact commands:
- `node editor/test/canvas/package-browser-smoke.mjs`: exit 0, 4/4 assertions, zero failures; `browser-smoke.log`.
- `python3 editor/test/canvas/package-dependencies.py --baseline tmp/canvas-editor-224/CE-PACKAGE/attempt-1 --output tmp/canvas-editor-224/CE-PACKAGE/retry-20260930-192218/dependency-comparison.json`: exit 0, 10/10 assertions, zero failures; `dependencies.log`.
- `CARGO_TERM_QUIET=true cargo metadata --locked --manifest-path editor/src-tauri/Cargo.toml --format-version 1`: exit 0, 461 packages/nodes; `cargo-metadata.log`.
- `CARGO_TERM_QUIET=true cargo tree --locked --manifest-path editor/src-tauri/Cargo.toml -p vactr`: exit 0; `cargo-tree.log`.

Chromium 151.0.7922.34 launched, verified WebGL2 pixel [0,255,0,255], and closed.
All 430 original Cargo identities/checksums and existing npm records remain preserved;
31 added native closure identities remain attributable. No locks regenerated.
The dependency output path is unique to this retry because the verifier exclusively
creates output; original attempt-2 evidence is preserved. `snapshot-audit.json` matched
all eight owned pre-node snapshot files by bytes/modes; `source-stability.json` confirms
all ten checked paths unchanged during verification. Historical failed inventory and
metadata logs and historical native blocked classification remain preserved.

Read-only `/root/package_scoped_review` found no material issue in scripts, dependency
contract and design alignment. Improve author self-review checked counts, full logs,
source stability, ownership and completion boundaries; no unresolved high/mid issue.
Immutable fresh-read preimage, intent, diff and postimage: `edit-progress/`, mirrored
under `/private/tmp/vactr-224-implementation/CE-PACKAGE/retry-20260930-192218/`.

Implementation-phase work is complete. Submit actual positive counts through this
Step 6 payload for the runner-owned progress decision; no acceptance is manufactured.
Native progress, test-integrity, adversarial and integration decisions remain downstream,
with existing completion checkboxes unchecked. Dependent admission uses runtime-owned
acceptedPlanIds. Shared manifest/index/archive and Git finalization remain downstream.
No Rust, Swift or TypeScript edits, dependency writes or Git mutations in this retry.

### Session: 2026-09-30 20:30 — CE-PACKAGE fresh Step 6 handoff
CE-CONTRACT is admitted by runtime acceptedPlanIds; committed P4 and accepted design
15.3.6 remain aligned. Assigned reviewFeedback.findings is empty. Prior integration
finding 3 is addressed with fresh implementation-exercising verification and structured
positive counts for the native progress gate. No dependency or source repair was needed.

Evidence: `tmp/canvas-editor-224/CE-PACKAGE/retry-20260930T203042506761/`.
All four commands ran in the foreground to terminal exit, with full logs and per-command
JSON recording cwd, start/end and exit status:
- `node editor/test/canvas/package-browser-smoke.mjs`: exit 0; 4/4 assertions, zero failures; `browser-smoke.log`.
- `python3 editor/test/canvas/package-dependencies.py --baseline tmp/canvas-editor-224/CE-PACKAGE/attempt-1 --output tmp/canvas-editor-224/CE-PACKAGE/retry-20260930T203042506761/dependency-comparison.json`: exit 0; 10/10 assertions, zero failures; `dependencies.log`.
- `CARGO_TERM_QUIET=true cargo metadata --locked --manifest-path editor/src-tauri/Cargo.toml --format-version 1`: exit 0; `cargo-metadata.log`.
- `CARGO_TERM_QUIET=true cargo tree --locked --manifest-path editor/src-tauri/Cargo.toml -p vactr`: exit 0; `cargo-tree.log`.

`snapshot-audit.json` matches all eight runtime pre-node owned files by bytes/modes.
`source-stability.json` confirms ten checked paths unchanged during verification.
No dependency locks regenerated; 430 baseline Cargo identities/checksums and existing
npm package records remain preserved. Historical failures and blocked native decisions
remain intact. Fresh native acceptance is not claimed.

Read-only `/root/package_contract_review` found no material issue in preparation scripts,
scoped dependency changes or accepted design alignment. Improve author self-check verified
required gates, positive counts, complete logs, source stability and ownership: no unresolved
high/mid finding or implementation-phase verification gap. Immutable append intent,
preimage/diff/postimage: `edit-progress/`, mirrored under
`/private/tmp/vactr-224-implementation/CE-PACKAGE/retry-20260930T203042506761/edit-progress/`.

Implementation-phase work is complete. Runner progress, test-integrity, adversarial and
integration decisions remain downstream; their checkboxes stay unchecked. Submit this
fresh Step 6 JSON to the native gate without inventing acceptance. CE-SHELL admission
requires runtime acceptance. Shared indexes/archive/manifest and Git finalization belong
later steps. No Rust, Swift, TypeScript, dependency or Git state changes in this run.
