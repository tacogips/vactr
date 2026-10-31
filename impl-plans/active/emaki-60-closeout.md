# EMK-CLOSEOUT: full gates G1-G10, packaging absent-case proof, docs, mise tasks and index

**Status**: Ready (after all other emaki plans)
**Plan ID**: EMK-CLOSEOUT (wave 6; runs alone)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 11.3 (proof), 15 (G1-G10), 16 (W5); decisions EM-D3, EM-D14, EM-D15
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

This plan proves the whole request on the combined tree:

- emaki works standalone (check, test, build, perf, e2e, smoke);
- vactr consumes it with no manual step, including a clean `npm ci` with and
  without `emaki/node_modules` (G5);
- every existing vactr suite passes: vitest, style, e2e in both browsers,
  examples and Tauri;
- the canvas budgets did not regress (G9, with the repository gates from
  B11 / EM-D14);
- the vitest count reconciles against the 2384 baseline (EM-D15).

It then records the evidence in the design, adds the mise tasks and README
mentions, and indexes the plans.

## Non-goals

- No product or test change. If a gate fails, stop: record the failure,
  classify the owner plan, and set Status to `Blocked`. The review step
  routes the repair.
- No archiving of plans; the workflow's `plan-archive` node does that.
- No change to `run-001` evidence.

## Dependencies

- **dependsOn**: EMK-CORE, EMK-DECOR, EMK-WIDGETS, EMK-LANG, EMK-DIFF,
  EMK-API, EMK-VAC-SWITCH, EMK-DIFF-DEMO, EMK-JSON-DEMO, EMK-SHOWCASE,
  EMK-VAC-PLUGINS, EMK-PERF.

## writePaths

Docs and tasks:
- `design-docs/specs/design-emaki.md`
- `design-docs/specs/architecture.md`: the emaki row status cell only
- `README.md`, `mise.toml`, `impl-plans/README.md`

Evidence written by G9:
- `design-docs/specs/evidence/canvas-cutover/emaki-001`

Generated:
- `emaki/node_modules`, `emaki/dist`, `editor/node_modules`,
  `editor/dist`, `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/src-tauri/target` (artifactRoots)

This plan's own file:
- `impl-plans/active/emaki-60-closeout.md`

## sharedPaths

None.

## Tasks

### C1: Gates

Run serially, each under the lock where heavy. Logs go to
`tmp/emaki/EMK-CLOSEOUT/<gate>.log`. Every record follows rule 10.

