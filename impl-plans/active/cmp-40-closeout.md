# CMP-40: Integration, Full Gate and Closeout

**Status**: Ready
**Plan ID**: CMP-40 (wave 4; serial)
**Design Reference**: `design-docs/specs/design-completion.md` 8, 9; `design-docs/specs/design-formatter-and-syntax.md` status note
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This is the serial reconciliation step. It:

- runs every workspace-wide gate over the joined tree;
- reruns any step a worker recorded as BLOCKED;
- repairs cross-plan defects serially;
- updates the user-facing docs and the plan index;
- archives the plans.

The fanout workers scoped their gates to their own files. Only this plan
proves the combined tree.

## Non-goals

- No new features and no design changes. If the gate reveals a design
  defect, record it and stop, rather than redesigning.
- No reformatting of the committed `.vact` files.
- No git commit or push. The orchestrator commits, and pushes only
  `wf/syntax-fmt`.

## Dependencies

- **dependsOn**: CMP-10, CMP-15, FST-50, EDS-10, EDS-11, EDS-12, CMP-20,
  CMP-21, CMP-30, CMP-31, CMP-32
- **Blocks**: none

## writePaths

- `README.md`: the LSP completion, editor completion (popup keys) and
  `vactr fmt` space-indent repair bullets, near the existing lines
  124-125 and 163.
- `impl-plans/README.md`: the rows for these plans.
- `design-docs/specs/design-completion.md`: the Status line only.
- `design-docs/specs/design-formatter-and-syntax.md`: the session-226
  status sentence only.
- `impl-plans/active/cmp-dispatch.json`: the status notes.
- the plan files `impl-plans/active/cmp-10-complete-core.md`,
  `cmp-15-completion-types.md`, `fst-50-fmt-space-repair.md`,
  `eds-10-syntax-span-core.md`, `eds-11-wasm-format-loader.md`,
  `eds-12-format-core-tool-wasm.md`, `cmp-20-complete-lsp.md`,
  `cmp-21-complete-wasm.md`, `cmp-30-completion-service.md`,
  `cmp-31-completion-popup.md`, `cmp-32-completion-view-wiring.md` and
  this file: status, and a move to `impl-plans/completed/` when
  everything is green.
- Any file of a completed plan, only for a serial repair of a gate
  failure. Record the file, cause, fix and pre/post sha256.

## sharedPaths (read only)

Everything else.

## Tasks

1. Re-read every plan's Progress Log. List the BLOCKED steps and rerun
   them outside the Codex sandbox (full nextest, the release budget test,
   `mise run ts-build-wasm`).
2. Run the full gate below in order. On a failure, locate the owning
   plan, repair serially, and rerun the failing command, then the whole
   gate.
3. Check the file sizes: `wc -l` on every `.rs` file touched by this run
   is under 1000.
4. Check the fanout constraints:
   - `git status --porcelain=v1 -uall` shows only files inside the union
     of the plans' writePaths;
   - no `@codemirror/autocomplete` in `editor/package.json`;
   - `git diff --stat -- editor/src/app/apis.ts editor/src/code/language.ts`
     is empty.
5. Update the docs: the README bullets, the plan index rows, and the two
   design status lines ("Implemented and verified 2026-09-30" plus the
   test counts).
6. Archive: move the plan files to `impl-plans/completed/` with
   `git mv`-free plain moves (the orchestrator commits), and update
   their links in `impl-plans/README.md`.

## Verification (full gate; each exits 0 with a complete log in `tmp/cmp/CMP-40/attempt-<n>/`)

1. `CARGO_TERM_QUIET=true cargo build`
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings`
4. `cargo fmt --check`
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`
   (record the pass count; it must exceed 1647)
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --features lsp`
   (record the pass count; it must exceed 1663)
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --release --run-ignored only -E 'test(budget)'`
   (record the median, which must be under 5 ms)
8. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`,
   then the CMP-21 export listing command, which prints
   `complete_out_len complete_out_ptr complete_source fmt_out_len fmt_out_ptr fmt_source session_init`
9. `mise run ts-build-wasm`, then `mise run ts-test`
10. `mise run fmt-vact-check`
11. `mise run lint`
12. `cd editor && npm run check && npm test && npm run build`
    (record the vitest pass count; it must exceed 374)

## Completion Criteria

- [ ] Gate steps 1-12 exit 0, each with a complete log, and the counts
      and the budget median are recorded.
- [ ] No BLOCKED step remains unresolved, or each remaining one is
      reported explicitly with its evidence.
- [ ] The file size, path-scope, no-autocomplete and untouched
      `apis.ts`/`language.ts` checks pass.
- [ ] The README, plan index and design status lines are updated, and the
      plans are archived.
- [ ] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: Waits for every other plan.

## Related Plans

- **Depends On**: all CMP, FST-50 and EDS plans
