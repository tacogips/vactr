# LPI-CLOSEOUT: full gates, section 12 record, archive and README (finishes LP-EVIDENCE)

**Status**: Ready
**Plan ID**: LPI-CLOSEOUT (wave 4)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` section 7; `design-docs/specs/design-live-performance.md` 8.4 and 9; `impl-plans/active/live-perf-evidence.md` TASK-V3 to TASK-V5
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Intent and Context

After LPI-WIRE, LPI-VIM-STOP, LPI-NS-INDEX, LPI-PLAYBACK and LPI-BEHAVIOR,
the merged tree should be fully green. This plan:

- proves that with every gate, run serially;
- writes the verification record into design-live-performance.md as
  section 12;
- completes and archives the seven live-perf plans and
  `live-perf-dispatch.json`;
- updates `impl-plans/README.md` and `README.md`.

This finishes LP-EVIDENCE TASK-V3 to TASK-V5 as specified in its plan, with
the integration adjustments in design section 7.

## Non-goals

- No product or test code change. A failing gate is reported with its log
  path and the owning plan (see "If a gate fails"); it is never fixed or
  loosened here.
- No `--write-evidence` e2e run, and no edit to the canvas `run-001`
  evidence.
- Do not archive the `live-perf-int-*` plans or
  `live-perf-int-dispatch.json`. The running workflow references them at
  their active paths. They move in the integration commit after final
  review (precedent: the vim-keymap dispatch note in
  `impl-plans/README.md`).
- No commit or push. The workflow's integration step commits on
  `merge/live-perf`. Merging into main is out of scope.

## Dependencies

- **dependsOn**: LPI-WIRE, LPI-VIM-STOP, LPI-NS-INDEX, LPI-PLAYBACK,
  LPI-BEHAVIOR.
- Runs alone in wave 4.

## writePaths

- `design-docs/specs/design-live-performance.md` (append section 12 only)
- `design-docs/specs/design-live-perf-integration.md` (status line only)
- `design-docs/specs/architecture.md` (status cell of the 2026-10-18
  integration row only)
- `README.md`
- `impl-plans/README.md`
- `impl-plans/active/live-perf-contract.md`
- `impl-plans/active/live-perf-engine.md`
- `impl-plans/active/live-perf-session-momentary.md`
- `impl-plans/active/live-perf-session-stop.md`
- `impl-plans/active/live-perf-editor-momentary.md`
- `impl-plans/active/live-perf-editor-stop.md`
- `impl-plans/active/live-perf-evidence.md`
- `impl-plans/active/live-perf-dispatch.json`
- `impl-plans/completed/live-perf-contract.md`
- `impl-plans/completed/live-perf-engine.md`
- `impl-plans/completed/live-perf-session-momentary.md`
- `impl-plans/completed/live-perf-session-stop.md`
- `impl-plans/completed/live-perf-editor-momentary.md`
- `impl-plans/completed/live-perf-editor-stop.md`
- `impl-plans/completed/live-perf-evidence.md`
- `impl-plans/completed/live-perf-dispatch.json`
- `impl-plans/active/live-perf-int-closeout.md` (this plan; Progress Log
  only)
- Artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules`, `editor/dist`, `editor/src-tauri/target`, `tmp`

## sharedPaths

None. Read-only: every other `impl-plans/active/live-perf-int-*.md`
(collect their verification results for section 12).

## Tasks

### TASK-C1: Serial gates

Run each gate alone, in this order, in the foreground. Log each to
`tmp/live-perf-int/closeout/<n>-<name>.log` with
`cmd > log 2>&1; echo "exit=$?" >> log`.

**LOCK** means: `until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done`,
then run, then `rmdir` the lock. Always release it (use a `trap`), and
never hold it while idle.

1. `CARGO_TERM_QUIET=true cargo build --all-targets`
2. `CARGO_TERM_QUIET=true cargo fmt --check`
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
5. `mise run build-wasm-release`
6. `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml`
7. In `editor/`: `npm run check`
8. LOCK, in `editor/`: `./node_modules/.bin/vitest run` (full default
   suite; testsRun > 0, failures 0)