1. **G1.** `export CARGO_TERM_QUIET=true; cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
2. **G2.** `cd emaki && rm -rf node_modules dist && npm ci --prefer-offline && npm run check && npm test && npm run build`.
3. **G5 (absent case).**
   1. `rm -rf emaki/node_modules`.
   2. `cd editor && rm -rf node_modules && npm ci --prefer-offline && npm run check && ./node_modules/.bin/vitest run test/code/emaki-single-instance.test.ts`
      (exit 0).
   3. Record `ls emaki/node_modules` (absent, or what `npm ci` created) and
      `ls -la editor/node_modules/emaki` (a symlink to `../../emaki`).
   4. `cd emaki && npm ci --prefer-offline`.
   5. Rerun step 2 for the present case.
4. **G6.** `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run`.
   Then reconcile:
   - write `tmp/emaki/EMK-CLOSEOUT/test-count.json` with `{ vactr, emaki, baseline: 2384, movedFiles, newTests }`;
   - gate: `vactr + emaki >= 2384 + newTests`, and the moved-file counts are
     equal to EMK-CORE's `test-disposition.json`.
5. **G7.** `mise run build-wasm-release`, then
   `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`, then
   `cd editor && npm run test:style` (16/16).
6. **G8.**
   - `cd editor && npm run e2e -- --browser all --profile behavior --out ../tmp/emaki/EMK-CLOSEOUT/e2e`
     (failureCount 0, both browsers);
   - `cd editor && npm run e2e:examples` (8/8).
7. **G9.** Follow the quiet-host lock procedure in `editor/test/e2e/README.md`
   lines 40-52, then run
   `cd editor && npm run e2e -- --browser all --profile measure --write-evidence --run-id emaki-001`
   with the release-wasm build from step 5.

   Compare the summary to the B11 thresholds:

   | Metric | Gate |
   |--------|------|
   | Input p95 | <= 50 |
   | Text-dirty frame work p95 | <= 8 (record against the 4 ms target and the run-001 baselines 4.3 / 3) |
   | Animation frame work p95 | <= 8 |
   | Frame interval p95 | <= 20 |

   Both browsers must pass.
8. **G3 and G4 rerun on the final tree.**
   - `cd emaki && npm run build && npm run perf -- --browser all --out ../tmp/emaki/EMK-CLOSEOUT/perf`
     (`allPass`);
   - `cd emaki && npm run e2e -- --browser all --out ../tmp/emaki/EMK-CLOSEOUT/e2e-emaki`;
   - `cd emaki && node scripts/smoke.mjs --browser all --out ../tmp/emaki/EMK-CLOSEOUT/smoke`,
     with no `skipped` entries allowed.
9. **G10.** `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml`.
   Expect exit 0, failures 0, and testsRun equal to the baseline of 17. That
   baseline was recorded by the live-perf-int closeout
   (`impl-plans/completed/live-perf-int-closeout.md`). This run changes no
   Rust or Tauri file; confirm with
   `git diff --stat a6e5bea -- editor/src-tauri src Cargo.toml` -> empty.
   If the count differs, record both numbers and set Status to `Blocked`.
10. **Boundary checks.**
    - `cd emaki && ./node_modules/.bin/vitest run test/boundary.test.ts`;
    - `cd editor && ./node_modules/.bin/vitest run test/code/emaki-boundary.test.ts`;
    - `grep -rn "vactr" emaki --include=*.ts --include=*.mjs --include=*.html -l`
      must print nothing (exclude `node_modules` and `dist`).
11. **Line counts.** `wc -l` over all new and touched source files under
    `emaki/src`, `emaki/examples`, `emaki/scripts` and `editor/src/code`:
    each under 1000.

### C2: Docs and tasks (only after C1 passes)

- **`design-emaki.md`**:
  - change the Status line to `Implemented (2026-10-31, workflow session 586)`;
  - append exactly one section, `## 19. Verification record (workflow session 586)`,
    with a gate table (command, exit, counts, log path), the perf raw-number
    table per browser, the G9 metric table, the test-count reconciliation,
    and the G5 install-state evidence;
  - add residual notes: physical iPad not validated; EQ1-EQ6 defaults
    applied;
  - record a divergence from design 8.4: PageUp and PageDown are not
    handled by `KeyboardController`, today or after this run (EMK-WIDGETS
    non-goal), so the 8.4 PageUp/Down snapping sentence describes browser
    scrolling only;
  - cite the wave-join logs `tmp/emaki/wave-<n>-join/`.
- **`architecture.md`**: change the emaki row status cell from `Proposed`
  to `Implemented`. No other edit.
- **`mise.toml`**: append tasks in the existing format:
  - `[tasks.emaki-check]` `run = "cd emaki && npm run check"`
  - `[tasks.emaki-test]` `run = "cd emaki && npm test"`
  - `[tasks.emaki-build]` `run = "cd emaki && npm run build"`
  - `[tasks.emaki-perf]` `run = "cd emaki && npm run perf"`
  - `[tasks.emaki-e2e]` `run = "cd emaki && npm run e2e"`

  Each gets a one-line description. Then `mise tasks ls | grep emaki` must
  show 5.
- **`README.md`**: in `## Editor`, add a short paragraph:
  - the code editor engine is the emaki library at `emaki/`, consumed via
    `file:../emaki` (later `github:tacogips/emaki#<sha>`);
  - inline sequencer and oscilloscope widgets, with the "Inline widgets"
    toggle and `vactr.inlineWidgets`;
  - beat glow.

  In `## Development`, add the five mise tasks.
- **`impl-plans/README.md`**: in the existing section
  `## emaki editor library (2026-10-31)`, which step 4 already created,
  replace the `Status: Ready...` sentence with
  "Status: gates passed; awaiting final integration review", followed by the
  key counts (vitest vactr/emaki, e2e, perf allPass). Keep the links
  unchanged.

## Invariants

- No file outside writePaths changes. Record `git status --porcelain`
  before and after C2.
- Gates are never relaxed. A timing-sensitive rerun is recorded as a
  separate record, not as a replacement.

## Verification

- C1 gates 1-11, each with a record.
- `mise tasks ls | grep -c emaki-` -> 5.
- `grep -c '## 19. Verification record (workflow session 586)' design-docs/specs/design-emaki.md` -> 1.

## Completion Criteria

- [ ] Every C1 gate passes in the record shape. G3, G8 and G9 pass in both
  browsers. The G6 reconciliation holds.
- [ ] C2 docs, tasks and index are done.
- [ ] Status is `Implemented; awaiting review`, or `Blocked` with a
  classified owner when a gate failed.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
