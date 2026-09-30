# CMP-40: Integration Review, Full Gate and Closeout

**Status**: Ready
**Plan ID**: CMP-40 (wave 4; serial; the only dispatched plan of run session 234)
**Design Reference**: `design-docs/specs/design-completion.md` 3-9 (section 8 "Closeout run (session 234, no design change)"); `design-docs/specs/design-formatter-and-syntax.md` 3.9, 5.4, 5.5, 8
**Dispatch**: `impl-plans/active/cmp-closeout-dispatch.json` (`plans` = [CMP-40]; the 11 accepted plans are in `acceptedDependencies`)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30 (session 234: closeout manifest, concrete-file path scope)

## Intent and Context

The user asked (2026-09-30) for completion suggestions that appear while
typing, plus three follow-ups: a CodeMirror-free tree-sitter span core, a
formatter space-indent repair, and the `VACTR_WASM` loader fix of the
wasm format test. Run session 226 committed the design and plans at
`a61fc2e`. It then implemented and accepted CMP-10, CMP-15, CMP-20,
CMP-21, CMP-30, CMP-31, CMP-32, EDS-10, EDS-11, EDS-12 and FST-50. That
work is still uncommitted: 56 source, test and fixture files (the
sharedPaths below) plus the 12 plan files.

The first CMP-40 dispatch failed with "fanout snapshot exceeds 512
entries" (tacogips/riela#128) because the CMP-40 entry of
`impl-plans/active/cmp-dispatch.json` listed directories. That entry is
NOT fixed and NOT used again. Session 234 dispatches this plan from the
new manifest `impl-plans/active/cmp-closeout-dispatch.json`, which lists
concrete files only: 30 writePaths and 56 sharedPaths, 86 entries.

This plan is the serial closeout. It:

1. checks the path scope;
2. reviews every change since `a61fc2e` together against the two design
   docs;
3. repairs any material finding in place, with a minimal edit;
4. runs the full gate;
5. updates the docs and the plan index;
6. archives the plans.

The orchestrator commits and pushes afterwards.

## Non-goals

- No redesign, no new features and no re-implementation. The working
  tree is the accepted implementation.
- No edits to `editor/src/app/apis.ts` or `editor/src/code/language.ts`.
  No `@codemirror/autocomplete`, no `npm install`, and no change to
  `package.json`, `package-lock.json` or `Cargo.toml`/`Cargo.lock`.
- No reformatting of committed `.vact` files. No `cargo fmt` without
  `--check`; rustfmt is allowed only on a single repaired `.rs` file.
- No edits to `impl-plans/active/cmp-dispatch.json` (it is moved
  unchanged) or to `impl-plans/active/cmp-closeout-dispatch.json` (it
  stays in `impl-plans/active/`, like `fst-dispatch.json`).
- No git commit, stash, checkout, reset, branch, merge or push. The
  orchestrator commits and pushes only `wf/syntax-fmt`.
- `impl-plans/active/fst-dispatch.json`, the `mod004-*` and `modular-*`
  plans and any other file not named here are left untouched.

## Invariants

- Every file in the sharedPaths list keeps its content unless Task 2
  records a material finding against it.
- `git diff -- editor/src/app/apis.ts editor/src/code/language.ts` stays
  empty.
- Every `.rs` file stays under 1000 lines.
- The wasm module keeps exactly the pinned exports listed in gate
  step 8.

## Dependencies

- **dependsOn**: none in the dispatch. CMP-10, CMP-15, FST-50, EDS-10,
  EDS-11, EDS-12, CMP-20, CMP-21, CMP-30, CMP-31 and CMP-32 were all
  accepted in session 226 and are listed as `acceptedDependencies` in
  the closeout manifest.
- **Blocks**: none

## writePaths (30 concrete files)

Docs:

- `README.md`
- `impl-plans/README.md`
- `design-docs/specs/design-completion.md`: the Status sentence in the
  header (currently "Status: Design, 2026-09-30, branch") only.
- `design-docs/specs/design-formatter-and-syntax.md`: the sentence
  "Follow-up amendment (2026-09-30, session 226, status: Design)" only.

