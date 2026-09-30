# CE-PACKAGE: Canvas editor preparation implementation plan

**planId**: CE-PACKAGE
**planPath**: impl-plans/active/canvas-editor-224-package-preparation.md
**Status**: Ready for Step5 review; implementation not started
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
| P1 | package.json and package-lock.json | Not started |
| P2 | src-tauri/Cargo.toml and Cargo.lock | Not started |
| P3 | dependency-evidence.md | Not started |

### P1: package.json and package-lock.json
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add exact local playwright1.62.1 dev dependency using npm install --save-dev --save-exact playwright@1.62.1 --ignore-scripts. Record source/version/integrity and diff; do not upgrade other packages. Global playwright shim has no configured version, so consumers use the node runner with local import. Install Chromium via local CLI only as foreground bounded preparation; retain its logs and record unavailable browser/network prerequisites without passing evidence.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### P2: src-tauri/Cargo.toml and Cargo.lock
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Add local root vactr path dependency with host-native enabled. Keep locked Tauri/plugin versions; no new core Cargo dependencies. Supply-chain-secure-install skill required. Generate lock serially via CARGO_TERM_QUIET=true cargo generate-lockfile --manifest-path editor/src-tauri/Cargo.toml; inspect new native transitive dependency tree and ensure root Cargo.lock untouched. Downstream native shell changes only targets/entry/config and uses locked checks.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### P3: dependency-evidence.md
**Status**: Not started
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Record pinned artifacts/integrities, full install/generate logs and statuses. If exact dependency cannot resolve, report real dependency verification failure, not workflow-readiness failure; request serial plan amendment before changing pin. Include npm exec -- playwright install chromium output and actual browser install path/version for final runner.

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
- [ ] Task-only pinned dependency diffs and complete logs
- [ ] Browser binary available or verification gap explicitly recorded; no worker regenerates locks
- [ ] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.
