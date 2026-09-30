# FST-40: Serial Reconciliation, Full Gate and Closeout

**Status**: Ready
**Plan ID**: FST-40 (wave 4, serial, last)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` sections 7-8
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan finishes the formatter and syntax change:
- reruns any step that an earlier plan recorded as BLOCKED inside the
  sandbox (network, `~/.cache/tree-sitter`, a full nextest run);
- runs the whole section 8 gate on the joined tree;
- documents the new verb and grammar in `README.md`;
- archives the FST plans.

It is the only plan that edits the shared indexes.

## Non-goals

- No feature work. A defect found here is repaired in the file that owns
  it. Record each repair as file, cause, fix, and pre/post sha256. Do
  not weaken or delete any assertion.
- Do not reformat any committed `.vact` file (F5). If
  `fmt-vact-check` reports a file, record it as a finding.
- No change to `design-docs/user-qa/*`. Its recommendations remain the
  defaults.

## Dependencies

- **dependsOn**: FST-10, FST-11, FST-20, FST-21, FST-22, FST-23, FST-30
- **Blocks**: none

## writePaths

- `README.md`: add a "Formatting" section, placed next to the existing
  CLI verbs, covering the following:
  - `vactr fmt [--check] <PATH>...|-`;
  - the exit codes;
  - `mise run fmt-vact-check`.

  Also add a "Syntax (tree-sitter)" section covering the following:
  - the answer to the C/WASM question, in one paragraph pointing to the
    design doc's section 2;
  - `mise run ts-generate`, `mise run ts-test` and
    `mise run ts-build-wasm`;
  - the fact that the editor build picks up `tree-sitter-vact.wasm`
    when it is present and otherwise falls back.

  In the editor build block (around `README.md:172-180`), add
  `mise run ts-build-wasm` before the `npm run build` line.
- `impl-plans/README.md`: update the FST rows to Completed and move
  them to "Completed Plans".
- `impl-plans/completed/`: move the plan files here with `git mv`. The
  orchestrator does this; workers never change git state. The plans
  are `fst-10-fmt-core.md`, `fst-11-ts-grammar.md`, `fst-20-fmt-cli.md`,
  `fst-21-fmt-lsp.md`, `fst-22-fmt-wasm.md`,
  `fst-23-editor-syntax.md`, `fst-30-editor-format.md`,
  `fst-40-closeout.md` and `fst-dispatch.json`.
- `impl-plans/active/fst-*.md`: header Status lines and Progress Logs
  only.
- `design-docs/specs/design-formatter-and-syntax.md`: the Status line
  only ("Implemented <date>, commit <sha>").
- Any file already in an FST plan's writePaths, but ONLY for a
  recorded serial repair.

## sharedPaths (read-only)

Every FST plan file and every file written by FST-10..30.

## Implementation Key Points

1. Before running anything, collect every BLOCKED entry from the
   Progress Logs of FST-10..30. Rerun each one outside the Codex
   sandbox, and record its exit status and log path.
2. If `npm install` was blocked in FST-23, run
   `cd editor && npm install --save-exact web-tree-sitter@0.27.0` here.
   Then confirm that the lockfile and `package.json` pin exactly
   `0.27.0`.
3. Run the gate in the order below. Each command runs in the
   foreground, and its full log goes to
   `tmp/fst/FST-40/<n>-<name>.log`, ending with `exit=<status>`.
4. A failure is repaired serially in its owning file, then the failing
   command and every later command are rerun.
5. Scope check: `git status --short` must list only paths covered by
   an FST plan's writePaths, plus `design-docs/**` and `impl-plans/**`.
   No `.wasm` file, and nothing under `tmp/`, `target/`,
   `editor/dist` or `node_modules`, may appear.

## Verification (the full gate; each must exit 0, with a complete log)

1. `CARGO_TERM_QUIET=true cargo build`
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings`
4. `cargo fmt --check`
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --features lsp`
7. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`,
   then run the FST-22 export listing. It must print
   `fmt_out_len fmt_out_ptr fmt_source session_init`.
8. `mise run ts-generate`, then `mise run ts-test`, then
   `mise run ts-build-wasm`, then `mise run fmt-vact-check`.
9. `cd editor && npm run check`, then `npm test`, then
   `VACTR_REQUIRE_SESSION_ABI=1 npm run build`. After the build,
   `ls dist/vactr.wasm dist/tree-sitter-vact.wasm dist/highlights.scm dist/web-tree-sitter.wasm`
   (use the confirmed runtime name) must list all four files.
10. `mise run lint`
11. Rust file sizes: take the `.rs` files from
    `git diff --name-only HEAD -- '*.rs'` plus
    `git ls-files -o --exclude-standard -- '*.rs'`, run `wc -l` on
    them, and check that every file has fewer than 1000 lines.
12. `git diff --check`
13. `git status --short` passes the scope check above.

## Completion Criteria

- [ ] Every BLOCKED step from FST-10..30 has been rerun, with its exit status and log.
- [ ] Gate commands 1-13 pass, each with a complete log.
- [ ] The README documents `vactr fmt` and the grammar tasks, and the design doc Status line is updated.
- [ ] Every FST plan is marked Completed, the `impl-plans/README.md` rows are updated, and the plans are archived by the orchestrator.
- [ ] Every serial repair and every finding is listed in the Progress Log.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Blockers**: Waits for all FST plans.

## Related Plans

- **Depends On**: FST-10, FST-11, FST-20, FST-21, FST-22, FST-23, FST-30