The 13 archive sources in `impl-plans/active/`:

- `impl-plans/active/cmp-10-complete-core.md`
- `impl-plans/active/cmp-15-completion-types.md`
- `impl-plans/active/cmp-20-complete-lsp.md`
- `impl-plans/active/cmp-21-complete-wasm.md`
- `impl-plans/active/cmp-30-completion-service.md`
- `impl-plans/active/cmp-31-completion-popup.md`
- `impl-plans/active/cmp-32-completion-view-wiring.md`
- `impl-plans/active/cmp-40-closeout.md`
- `impl-plans/active/eds-10-syntax-span-core.md`
- `impl-plans/active/eds-11-wasm-format-loader.md`
- `impl-plans/active/eds-12-format-core-tool-wasm.md`
- `impl-plans/active/fst-50-fmt-space-repair.md`
- `impl-plans/active/cmp-dispatch.json`

The 13 archive destinations: the same basenames under
`impl-plans/completed/`.

## sharedPaths (56 concrete files: the source changed since a61fc2e)

These are read for the integration review. Only a material finding
(Task 2) may edit one of them, with a minimal edit.

- Rust, modified (10): `src/lib.rs`, `src/cli/tests/fmt.rs`,
  `src/fmt/mod.rs`, `src/fmt/tests/mod.rs`, `src/fmt/tests/mutation.rs`,
  `src/host/wasm/mod.rs`, `src/lsp/analysis.rs`, `src/lsp/server.rs`,
  `src/lsp/tests/analysis.rs`, `src/lsp/tests/mod.rs`
- Rust, new (14): `src/complete/mod.rs`, `src/complete/context.rs`,
  `src/complete/rank.rs`, `src/complete/scope.rs`,
  `src/complete/sources.rs`, `src/complete/tests/mod.rs`,
  `src/complete/tests/context.rs`, `src/complete/tests/rank.rs`,
  `src/complete/tests/robust.rs`, `src/complete/tests/scope.rs`,
  `src/fmt/repair.rs`, `src/fmt/tests/repair.rs`,
  `src/host/wasm/complete_abi.rs`, `src/lsp/tests/complete.rs`
- Fixtures, new (10): `src/fmt/tests/fixtures/` + `space2`, `space4`,
  `space-ambiguous`, `space-mixed-lines`, `space-other-error`, each with
  `.in` and `.out`
- Editor, modified (7): `editor/src/app/deps.ts`, `editor/src/app/main.ts`,
  `editor/src/code/format.ts`, `editor/src/code/mount.ts`,
  `editor/src/code/syntax.ts`, `editor/test/code/format.test.ts`,
  `editor/test/wasm/format.test.ts`
- Editor, new (15): `editor/src/code/completion.ts`,
  `editor/src/code/completion-types.ts`,
  `editor/src/code/completion-popup.ts`,
  `editor/src/code/completion-view.ts`, `editor/src/code/format-core.ts`,
  `editor/src/code/syntax-core.ts`, `editor/src/code/tool-wasm.ts`,
  `editor/test/code/completion.test.ts`,
  `editor/test/code/completion-types.test.ts`,
  `editor/test/code/completion-popup.test.ts`,
  `editor/test/code/completion-view.test.ts`,
  `editor/test/code/syntax-core.test.ts`,
  `editor/test/code/tool-wasm.test.ts`,
  `editor/test/wasm/complete.test.ts`,
  `editor/test/wasm/completion-engine.test.ts`

`cmp-closeout-dispatch.json` lists every one of these files in full.
`editor/src/code/language.ts` and `editor/src/app/apis.ts` are read by
the grep checks only and are never edited.

## Edit protocol

- Before each edit, re-read the file. Record its `shasum -a 256` and a
  one-line intent in this plan's Progress Log. Record the post-edit hash
  after the edit.
- If a file drifted from its last recorded hash, stop and report.
- Edit only writePaths, plus a sharedPath under Task 2.
- Evidence: `tmp/cmp-closeout/CMP-40/attempt-<n>/NN-<name>.log`, one full log per
  command, each ending with `exit=<status>`. `tmp/` is gitignored and
  never committed. A missing or truncated log is not a pass.
