# LP-EVIDENCE: Silent behavior e2e, full gates, verification record, closeout

**Status**: Ready (after all wave-2 plans)
**Plan ID**: LP-EVIDENCE (wave 3)
**Design Reference**: `design-docs/specs/design-live-performance.md` 8.3, 8.4, 9, 11
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-09 (session 343: section 12 title; accepted wave-2 dependencies)

## Intent and Context

The wave-2 plans add the engine, session and editor behavior, each with
focused tests. This plan proves the features work together in real
browsers, silently:

- a right-drag on a bound number shows a live value without changing the
  text, and the context menu stays hidden on the site;
- a two-finger touch tweak works;
- `Mod-.` drains and then idles;
- `Mod-Shift-.` cuts and then idles.

It then runs every intake gate serially (heavy ones under the measurement
lock), records the results in the design document, and archives the plans.

## Non-goals

- No product behavior change. A defect found here is reported to the
  workflow review for selective redispatch to the owning plan. The only
  exception is the perf-hook accessors listed below.
- No change to any threshold, workload or existing e2e check, and no
  `--write-evidence` run. The canvas `run-001` evidence stays untouched.
- No commit or push from inside the Codex sandbox (`.git` is read-only
  there). The workflow's verification and integration step commits and
  pushes non-force to `origin wf/live-perf` outside the sandbox.

## Dependencies

- **dependsOn**: LP-ENGINE, LP-SESSION-STOP, LP-SESSION-MOMENTARY,
  LP-EDITOR-STOP, LP-EDITOR-MOMENTARY.
- Session 343: integration review has already accepted LP-ENGINE,
  LP-SESSION-MOMENTARY, LP-EDITOR-STOP and LP-EDITOR-MOMENTARY (manifest
  `acceptedDependencies`, commit ab377d6). The only remaining dispatched
  dependency is LP-SESSION-STOP (TASK-S5).
- **Blocks**: none.

## writePaths

- `editor/src/code/perf-hook.ts`
- `editor/src/code/mount.ts` (passes the new perf-hook accessors only)
- `editor/test/e2e/behavior.mjs`
- `editor/test/canvas/mount.test.ts` (one perf-hook row only)
- `design-docs/specs/design-live-performance.md` (appends section 12 only)
- `impl-plans/README.md`
- `impl-plans/active/live-perf-contract.md`
- `impl-plans/active/live-perf-engine.md`
- `impl-plans/active/live-perf-session-stop.md`
- `impl-plans/active/live-perf-session-momentary.md`
- `impl-plans/active/live-perf-editor-stop.md`
- `impl-plans/active/live-perf-editor-momentary.md`
- `impl-plans/active/live-perf-evidence.md`
- `impl-plans/active/live-perf-dispatch.json`
- `impl-plans/completed/live-perf-contract.md`
- `impl-plans/completed/live-perf-engine.md`
- `impl-plans/completed/live-perf-session-stop.md`
- `impl-plans/completed/live-perf-session-momentary.md`
- `impl-plans/completed/live-perf-editor-stop.md`
- `impl-plans/completed/live-perf-editor-momentary.md`
- `impl-plans/completed/live-perf-evidence.md`
- `impl-plans/completed/live-perf-dispatch.json`
- Artifact roots, also listed in the manifest's `artifactRoots`:
  - `target`
  - `tree-sitter-vact/tree-sitter-vact.wasm`
  - `editor/dist`
  - `editor/node_modules/.vite`
  - `editor/src-tauri/target`
  - `tmp/live-perf/e2e`
  - `tmp/live-perf/gates`

## sharedPaths

None. Read-only:

- `editor/test/e2e/run.mjs`, `editor/test/e2e/silent-sink.mjs`
- `editor/src/code/transport.ts` (`outputHistory`)
- `editor/src/app/apis.ts` (`momentaryHit`)
- `editor/test/style/ui-style.mjs`

## Tasks

