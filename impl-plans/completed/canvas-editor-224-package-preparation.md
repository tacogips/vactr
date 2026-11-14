> Superseded by the canvas-cutover plans (completed 2026-10-07).

# CE-PACKAGE: Canvas editor preparation implementation plan

**planId**: CE-PACKAGE
**planPath**: impl-plans/completed/canvas-editor-224-package-preparation.md
**Status**: In Progress — P6 implementation verified; native progress and formal reviews pending
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
- impl-plans/completed/canvas-editor-224-dependency-evidence.md
- impl-plans/completed/canvas-editor-224-package-preparation.md
- tmp/canvas-editor-224/CE-PACKAGE/native-tools/toolchain.json

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

## Session 235 P5 native prerequisites and runner reconsideration
**Status**: Implementation complete; runner reconsideration and formal reviews pending. **Parallelizable**: Yes with GPU/telemetry; preparation serial within plan.
**Design trace**: 15.3.6 requires buildable mobile integration; tooling absence is distinct from physical-device absence.
P1-P4 exact versions and valid locks remain unchanged. Do not reinstall working Playwright or regenerate locks. Native blocked fingerprint `aebbbc2b8437d2d47e765ae65108524c10bddc2760d99737fca5a003262116af` remains historical until replaced.

First request runner-owned reconsideration using existing source-matched evidence `tmp/canvas-editor-224/CE-PACKAGE/retry-20260930T203042506761/`: browser-smoke.log exit0 4/4, dependencies.log exit0 10/10, metadata/tree exit0, source-stability.json. Recognition repair is runner-owned; `root-observations/classifier-repair-check.json` has8/8 classifier checks, not application acceptance. Preserve positive assertions, zero failures, final exit and source checks. Formal integrity/adversarial/integration gates remain mandatory; no manually accepted flag. If new tooling alters evaluated inputs, supplement fresh checks with unique output paths and complete terminal metadata.

P5 generates task-local evidence under `tmp/canvas-editor-224/CE-PACKAGE/native-tools`; only its exact small `toolchain.json` record and own dependency-evidence/progress files are tracked deliverables. Generated tools/caches/logs are referenced evidence, not recursive source ownership. No application dependency update or shared mise policy edit. Read supply-chain-secure-install before tool installation. Commands below run foreground; capture actual selected toolchain, SDK and exact CLI release/source/checksum before installation:
- `RUSTUP_TOOLCHAIN=1.98.1 rustup target list --installed`
- `xcodebuild -version` and `xcodebuild -showsdks`
- `RUSTUP_TOOLCHAIN=1.98.1 rustup target add aarch64-apple-ios aarch64-apple-ios-sim --toolchain 1.98.1`: install only missing compiler targets for the existing chosen toolchain; no toolchain update. If sandbox/network denies, log actual denial separately from device absence.
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_HOME=tmp/canvas-editor-224/CE-PACKAGE/native-tools/cargo-home CARGO_TERM_QUIET=true cargo info tauri-cli --registry crates-io`: inspect official CLI metadata and its release compatibility with locked tauri2.11.6/tauri-build2.6.3. Do not assume CLI equals library version. Resolve a single exact supported release, record it in native-tools/toolchain.json and append the literal install command to own progress before running. Review source/version/lock/license/build-script inputs per supply-chain skill; if compatibility cannot be established, block tool preparation honestly.
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_HOME=tmp/canvas-editor-224/CE-PACKAGE/native-tools/cargo-home CARGO_TERM_QUIET=true cargo install tauri-cli --version "${CE_TAURI_CLI_VERSION}" --locked --root tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli`: CE_TAURI_CLI_VERSION must be the one exact recorded release above, not latest/range; set it from reviewed metadata and preserve literal expanded command in logs. This installs into task-local root, not global Cargo bin.
- `CARGO_TERM_QUIET=true tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli/bin/cargo-tauri --version`
- `RUSTUP_TOOLCHAIN=1.98.1 rustup target list --installed`: confirm both targets. Hash CLI binary and record toolchain/release identities for CE-SHELL/CE-FINAL; inspect app manifests/locks pre/post to prove no drift.