- A command the Codex sandbox cannot run (full nextest, localhost
  binding, `~/.cache/tree-sitter`, network, anything that writes `.git`)
  is recorded as `BLOCKED` with the command, exit code and stderr. The
  orchestrator reruns it outside the sandbox before commit. Never report
  it as passed.

## Tasks

### Task 1: path-scope baseline (no edits)

Use read-only git commands only (`.git` is read-only in the sandbox; do
not use `git status`, which may try to refresh the index).

- `git diff --name-only a61fc2e` must list exactly 30 paths: the 17
  modified sharedPaths (10 Rust, 7 editor), the 12 plan `.md` files in
  the archive list, and `design-docs/specs/design-completion.md`
  (the session-234 section 8 closeout paragraph).
- `git ls-files --others --exclude-standard` must list exactly 40 paths:
  the 39 new sharedPaths (14 Rust, 10 fixtures, 15 editor) and
  `impl-plans/active/cmp-closeout-dispatch.json`.
- Any extra or missing path is a finding. Do not delete or revert
  anything.
- `git diff --stat -- editor/src/app/apis.ts editor/src/code/language.ts`
  must be empty.
- `grep -rn "@codemirror/autocomplete" editor/package.json editor/src`
  must have no match.
- `wc -l` on every `.rs` sharedPath: each must be under 1000.

### Task 2: combined integration review (repair only material findings)

Check each item and record `OK` or `FINDING <file:line> <cause>` in the
Progress Log.

1. **LSP superset** (design-completion 4.2): `src/lsp/analysis.rs`
   delegates completion to `crate::complete`. It still offers prelude
   natives, document top-level names, manifest keywords, package
   prefixes and qualified names. `src/lsp/tests/complete.rs` asserts the
   superset rule over a table. A careless reading takes a missing source
   for a ranking change. Only a candidate that the old `starts_with`
   filter returned and the engine drops is a finding.
2. **wasm ABI vs service** (design-completion 5 vs 6.1): the export names
   are pinned (`complete_source`, `complete_out_ptr`, `complete_out_len`).
   The JSON field names and kind strings emitted by
   `src/host/wasm/complete_abi.rs` must match the parser in
   `editor/src/code/completion.ts` and the types in `completion-types.ts`.
   Ranges are UTF-8 bytes on the wire and UTF-16 after the service
   mapping. `editor/test/wasm/completion-engine.test.ts` is the
   end-to-end proof.
3. **Span classes** (design-formatter-and-syntax 5.5): every class that
   `editor/src/code/syntax-core.ts` can emit must be a `vact-tok-*` class
   that `editor/src/code/language.ts` defines. Compare the two with grep.
   Do not edit `language.ts`: a mismatch is repaired in `syntax-core.ts`.
4. **CodeMirror-free cores**: `syntax-core.ts`, `format-core.ts`,
   `tool-wasm.ts`, `completion.ts`, `completion-types.ts` and
   `completion-popup.ts` have no `@codemirror/view` import. Only
   `completion-view.ts`, `syntax.ts`, `format.ts` and `mount.ts` may
   import it.
5. **FST-50 repair** (design-formatter-and-syntax 3.9): the
   `space-ambiguous`, `space-mixed-lines` and `space-other-error`
   fixtures are refused and returned byte-identical. `space2` and
   `space4` convert, are idempotent, and are semantically equal to the
   tab version. All of this is covered by `src/fmt/tests/repair.rs`, and
   the mutation test in `src/fmt/tests/mutation.rs` accepts the
   repaired-or-refused rule.
6. **wasm32-clean**: `src/complete` compiles under the host-wasm wasm32
   build (gate step 8).

**Material** means a gate failure, a design rule violated, or a
cross-plan contract mismatch. Style preferences are not material; record
them as notes only.

**Repair**: make the smallest edit in the owning file, then record the
file, cause, fix, and pre/post sha256. Re-check the file's line count if
it is `.rs`. Rerun the failing command, then the whole gate. If a
finding needs a design change, record it and stop without redesigning.