### TASK-V1: Perf-hook accessors (`editor/src/code/perf-hook.ts`, `editor/src/code/mount.ts`)

These are active only with `?perf=1`, like every existing accessor.
`window.__vactrPerf` gains:

- `transportOutput(): string`: the transport bar's current output value.
- `outputHistory(): string[]`: from `TransportBar.outputHistory()`.
- `momentaryLabels(): string[]`: the labels of the `momentary` animated
  rows rendered in the last presented frame. mount.ts records them where it
  appends the animated rows.
- `momentaryHit(pos: number): boolean`: from `surface.momentaryHit`.
- `coordsAtPos(pos: number): {left, right, top, bottom} | null`: from
  `surface.coordsAtPos`.

They are inert without `?perf=1`. Add one row to the existing perf-hook
vitest in `editor/test/canvas/mount.test.ts` (it uses `installPerfHook`),
proving the accessors exist and return plain data.

### TASK-V2: Behavior checks (`editor/test/e2e/behavior.mjs`)

Add four `check(...)` blocks. They run after the existing checks, in both
Chromium and WebKit, through the existing silent sink.

**Fixture.** Fill the bridge textarea with:

```
d1 s :analog > note [:a4 :c5] > lpf 800 > room 0.6 > gain 0.1
```

Click `.vact-run`. Then poll until `momentaryHit(<offset of "800">+1)` is
true, with a 10 s timeout.

**`momentary-right-drag`:**

1. Record the doc with `__vactrPerf.doc()`.
2. Install a window capture `contextmenu` probe that records
   `defaultPrevented` after dispatch (`setTimeout(0)`).
3. Center the mouse on the literal using `coordsAtPos`.
4. `page.mouse.down({ button: 'right' })`, then move 80 px up in 8 steps.
5. During the hold, assert:
   - `momentaryLabels()` has one label whose number is not 800;
   - the doc is unchanged;
   - every recorded `contextmenu` was prevented.
6. Release with `page.keyboard.down('Shift')`, then
   `mouse.up({ button: 'right' })`, then `keyboard.up('Shift')`.
7. Within 10 frames `momentaryLabels()` is empty.
8. Repeat without Shift: the labels are still present 100 ms after release
   (gliding with the default 1000 ms) and empty by 1500 ms.
9. A right-click on a non-site position records a `contextmenu` that is
   **not** prevented.

**`momentary-two-finger`:**

- Dispatch synthetic `pointerdown` events with `pointerType: 'touch'`,
  ids 21 and 22, 60 ms apart, on the literal. Reuse the existing WebKit
  touch-event helper pattern in behavior.mjs.
- Move both 60 px up: labels appear and the doc is unchanged.
- `pointerup` on id 22: the labels glide away.
- Record a limitation note that this is synthetic multi-touch, and that a
  physical iPad check is pending.

**`stop-gentle-shortcut`:**

1. Make sure audio is running: the eval starts it, and the silent sink is
   installed.
2. Wait until `transportOutput()` is `running`, or the history is empty.
3. Press `Meta+.` (macOS host; `navigator.platform` is `MacIntel`).
4. Within 20 s, `outputHistory()` contains `draining`, followed later by
   `idle`.

**`cut-shortcut`:**

1. Click `.vact-run` again and wait 500 ms.
2. Press `Meta+Shift+.`.
3. Within 3 s, `transportOutput()` is `idle`, and no `draining` entry was
   added after the keypress.

**Bounds.** All waits are bounded. Failures throw with a JSON detail, the
same way the existing checks do. The silent-sink post-sink peak assertions
stay global and unchanged.

### TASK-V3: Serial gates

Run each gate alone, in this order. Log each to
`tmp/live-perf/gates/<n>-<name>.log`.

Gates marked LOCK run inside the measurement lock:

```
until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done
echo live-perf-evidence > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner
trap 'rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock' EXIT
```

Never hold the lock while idle.

