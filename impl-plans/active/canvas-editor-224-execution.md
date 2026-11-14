# Canvas editor session 224 execution contract

Issue: codex-design-and-implement-review-loop-session-224. Workflow mode: issue-resolution.
Design accepted by Step3 communication comm-002814, version0.3.52; no design findings.
This node writes plans only; all implementation criteria initially unchecked.
Code-free instructions override impl-plan skill examples requiring Rust code.

## Ownership and drift protocol

All plans use main in this existing checkout. No worktrees/private branches or worker Git operations.
Read this contract, own plan, accepted15.3 and every target file immediately before each edit.
Capture full current target bytes and SHA256 plus git diff versus HEAD to immutable task/session
snapshots under /private/tmp/vactr-224-implementation/<planId>/<edit-number>/ BEFORE editing.
Create snapshot files exclusively, make them read-only, and never overwrite an earlier edit intent.
Snapshot the intended change, predecessor contracts and preserved dirty hunks separately.
Compare expected prehash immediately before writing; if changed, stop that edit, fresh-read,
rebase only its intent on new bytes and record drift. Never restore older whole-file snapshots.
After edit capture posthash and diff; compare all touched paths to manifest allowlist and baseline.
Do not alter baseline MusicDSP/rumble hunks. Existing protocol/types.ts, session/protocol.rs,
session/editors.rs and indexes have unrelated work; only named owner may add task hunks.
Send predecessor contract snapshots to successor after completion; successor verifies posthash.
Before joining, reconcile all worker posthashes serially, inspect current diffs and repair lost
intent with fresh reads; preserve both user hunks and accepted task behavior. Whole directories
in manifests grant task-file changes only, never permission to replace unrelated content.
During staged migration retain compatibility exports/types until CE-JOIN removes them.
Headless controllers may be added alongside legacy attach wrappers, but do not instantiate
EditorView in new production code. CE-JOIN removes all legacy wrappers and ports old tests.
A whole-editor check blocked solely by staged sibling migration is not a pass; record
exact failures and run isolated new behavior tests, then CE-JOIN must resolve all before acceptance.
Every worker changes only its own progress log; shared indexes/broad formatting/archive
belong CE-FINAL; initial lock/dependency preparation belongs serial CE-PACKAGE. CE-JOIN alone owns editor cutover and serial consumer repair.

## Review and Git gates

Step5 independently reviews ALL plans. No implementation before acceptance plus task-owned
design/plans commit and successful non-force push. Coordinator (not worker) stages only task
paths/hunks; inspect git diff --cached --name-status and --stat, verify no baseline user hunks,
show mandated commit summary and commit message, git commit then git push origin main.
Authorization is already supplied; do not ask again. DNS/push failure is a repository gate,
not workflow provenance failure; keep implementation fanout pending until push succeeds.
Do not stage dirty impl-plans/README.md wholesale; add task rows only during CE-FINAL.
Final Git operations serialize after review and required checks; no third-party messaging/PRs.

## Required tools and evidence

Rust implementation uses .agents/agents/rust-coding.md with design/plan/file deliverables.
After ANY Rust modifications invoke .agents/agents/check-and-test-after-modify.md with exact
modified files and test intent. Read rust-coding-standards and relevant supply-chain skills.
Any touched Rust file reaching1000 lines is split into cohesive siblings under its owning
module; record exact added path in manifest by serial amendment before later fanout.
All Cargo commands use CARGO_TERM_QUIET=true. nextest additionally uses
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1.
Workers run local targeted checks; serial finalization runs broad checks. Use --locked for shell check/test after CE-PACKAGE, so commands never regenerate dependency locks.
No broad formatter
writes against dirty user files: format owned sections then cargo fmt -- --check, reporting
pre-existing failures with evidence instead of changing unrelated files.
Run commands in foreground, capture complete stdout/stderr to per-command logs, record
command/cwd/start/end/exit and retain/poll yielded session until exit. Do not detach services.
Browser node runner owns its finite HTTP server and browser and closes them before returning;
interactive device/manual sessions use runner-owned lifecycle and explicit captured evidence.
Logs are complete only after terminal exit. A missing/incomplete log is not passing evidence.
A failed/skipped/unavailable gate stays unchecked with reason; unit tests are notGPU/IME/iPad
or physical synchronization proof. Apply improve after own output/diff and fix material issues.

## Progress and completion

At each session append timestamp, tasks completed/in progress, hashes/intent snapshot paths,
commands with complete log paths and exit statuses, findings/fixes, remaining evidence and
next dependency. Check a criterion only with evidence. Mark In Progress while editing;
Completed requires its actual criteria, including hardware gates where applicable. CE-FINAL
moves only completed plans and updates only task index rows. Do not erase logs when archiving.
No Cursor adapter: supplied codex-agent references are repository contracts, not UI behavior.
Intentional divergence: GPU source supersedes visible CodeMirror; output correlation supersedes
receipt timing. All other language/binding/browser-first/native host decisions remain.

## Session 227 rerun boundary and native gates
Runner runtimeVariables/workflowInput and resolved provenance are authoritative. This
Step4 amends remaining work only; Step3 accepted design 15.3 remains the baseline. No
workflow/package registry discovery or repair is part of any plan. Historical issue224
and plan IDs/paths stay stable; rerun execution is session227.
State retry follows CE-GPU acceptance; CE-TELEMETRY alone owns midi_clock.rs generation
observation and publisher consumption; CE-PACKAGE retries installed preparation without
regenerating valid locks. Existing CE-CONTRACT/CE-GPU checks do not automatically admit
pending workflow reviews. Native blocked results stay blocked until runner-owned retries
and review gates actually succeed. Before dependent fanout use runtime-accepted plan IDs,
not Markdown checkboxes or a manually changed manifest status.

For every retry, supply testsRun/testsPassed/failureCount (actual assertions), exact
commands/cwd, complete log paths, terminal exits and pre/post source hashes to the native
progress gate using the implementation node's runner-supplied mechanism. Retain returned
decision and review references in the own-plan log. Never invent a gate CLI or manufacture
accepted results. A missing result remains pending. Formal test-integrity, adversarial and
integration review follow implementation verification; improve author review is additional.
Workers append own progress only. Serial coordinator reconciles source hashes and accepts
contracts before successor waves; indexes, manifest updates, lock reconciliation, archive
and Git remain serial. Step5 must accept these amended plans, then coordinator commits
and non-force pushes owned plan amendments before further implementation fanout. Current
Step4 does not commit unreviewed amendments or execute the planned implementation tests.