### Task 3: full gate

Run the commands under Verification in order. Record each exit status,
log path and count.

### Task 4: docs (only after the gate is green, or all remaining BLOCKED steps are listed)

- `README.md`:
  - In the `src/lsp/` bullet (around lines 122-124), add that
    `textDocument/completion` uses the engine in `src/complete/`.
  - After the `src/fmt/` bullet (lines 125-126), add a
    `src/complete/` bullet: the context-aware, scope-aware, ranked
    completion engine, wasm32-clean, shared by the LSP and the editor
    through the `complete_source` export.
  - In the editor feature bullet (around lines 161-164), add the
    completion popup. It appears while typing; `Ctrl-Space` opens it
    manually, Arrow/PageUp/PageDown move, `Enter`/`Tab` accept and
    `Escape` closes (design-completion 6.3).
  - In "Formatting" (line 204 onward), add a sentence: a file whose
    only reader errors are space indentation (`indent-space`) is
    repaired when a single indent unit divides every leading-space width
    and the re-read is clean. Mixed, ambiguous or otherwise erroneous
    files are still left unchanged with exit 3.
  - In "Syntax (tree-sitter)" (line 213 onward), add a sentence:
    tree-sitter spans come from the CodeMirror-free
    `editor/src/code/syntax-core.ts` (`{from, to, class}` with the
    `vact-tok-*` classes), so a non-EditorView renderer can use them.
- `design-docs/specs/design-completion.md` header: change "Status:
  Design, 2026-09-30" to "Status: Implemented and verified 2026-09-30
  (session 234)", with the nextest counts (plain and `--features lsp`)
  and the vitest count. Name `impl-plans/completed/` as the plan
  location. Make no other edit.
- `design-docs/specs/design-formatter-and-syntax.md` amendment sentence:
  change "status: Design" to "status: Implemented and verified
  2026-09-30 (session 234); FST-50, EDS-10..12 are in
  `impl-plans/completed/`". Make no other edit.
- `impl-plans/README.md`:
  - Remove the 13 active rows (currently lines 42-54: `cmp-dispatch.json`
    and the cmp/eds/fst-50 plans).
  - Add one active row for `cmp-closeout-dispatch.json`, in the format
    of the `fst-dispatch.json` row: "Dispatch manifest for the
    session-234 closeout (plans [CMP-40]; 11 accepted dependencies;
    all 12 plans completed and archived session 234)".
  - Add 13 rows at the top of the "Completed Plans" table, in the format
    of the `fst-*` rows ("2026-09-30 (<ID>, <summary>; CMP-40 combined
    gate passed; archived session 234)"). The `cmp-dispatch.json` row
    says it is the session-226 manifest, superseded for CMP-40 by
    `cmp-closeout-dispatch.json`.
  - The CMP-32 row also notes: "merge note: expect conflicts with the
    main canvas-editor plans J1 (`code/mount.ts`) and C1
    (`app/deps.ts`), and in `app/main.ts`".

### Task 5: plan status and archive (last)

1. Set the `**Status**:` line of each of the 12 plans to `Completed`.
   Some are currently Ready or In Progress. Change no other line of the
   11 accepted plans.
2. Do not edit `cmp-dispatch.json` or `cmp-closeout-dispatch.json`.
   Record the closeout (date, run, gate counts) in this plan's Progress
   Log only.
3. Append this plan's Progress Log entry, then tick its Completion
   Criteria.
4. Move the 13 files from `impl-plans/active/` to `impl-plans/completed/`
   with plain `mv` (no `git mv`), and move this file last. After the
   move, `ls impl-plans/active | grep -E '^(cmp-|eds-|fst-50)'` must
   print exactly `cmp-closeout-dispatch.json`, and
   `ls impl-plans/completed | grep -E '^(cmp-|eds-|fst-50)' | wc -l`
   must print 13.
5. `grep -rn "active/cmp-\|active/eds-\|active/fst-50" README.md impl-plans/README.md design-docs/`
   must have no match except `active/cmp-closeout-dispatch.json`.

