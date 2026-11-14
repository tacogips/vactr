# CE-PACKAGE: Canvas editor preparation implementation plan

**planId**: CE-PACKAGE
**planPath**: impl-plans/active/canvas-editor-224-package-preparation.md
**Status**: In Progress — native blocked result retained; preparation retry and formal reviews pending
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
| P4 | Portable browser/dependency retry and native/formal gates | Not started |

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
**Status**: Not started
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
- [ ] Portable installed-browser smoke and dependency preservation assertions exit 0.
- [ ] Full locked metadata/tree exits 0 and dependencies stay unchanged across retry.
- [ ] Native progress gate accepts implementation-exercising results/source identities.
- [ ] Formal workflow reviews accepted with no unresolved high/mid findings.

### Session: 2026-09-30 — Step4 session 227 amendment
P4 added with two preparation-specific verification scripts and exact commands. Native
blocked result remains authoritative; no command, formal review or native progress retry
was executed by this planning node. Existing browser/dependency assertions retained.