Do not run mobile generation or compile concurrently in P5; CE-SHELL owns shared library target checks, CE-FINAL owns generated Apple integration and simulator packaging. Prerequisite downloads/install use this evidence directory; if an installation restriction occurs report exact command/log/exit. Missing signing/physical device never exempts available unsigned compiler checks. Preserve old missing-cli exit101 as historical, not permanent.
- [ ] Runner replaces blocked package decision using valid evidence and formal reviews.
- [x] Exact compatible task-local CLI and iOS/device+simulator Rust targets available, or actual install failure documented with full exit/log.
- [x] Application dependency versions/locks unchanged; compiler, signing, simulator and physical-device statuses separate.
### Session: 2026-09-30 — Step4 session235
P5 fills native prerequisite gap within existing package ownership. P4 evidence submitted for later runner reconsideration; no repeated implementation smoke or lock regeneration in this node.

### Session: 2026-09-30 21:34 — P5 prerequisite preparation started
Runtime dependsOn is empty; retained CE-CONTRACT proof is external admission. P5 aligns with accepted design15.3.6. Xcode26.6 and iOS26.5 SDK inventory exit0; Rust1.98.1 iOS device/simulator target installation exit0. Official registry stable tauri-cli2.11.5 is selected using v2 CLI policy and matching locked tauri-utils2.9.3 schema, not library patch equality. Metadata/source/build-script/license/checksum review: `tmp/canvas-editor-224/CE-PACKAGE/native-tools/toolchain.json`. Exact foreground command recorded before execution:

`RUSTUP_TOOLCHAIN=1.98.1 CARGO_HOME=tmp/canvas-editor-224/CE-PACKAGE/native-tools/cargo-home CARGO_TERM_QUIET=true cargo install tauri-cli --version 2.11.5 --locked --root tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli`

Evidence directory: `tmp/canvas-editor-224/CE-PACKAGE/native-tools/attempt-20260930T213229`. No application dependencies or locks will be regenerated. CLI compilation is tool preparation only; app compile, signing, simulator packaging and physical device remain separate downstream gates.

### Session: 2026-09-30 21:42 — P5 implementation handoff

P5 preparation completed. Exact task-local tauri-cli2.11.5 installed with Rust1.98.1 and --locked; install exit0, version exit0, binary hash recorded. Both iOS compiler targets installed and confirmed. Xcode26.6 and iOS26.5 device/simulator SDK inventory exit0. Application manifests/locks and both portable runners remained byte-identical. No Rust/Swift/TypeScript source edits, Git operations, global CLI install, app dependency updates or lock generation.

