# Canvas editor session 224 execution contract

Issue: codex-design-and-implement-review-loop-session-224. Workflow mode: issue-resolution.
Design accepted by Step3 communication comm-002814, version0.3.52; no design findings.
This node writes plans only; historical implementation evidence remains intact. New recovery criteria are unchecked.
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

## Session235 recovery contract (supersedes stale pending instructions above)
Step3 accepted existing design15.3; design checkpoint8ee36f23e2311455126f6486db992d33227ff3e2 and accepted-plan checkpoint4fc37414e2f2704f6737f1545071bf506bd9c0f8 are historical. Current amendments require independent Step5 acceptance, then coordinator-only owned-plan commit/non-force push before dispatch. Operator authorization persists; no permission rediscovery or registry tasks.

Seven nominal waves remain a DAG:
1. CE-CONTRACT
2. CE-GPU, CE-TELEMETRY, CE-PACKAGE
3. CE-STATE, CE-AUDIO, CE-CLOCK
4. CE-INPUT, CE-VISUAL, CE-SHELL
5. CE-CONSUMERS
6. CE-JOIN
7. CE-FINAL
Runtime may retain accepted CE-CONTRACT/CE-STATE/CE-INPUT using source-matched evidence; nominal edges are not deleted. CE-GPU is excluded from retained acceptance until first-frame cache repair/review. After admission, remaining nominal waves are GPU/telemetry/package; audio/clock; consumers/visual/shell; join; final. Use runner native acceptedDependencies mechanism and provenance checks, not manual accepted flags; rejected retained provenance returns its owner to evidence-only verification. Post-GPU state/input reconciliation remains mandatory. Gate capacity/retry lifecycle is runner-owned; never repair workflow package here or bypass independent review.

No current source allocation is added for unrelated27 lint diagnostics. Step5 explicitly decides the telemetry scoped-baseline proposal; canonical global Clippy exit101 remains failed. Both historical telemetry blocked fingerprints and package blocked fingerprint remain untouched. Rust verification agent continues all eight independent commands after lint failure. Device/compiler preparation stays CE-PACKAGE; generated Apple outputs stay serial CE-FINAL. Exact tool pins come from recorded official release compatibility, never guessed library/CLI equality; no application lock/version updates.


## Session236 resumed baseline and external dependency admission
**Status**: Bounded metadata amendment authored; new Step5 review/checkpoint pending.
Session235 Step5 accepted the behavior/tooling amendments in comm-003003
(`tmp/canvas-editor-224/root-observations/session-235-accepted-plan-review.json`).
Those amendments and all13 plan files/nominal edges are retained. No design restart,
source repair, tool installation, cap change or implementation acceptance occurs here.

Current resumed intake is main at4e34e25039bdbe8521e1a5aa014e687d4de2b912,
with origin/main tracking the same revision and origin URL https://github.com/tacogips/vactr.git.
Evidence: `tmp/canvas-editor-224/plan-amendment-236/git-context.json` and complete
per-command logs beside it. Original2793, design8ee36f23 and prior plan4fc37414
are historical identities only; do not populate current checkpoint intake from them.
The operator's `root-observations/concurrent-checkpoint-4e34e25.json` records a
successful remote lookup. Fresh lookup in this node exits128 (github.com DNS unavailable);
it does not establish current remote state. Checkpoint coordinator rechecks HEAD/branch,
scopes only current task plan diffs, then requires the authorized non-force push to
succeed before fanout. Preserve the separate committed baseline and any new foreign hunks.

`tmp/canvas-editor-224/plan-amendment-236/retained-source-proof.json` matches17
CONTRACT/STATE/INPUT source/test identities against formal comm-002949 review evidence,
current bytes and the committed4e34e25 objects. STATE plan-only retention amendments
were accepted in comm-003003; historical gate failures stay preserved. New Step5 must
independently confirm this proof before checkpoint admission; this author creates no gate
acceptance. Keep each accepted dependency's planPath, commit and provided contract in
acceptedDependencies; plans[] contains only the ten remaining owners. All13 acceptedPlanPaths
stay referenced. Do not retain GPU: G4 repair and all independent gates are still required.

The inspected dispatcher validates external IDs and removes their edges from projected
remaining items; its worker acceptedPlanIds contains only remaining dispatched IDs. Thus
workers read external contracts/commit proof from reviewContext.externalAcceptedDependencies
and this contract, instead of requiring external IDs in worker acceptedPlanIds. Seven
nominal waves remain documented; five projected remaining waves are GPU/telemetry/package,
audio/clock, consumers/visual/shell, join, final. No runner source changes or Git fanout.
If source proof or formal continuation admission fails, serial coordinator amends dispatch
for evidence-only verification by that named owner and obtains review/checkpoint before
redispatch; never restore source snapshots or silently replay accepted implementation.

After GPU repair acceptance, serial CE-FINAL reruns all three STATE S4 commands,
INPUT typecheck/input suite and the combined code/app/bind/params/canvas/visual/protocol
suite with new complete logs/source hashes. Nominal GPU dependencies are not erased by
external admission; retained behavior must pass on repaired and final joined bytes.
Each remaining worker updates only its own plan log; shared indexes, final archiving,
lock reconciliation, lost-intent repair and Git remain serial.