## Verification (full gate)

Each command exits 0 with a complete log in
`tmp/cmp-closeout/CMP-40/attempt-<n>/`.

1. `CARGO_TERM_QUIET=true cargo build`
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings`
4. `CARGO_TERM_QUIET=true cargo fmt --check`
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run`:
   0 failed. The pass count must be above 1647 (the session-222
   baseline), and it must include `complete::`, `fmt::tests::repair` and
   `cli::tests::fmt`.
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --features lsp`:
   0 failed. The pass count must be above 1663, and it must include
   `lsp::tests::complete`.
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --release --run-ignored only -E 'test(budget)'`:
   at least 1 passed. Record the median, which must be under 5 ms.
8. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`,
   then the CMP-21 export listing:
   `node --input-type=module -e "import fs from 'node:fs'; const m = await WebAssembly.compile(fs.readFileSync('target/wasm32-unknown-unknown/debug/vactr.wasm')); console.log(WebAssembly.Module.exports(m).map(e => e.name).filter(n => n.startsWith('complete_') || n.startsWith('fmt_') || n === 'session_init').sort().join(' '))"`
   must print exactly
   `complete_out_len complete_out_ptr complete_source fmt_out_len fmt_out_ptr fmt_source session_init`.
9. `mise run ts-build-wasm`, then `mise run ts-test`
10. `mise run fmt-vact-check`
11. `mise run lint`
12. `cd editor && npm run check && npm test && npm run build`: vitest
    has 0 failed and more than 374 passed, including `completion`,
    `completion-types`, `completion-popup`, `completion-view`,
    `syntax-core`, `tool-wasm`, `wasm/complete`,
    `wasm/completion-engine` and `wasm/format`. Step 8 must run first,
    because the wasm tests load the step-8 artifact.

Test ownership: this plan's behavioral evidence is gate steps 5, 6 and 12,
committed nextest and vitest suites with positive counts. No `tmp/`
script counts as evidence.

## Completion Criteria

- [ ] Task 1 checks pass: 30 tracked-diff paths and 40 untracked paths
      exactly as listed, no apis.ts/language.ts diff, no autocomplete,
      and every `.rs` file under 1000 lines.
- [ ] Task 2 items 1-6 are each recorded OK, or are repaired and
      re-verified.
- [ ] Gate steps 1-12 exit 0 with complete logs, and the counts and the
      budget median are recorded. Any BLOCKED step is listed with its
      evidence for the orchestrator to rerun.
- [ ] The README, the two design status sentences and
      `impl-plans/README.md` are updated as in Task 4.
- [ ] The 13 files are in `impl-plans/completed/` and each plan is
      marked Completed. `impl-plans/active/` keeps only
      `cmp-closeout-dispatch.json` of the cmp/eds/fst-50 names, and the
      stale-link grep is clean.
- [ ] The Progress Log is updated with commands, exit codes, log paths
      and hashes.

## Progress Log

### Session: 2026-09-30 (plan created, session 226)
**Tasks Completed**: Plan authored.
**Blockers**: The dispatch failed because the manifest listed directories (riela#128).

### Session: 2026-09-30 (plan rewritten, session 233)
**Tasks Completed**: Rewrote the plan with concrete-file writePaths and sharedPaths. This is a closeout only, with no design change.
**Blockers**: `cmp-dispatch.json` still carried the directory entries, so the run did not dispatch.

### Session: 2026-09-30 (plan revised, session 234)
**Tasks Completed**: Pointed the dispatch at the new `impl-plans/active/cmp-closeout-dispatch.json` (plans [CMP-40], 11 acceptedDependencies, 86 concrete paths). `cmp-dispatch.json` is now moved unchanged. The Task 1 path counts were fixed to 30 tracked plus 40 untracked. The Task 5 archive check now keeps `cmp-closeout-dispatch.json` in active/. The status text now says session 234.
**Blockers**: none

## Related Plans

- **Accepted dependencies**: CMP-10, CMP-15, CMP-20, CMP-21, CMP-30, CMP-31, CMP-32, EDS-10, EDS-11, EDS-12, FST-50