9. LOCK, in `editor/`: `npm run test:perf`
10. LOCK, in `editor/`: `npm run test:style`
11. In `editor/`: `VACTR_REQUIRE_SESSION_ABI=1 npm run build`, then
    immediately LOCK:
    `npm run e2e -- --browser all --profile behavior --run-id live-perf-int-001 --out ../tmp/live-perf-int/closeout/e2e/live-perf-int-001`
    - exit 0, stdout JSON `pass: true` and `failureCount: 0`;
    - the four live-perf checks pass in chromium and webkit, together
      with every existing behavior, Vim and keymap check;
    - post-sink peak 0.
12. `mise run build-wasm-release`, then in `editor/`
    `VACTR_REQUIRE_SESSION_ABI=1 npm run build`, then immediately LOCK:
    `npm run e2e:examples` -> 8/8. Copy `tmp/examples-e2e/result.json`
    to `tmp/live-perf-int/closeout/12-examples-result.json`.
13. LOCK:
    `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-fail-fast`
    - command timeout of at least 2400 s;
    - testsRun > 0, failures 0.

**Re-runs.** A timing-sensitive test may be re-run once, as its own logged
record. This covers `test/e2e/large-eval.test.ts` scaling, `test:perf`
and e2e timing. If it fails twice, it is a gate failure. Never change a
threshold.

**If a gate fails:**

- Stop.
- Record the command, log path, failing test and the owning plan in the
  Progress Log:
  - the wire or song tests belong to LPI-WIRE;
  - Vim and stop shortcuts belong to LPI-VIM-STOP;
  - `src/ns` belongs to LPI-NS-INDEX;
  - examples playback belongs to LPI-PLAYBACK;
  - behavior checks belong to LPI-BEHAVIOR;
  - anything else is reported as unowned.
- Report blocked. Do not continue to TASK-C2.

### TASK-C2: Section 12 record (`design-docs/specs/design-live-performance.md`)

Append `## 12. Verification record (integration, workflow session 433)`
after section 11 and before `## References`. Change nothing else in the
document. It contains:

- a table of gates 1-13: command, exit status, tests run and passed, log
  path (and the re-run record if a re-run happened);
- the four behavior checks per browser with pass status, and the e2e
  output path;
- the integration fixes, one line each, pointing to
  `design-live-perf-integration.md`:
  - I1: wire tags `0x1F`/`0x20`, song-only decode;
  - I2/I3: Vim fallback routing;
  - I4: the root cause established by LPI-BEHAVIOR's diagnosis, quoted
    from its Progress Log;
  - I5: the root cause established by LPI-PLAYBACK (or "stale build"),
    quoted from its Progress Log;
  - I6: name-space index equivalence proven by
    `src/ns/tests/migrate_index_equivalence.rs`.

  Only record root causes the diagnosis runs established; never record a
  hypothesis as a cause.
- the stated limitation: synthetic multi-touch only; the physical iPad
  two-finger check is pending.

### TASK-C3: Plan completion and archive

1. In each of the seven live-perf plans:
   - set `**Status**: Completed`;
   - add a final Progress Log entry dated 2026-10-18 that points to
     section 12.
2. In `live-perf-evidence.md`, also:
   - tick its completion criteria for the gates that passed;
   - record that gates ran under LPI-CLOSEOUT with the integration
     adjustments (union gate list, logs under `tmp/live-perf-int/closeout`).
3. In `live-perf-contract.md` and `live-perf-engine.md`, add a Progress
   Log note:
   - the tags were reassigned to `0x1F`/`0x20` (integration W1), because
     the case-sensitive pre-check grep missed the lowercase song constants;
   - the exact-length CtlMsg-first workaround in `ByteInbox::next` was
     removed (W2).
4. In `live-perf-dispatch.json`, add a top-level key
   `"archivedBy": "LPI-CLOSEOUT, workflow session 433, 2026-10-18"`.
   Keep the JSON valid.
5. Move the eight files one at a time from `impl-plans/active/` to
   `impl-plans/completed/` with the same names.
   - Use `mv`, not `git mv`; the integration commit stages the moves.
   - After each move, check that the target exists and the source is gone.