Fresh source-matched behavioral checks: browser4/4, dependency10/10, zero failures; locked metadata/tree exit0. Complete command evidence and counts: [dependency evidence](canvas-editor-224-dependency-evidence.md#session-2026-09-30-2142--p5-native-tool-preparation-terminal-evidence) and `tmp/canvas-editor-224/CE-PACKAGE/native-tools/attempt-20260930T213229`. Final identity record: `final-source-stability.json`; exact source/license/checksum/schema compatibility record: `tmp/canvas-editor-224/CE-PACKAGE/native-tools/toolchain.json`.

Read-only `/root/package_review` found no material issue; improve self-check found no unresolved high/mid issue or assigned implementation verification gap. External CE-CONTRACT proof remains retained; runtime dependsOn is empty. Submit fresh actual4/4 and10/10 counts for runner-owned reconsideration; no blocked classification or accepted flag edited. Formal reviews, review-dependent completion record, manifest/index/archive and Git are later workflow steps. Remaining unchecked review/runner criteria deliberately remain pending downstream. CE-SHELL owns app compilation; CE-FINAL owns generated Apple/simulator packaging; signing and physical device remain distinct/unverified. Operator GPU/telemetry findings are preserved for owners and are not package acceptance.

## Session237 bounded recovery contract (supersedes historical retry/install commands)
**Status**: Step5 amendment accepted (comm-003052); P6 implementation verified, native/formal acceptance pending.
**Context**: Current main7f6de4e contains accepted external formatter/syntax/completion/bass
integration; the package files differ from original attempt1. Preserve all original snapshots
and failed logs. Source236 is terminal failed policyBlocked; GPU/telemetry children completed
without root admission. Historical P1/P2/P5 are implemented: do not rerun installs, regenerate
locks or alter application versions to satisfy an old preservation comparison.

### P6: Current-input preservation and bounded native snapshots
**Parallelizable**: Yes with CE-GPU/CE-TELEMETRY; serial within package; no Git operations.
**Deliverables**: existing package-dependencies.py, dependency-evidence.md, own plan progress,
and exact native-tools/toolchain.json record only. No additional dependency or backend scope.
Read runtime manifest, package-current-intake.json and every current package/lock/runner before
editing. Apply execution contract immutable intent/pre/posthash/diff and drift checks. Step5
must independently admit the source-matched current-intake before this retry. Never silently
replace historical attempt1 baseline or relabel its comparison as successful.

Update package-dependencies.py narrowly to add explicit --current-intake mode (mutually exclusive
with historical --baseline). Verify immutable snapshot hashes, recorded commit identities and
current versions/lock records against the independently admitted intake. Require existing exact
Playwright1.62.1 and native path/feature contract, registry integrity/checksums, unambiguous local
vactr closure, positive assertion counts, zero failures, and source stability pre/post. Compare
current-intake immutable npm/Cargo snapshots instead of fixed historical430-count assumptions;
any additional post-intake dependency drift fails. Preserve original historical mode assertions
and output failures; the old check is historical evidence, not the current preservation gate.
An unexpected current-mode failure blocks package acceptance; no lock regeneration permitted.

Snapshot contract: dispatcher trackedPaths is deduplicated writePaths+sharedPaths. Directories
recurse before/after every node. Keep512 entries,8MB/file,64MB total and symlink/source ownership
protections unchanged. Remove recursive native-tools from both source tracking lists; track only
its exact1508-byte toolchain.json plus eight existing source/plan paths. Logs, downloaded CLI,
Cargo cache and targets are generated evidence artifacts, retained and referenced through complete
logs/checksums and dependency-evidence.md; never submit their directory as changedFiles or owned
source. Any authored application/script edit must stay in concrete writePaths; any unexpected
source edit is a real ownership failure. No runner/package source edits or policy waiver.

Reuse installed tauri-cli2.11.5 only after checking its version and SHA256 against toolchain.json
and root-observations/native-tool-renewal-236.json (2e6e37f8372015c8807342b8c3c8e80739d02b1787448fb6177f78e191848879).
Confirm Rust1.98.1 device/simulator targets and Xcode/SDK inventory. Preparation already has
terminal successful installation evidence; do not reinstall verified prerequisites. If inventory
actually drifts, report exact failure for serial review before any bounded prerequisite repair.
CE-SHELL owns app library compile; CE-FINAL owns Apple generation/packaging. Signing, simulator
execution, physical iPad and compile evidence remain separate.

**Commands and evidence** (repository root, create recovery-237 output directory exclusively;
if already present use a new recorded attempt suffix and expand exact commands in own log):
- `node editor/test/canvas/package-browser-smoke.mjs`: actual Chromium/WebGL2 assertions, positive counts, zero failures, terminal exit0; recognized native command keeps all assertion/exit checks.
- `python3 editor/test/canvas/package-dependencies.py --baseline tmp/canvas-editor-224/CE-PACKAGE/attempt-1 --output tmp/canvas-editor-224/CE-PACKAGE/recovery-237/historical-comparison.json`: retain true historical result, including any failures due to committed integration; never treat it as a current pass.
- `python3 editor/test/canvas/package-dependencies.py --current-intake tmp/canvas-editor-224/plan-amendment-237/package-current-intake.json --output tmp/canvas-editor-224/CE-PACKAGE/recovery-237/current-comparison.json`: planned new mode, not presently executable; prove all admitted records preserved, exact pins/closure/integrities, source stability, positive counts and zero failures.
- `CARGO_TERM_QUIET=true cargo metadata --locked --manifest-path editor/src-tauri/Cargo.toml --format-version 1`: complete resolved graph without lock mutation.
- `CARGO_TERM_QUIET=true cargo tree --locked --manifest-path editor/src-tauri/Cargo.toml -p vactr`: native feature closure and exact graph.
- `RUSTUP_TOOLCHAIN=1.98.1 rustup target list --installed`: device and simulator compiler prerequisites present.
- `xcodebuild -version` and `xcodebuild -showsdks`: actual selected Xcode/SDK inventory.
- `CARGO_TERM_QUIET=true tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli/bin/cargo-tauri --version`: exact2.11.5 reusable CLI.
- `shasum -a 256 tmp/canvas-editor-224/CE-PACKAGE/native-tools/cli/bin/cargo-tauri tmp/canvas-editor-224/CE-PACKAGE/native-tools/toolchain.json`: tool/source-review identities agree.
Retain every complete command log, final status and source hashes. Historical input matches alone
are insufficient after new integration. Submit fresh behavioral results to native progress and
all independent reviews; author output or a successful child is not root acceptance.

**Completion criteria**:
- [x] Independent Step5 admits current-intake package preservation and bounded snapshot contract.
- [ ] Native snapshot and ownership checks pass without policy/cap changes; full generated evidence retained.
- [x] Current-input behavioral and preservation checks pass; original historical failures retained distinctly.
- [x] Prerequisites verified and reused; compiler/sign/simulator/device statuses explicit.
- [ ] Native reconsideration, test-integrity, adversarial and root integration accept the actual handoff.

### Progress Log: 2026-10-01 — Step4 session237
Bounded planning amendment only. Recursive native-tools tracking replaced by exact audit record;
P6 preserves current committed dependency intake without hiding historical failures. Source/logs,
installed tools and application locks untouched. Intent/preimages/git/package-source evidence:
`tmp/canvas-editor-224/plan-amendment-237/`. Implementation commands above remain later work.


### Progress Log: 2026-10-01 — P6 implementation handoff
Current-intake mode implemented and verified on final source (verifier SHA256
7c8fe1afadc33412c4a8b080ec3c5e8ab045144a4370079345c910bb022b7fca).
Mutually exclusive historical mode retains authentic exit1,9/10, one failure due to admitted
web-tree-sitter addition; old/new assertions identical. Current preservation14/14 and real
Chromium/WebGL2 smoke4/4 pass. Tools reused: CLI2.11.5/version/hash, Rust1.98.1 iOS targets,
Xcode26.6/iOS26.5 SDKs all verified, locked metadata/tree exit0. Source pre/post equality and
native nine-file snapshot/cap audit12/12 pass; generated caches remain untracked evidence.
Bounded independent author assistance `/root/package_review` found one low symlink-check issue,
corrected before final reruns. Negative/equivalence checks6/6 pass. Improve self-check has no
unresolved high/mid finding or assigned verification gap. Exact commands/counts/status/logs:
[dependency evidence](canvas-editor-224-dependency-evidence.md#session-2026-10-01--p6-admitted-current-intake-preservation)
and `tmp/canvas-editor-224/CE-PACKAGE/recovery-237/`.
Fresh per-edit bytes/intents/pre/posthash/diff: recovery-237/edit-001..004 and mirrored
`/private/tmp/vactr-224-implementation/CE-PACKAGE/recovery-237/`. Existing dependency-evidence dirty
content retained. No application dependency, lock, toolchain audit, Rust/Swift/TypeScript, shared
manifest/index or Git mutation. Native post-node progress/reconsideration and all formal reviews
remain downstream; no author acceptance flag. CE-SHELL app compile and CE-FINAL Apple packaging,
signing/simulator execution/physical-device evidence remain distinct downstream obligations.
Known GPU/telemetry operator findings preserved for owners; package success cannot waive them.