### Progress Log: 2026-09-30 — Step4 session236
Bounded baseline/manifest repair only. Seventeen retained source/test hashes matched
reviewed evidence and committed objects. Independent metadata review, accepted-plan
checkpoint/non-force push, G4/T5/P5 and remaining implementation/native/device gates
remain pending. No implementation tests, production edits or Git mutations performed.

## Session237 snapshot and current-baseline recovery (latest precedence)
Accepted design15.3/comm-002814 and historical plan reviews comm-003003/comm-003011 remain
baseline; source236 failed terminal policyBlocked. No design restart, source staging or manual
implementation acceptance. Latest intake is main7f6de4e1c1bd8690ca45c69ca700c809c154cce9;
origin/main locally matches. Fresh ls-remote exit128 (DNS) does not prove current remote state.
Evidence `tmp/canvas-editor-224/plan-amendment-237/git-context.json` has complete per-command logs.
Step5 independently admits current baseline and exact planning delta, then coordinator stages
only reviewed canvas planning paths/hunks, commits and requires non-force push success before
fanout. Dirty implementation repairs, all foreign song-mode files and impl-plans/README.md are
excluded. Retain historical4e/3ce identities separately; never overwrite current intake with them.

Manifest has13 acceptedPlanPaths, ten remaining plans and three committed external dependencies.
Fresh proof matches16/17 historical hashes; deps.ts differs only by committed additive formatter,
syntax and completion imports/optional fields. Exact diff and committed/current hashes are in
`plan-amendment-237/retained-source-proof.json` and `retained-deps-delta.diff`. Step5 must inspect
and independently admit that delta before retention; otherwise serial evidence-only amendment,
review and checkpoint are required. No new acceptance asserted here. GPU/telemetry child success
is not root integration admission: retain both as pending native plans, preserve their repairs/logs,
and verify matching evidence before retry. Post-GPU STATE/INPUT and combined gates still required.

CE-PACKAGE source tracking removes recursive native-tools from writePaths/sharedPaths; exact
small toolchain.json remains tracked alongside every authored source/plan. Dispatcher trackedPaths
is writePaths+sharedPaths, so changedFiles-only narrowing cannot repair failure. Generated binary,
cache/download/install and terminal logs remain referenced evidence, not tracked source outputs.
Preserve512-entry/8MB-file/64MB-total limits and all source ownership/symlink protection. No cache
deletion, tool reinstall or runner policy edit. Exact evidence checksums/logs stay audited.
Current package preservation baseline is proposed in plan-amendment-237/package-current-intake.json
with immutable snapshot/current/commit hashes; independent Step5 admission required. Historical
attempt1 checks retain real failures distinctly. Only CE-PACKAGE updates its verifier; application
versions/locks are unchanged. CE-SHELL/FINAL verify available device+simulator components rather
than inferring compiler unavailability from lack of physical iPad.

CE-JOIN owns three additional concrete adapter files format.ts/syntax.ts/completion-view.ts and
existing test/code fixtures; UI-agnostic cores are read-only. Preserve current shortcut, tree-sitter
fallback and completion behavior without new backend or hidden view. Nominal seven-wave DAG and
projected five remaining waves are unchanged. Shared indexes, lock reconciliation, broad formatting,
archives and Git stay serial; each worker updates only its own progress.

All full-input verification must be refreshed if inputs changed, including current song-mode/bass/
formatter changes. GPU scope hashes allow only its scoped evidence retention. Canonical global
Clippy exit101 remains failure; historical27 diagnostic baseline cannot be reused after diagnostic
source drift. CE-TELEMETRY and CE-FINAL require fresh independent reconstruction/provenance of
current pre-task unowned diagnostics, zero introduced/owned diagnostics, no unknown blocks, and
reviewed scoped disposition; new failures remain failures and no unrelated cleanup ownership added.
Rust verification agents and all eight separate gates remain required. Compile, signing, simulator,
physical-device and hardware synchronization/budget evidence remain distinct and honest.

### Progress Log: 2026-10-01 — Step4 session237
Bounded plans/manifest recovery authored. Immutable preimages and intents, current git/package and
retained hashes under plan-amendment-237. No source modifications, installs, Git mutations or
application passes claimed; new independent Step5 review/checkpoint/fanout remain downstream.

### Author improve check: concurrent foreign drift
Initial full-source stability audit failed (verify.log: Python exit1; shell wrapper also
failed on zsh read-only variable status). Complete original log is retained. Ten structural
checks rerun with corrected scoped audit pass (verify2.log exit0), while foreign drift
is explicitly recorded in verify-2/author-self-check.json. Seven src/pattern/combinators
and src/types/song_rules.rs files changed concurrently; this node wrote planning files
only and no canvas source hash changed. Preserve those foreign changes; they invalidate
whole-input native/Wasm/lint reuse. Owners capture fresh current sources and independently
review renewed baseline provenance before accepting results. This is no gate waiver.