1. `CARGO_TERM_QUIET=true cargo build --all-targets`
2. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
3. `rustfmt --edition 2021 --check` on every `.rs` file changed since the
   plan checkpoint (`git diff --name-only <checkpoint>.. -- '*.rs'`)
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   (debug wasm)
5. `mise run build-wasm-release`
6. `cd editor/src-tauri && CARGO_TERM_QUIET=true cargo check`
7. `cd editor && npm run check`
8. `cd editor && ./node_modules/.bin/vitest run` (full default suite;
   testsRun > 0, failures 0)
9. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
10. `cd editor && npm run test:perf` (alone)
11. LOCK: `cd editor && npm run test:style`
12. LOCK: `cd editor && npm run e2e -- --browser all --profile behavior --run-id live-perf-001 --out ../tmp/live-perf/e2e/live-perf-001`
    - exit 0;
    - every behavior check passes in Chromium and WebKit, including the
      four new ones;
    - post-sink peak 0.
13. LOCK: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --no-fail-fast`
    - command timeout of at least 2400 s;
    - run alone;
    - testsRun > 0 and failures 0.

**If a gate fails:** do not loosen anything. Record the failing command,
its log path and the owning plan in the Progress Log, and stop. The
workflow review redispatches the owning plan. Report only final passing
runs in the verification records.

### TASK-V4: Verification record (`design-docs/specs/design-live-performance.md`)

Append the section **"12. Verification record (session 343)"**. It holds:

- a table with each gate command, its exit status, tests run and passed,
  and its log path;
- the four behavior-check results per browser;
- the e2e output path;
- the stated limitations: synthetic multi-touch only, and the physical iPad
  two-finger check is pending.

Change nothing else in the design.

### TASK-V5: Closeout

- Set each plan's status to Completed, with a final Progress Log entry.
- Move the six plans, this plan and `live-perf-dispatch.json` from
  `impl-plans/active/` to `impl-plans/completed/`, one file at a time, at
  the concrete paths in writePaths.
- Update `impl-plans/README.md`: the "Live performance controls" section
  becomes Completed and links to the completed paths.
- The workflow's integration step commits and non-force pushes to
  `origin wf/live-perf` outside the sandbox.

## Key Points a Careless Implementation Gets Wrong

- Every browser run uses the existing silent sink. Never click
  `.vact-audio` without it, and never change the macOS volume.
- Do not run two heavy suites at once. Runs 10, 11, 12 and 13 are
  strictly sequential, and 11-13 run under the lock.
- `--profile behavior` skips the measurement profile. Do not add
  `--write-evidence`.
- Playwright's key name is `'.'`, so the presses are `'Meta+.'` and
  `'Meta+Shift+.'`. The handler matches `code === 'Period'`.
- Do not edit product files beyond perf-hook.ts and mount.ts.

## Verification (exact commands; all must exit 0)

- The gates of TASK-V3 (1-13).
- `node --check editor/test/e2e/behavior.mjs`
- `cd editor && ./node_modules/.bin/vitest run test/e2e`
- `jq . impl-plans/completed/live-perf-dispatch.json` (valid JSON after the
  move)

## Completion Criteria

- [ ] The four behavior checks pass in Chromium and WebKit, and post-sink
  peak is 0.
- [ ] Gates 1-13 exit 0 with positive test counts where they apply.
- [ ] Section 12 is appended to the design doc with log paths.
- [ ] The plans and the manifest are archived file by file, and the README
  is updated.
- [ ] The Progress Log is updated.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.

### Session: 2026-10-09 (session 343 plan step)
**Tasks Completed**: none. Plan text updated only.
**Notes**:
- The section 12 title now reads "session 343".
- The dependency note now records that the other wave-2 plans are
  accepted.
- This plan runs after LP-SESSION-STOP TASK-S5 passes test-integrity,
  adversarial and integration review.
- Gate 3's rustfmt diff set is taken from 04dc4f2, so it also covers
  `src/sched/runtime/song/clock_tests.rs`.