6. Verify the archived manifest with
   `jq . impl-plans/completed/live-perf-dispatch.json` (exit 0).

### TASK-C4: Indexes and docs

- `impl-plans/README.md`, section "Live performance controls (2026-10-08)":
  - status becomes Completed (2026-10-18, workflow session 433);
  - every link points to `completed/` paths;
  - one sentence names the integration addendum
    (`../design-docs/specs/design-live-perf-integration.md`) and the gate
    summary.
  - In the "Live performance integration" section that the plan author
    added, mark the six LPI plans Completed. Their links stay at
    `active/`, with the archive note.
- `README.md`:
  - The transport bullet (around line 203-208) says the stop button stops
    gently (`Mod-.`, effect tails ring out) and the cut/hush button cuts
    now (`Mod-Shift-.`).
  - Add a short "Live performance" paragraph in the Editor section:
    right-drag a bound number (two-finger drag on iPad) for a momentary
    tweak that does not edit the text; on release it glides back over the
    "momentary glide" setting (default 1 s); hold Shift to snap back.
  - The Vim "Live-coding shortcuts" bullet (around line 280) lists
    `Mod-.` (stop) and `Mod-Shift-.` (cut) instead of "`Mod-.`".
  - English only, no emojis, and match the surrounding style.
- `design-docs/specs/design-live-perf-integration.md`: the status line
  becomes `Implemented (2026-10-18, workflow session 433)`.
  `design-docs/specs/architecture.md`: the 2026-10-18 integration row
  status becomes `Implemented`.

## Invariants

- No product or test file is modified by this plan. Check this with
  `git diff --stat` before and after, recorded in the Progress Log, which
  lists only writePaths.
- The design documents keep all existing content. Only the stated
  appends and status lines change.

## Verification (every command exits 0)

- Gates 1-13 of TASK-C1, with logs under `tmp/live-perf-int/closeout/`.
- `jq . impl-plans/completed/live-perf-dispatch.json` -> exit 0.
- `test ! -e impl-plans/active/live-perf-evidence.md && test -e impl-plans/completed/live-perf-evidence.md`
  -> exit 0. Repeat for each of the eight moved files, in one logged
  command chained with `&&`.
- `grep -c "## 12. Verification record" design-docs/specs/design-live-performance.md`
  -> `1`.
- `! grep -n "active/live-perf-\(contract\|engine\|session\|editor\|evidence\|dispatch\)" impl-plans/README.md`
  -> exit 0. This means no stale `active/` link to an archived live-perf
  file remains. The `active/live-perf-int-` links do not match this
  pattern.

## Completion Criteria

- [ ] Gates 1-13 pass, with positive test counts where they apply.
- [ ] Section 12 is appended with log paths and only established root
  causes.
- [ ] The seven plans and the manifest are completed and archived file by
  file. The archived manifest is valid JSON.
- [ ] `impl-plans/README.md`, `README.md`, the integration design status
  and the architecture row are updated.
- [ ] The Progress Log records pre/post sha256 for every edited file and
  the `git diff --stat` invariant check.

## Execution Rules (all plans in this manifest)

- Read each file fresh immediately before editing it. Record
  `shasum -a 256 <file>` before and after the edit in the Progress Log.
- Edit only writePaths. Report drift in any other file; never repair it.
- Run commands in the foreground only. Write each to its log with
  `cmd > log 2>&1; echo "exit=$?" >> log`.
- Do not commit, stash, branch or run any mutating git command. Read-only
  `git diff`, `git status` and `git show` are allowed.
- Use English only, with no emojis. Keep every touched source file under
  1000 lines.
- Verification record shape is exactly `{command, exitStatus: 0, testsRun,
  testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
  - Negative-control and mutation runs go only in `mutationEvidence`.
  - Repaired review findings go only in `addressedFeedback`, with status
    `repaired`.
- Edit only this plan's own Progress Log.

## Progress Log

### Session: 2026-10-18 (plan authored, workflow session 433 step 4)
**Tasks Completed**: none. Plan authored from the accepted design.
**Notes**: Ready for wave 4 after all other LPI plans.
