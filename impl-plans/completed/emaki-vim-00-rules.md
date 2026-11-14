# emaki-vim plans: common execution rules (read-only for every worker)

**Applies to**: every plan in `impl-plans/active/emaki-vim-dispatch.json`.
**Design**: `design-docs/specs/design-emaki-vim-migration.md`, accepted by
design review comm-019334 in workflow session 839. The open user question is
EQ7 in `design-docs/user-qa/pending-emaki-questions.md`. Its default is
"(a) no scope amendment", so parity stays required.
**Branch**: `main`, base `a998d1e`, one shared working directory
`/Users/taco/gits/tacogips/vactr`. Do not use private branches or worktrees.
Workers never run `git commit`, `git push`, `git stash`, `git checkout` or
`git reset`.

## User intent in one paragraph

Delete vactr's private Vim engine (`editor/src/code/vim/**`) and use the same
engine from the installed `emaki/vim` package (emaki `9dba0a3`, at
`editor/node_modules/emaki`). User-visible behavior must not change. Where
emaki lacks an API that is needed for parity, record it as a blocker. Do not
re-implement engine logic in vactr.

Four such gaps are already known from source:

| ID | Missing API | Parity check |
|---|---|---|
| VMB1 | document-boundary reset | P1 |
| VMB2 | `ex` status message | P3 |
| VMB3 | meta-key routing (conditional) | P4 |
| VMB4 | `normalizeKey` export | P2 |

The run is therefore expected to end `blocked`. A blocked plan is still done
when its own deliverables are in place and its evidence names the blocker
precisely.

## Hard constraints (all plans)

1. Do not modify `/Users/taco/gits/tacogips/emaki`, `editor/node_modules/**`,
   `editor/package.json`, `editor/package-lock.json`, any `*.rs`,
   `Cargo.toml` or `Cargo.lock`.
2. Do not run `npm install`, `npm ci` or `npm update`. The pin mismatch
   (package.json `6138022`, node_modules `9dba0a3`) is intentional.
3. Import engine symbols only from `'emaki/vim'`, or from `'emaki/engine'` /
   `'emaki'` where that is the existing pattern. Never deep-import
   `emaki/src/...`.
4. Do not copy engine logic into vactr. That includes `normalizeKey`, ex
   parsing, the undo ledger, dot-repeat, and key filtering for meta keys.
5. Never weaken, skip, delete or relax an existing behavioral assertion. Do
   not use `it.skip`, `it.fails`, `it.todo`, a loosened matcher or a deleted
   case.
   - Allowed: import-specifier and type-name changes.
   - Allowed: changing `vactr.*` to `emaki.*` only where the assertion
     checks compiled engine IDs (design 5.2).
   - Allowed: adding `hints: null, hop: null` to snapshot fixtures.
   - If an assertion fails because of VMB1 to VMB4, leave it failing and
     report it.
6. The parity suite `editor/test/canvas/vim-parity.test.ts` is owned by
   EVM-10 and is read-only for every later plan.

## Ownership and drift

7. Edit only your plan's `writePaths`. Edit `sharedPaths` only for the
   purpose stated in `sharedPathNotes`. Every other path is read-only.
8. Before the first edit of a file:
   - write one sentence of intent to `tmp/emaki-vim/<planId>/intent.txt`;
   - record `shasum -a 256 <file>` (or `absent`) in
     `tmp/emaki-vim/<planId>/sha256.txt`.

   Re-read the file right before each edit, and record the post-edit hash.
9. Before final verification, rehash every file you edited. If a hash changed
   and you did not change it, another worker drifted it. Stop, record the
   drift, and do not repair it. Repair happens serially after the wave joins.
10. If a fix is needed outside your writePaths, stop and report a scope
    amendment with the exact path and reason. Do not edit the file.

## Commands and evidence

11. Run every command in the foreground. Log each one as
    `<cmd> > tmp/emaki-vim/<planId>/<n>-<name>.log 2>&1; echo "exit=$?" >> <log>`.
    The log path and the exit value are the evidence. A truncated log is not
    a pass. Do not name a shell variable `status`.
12. Heavy runs (full `npx vitest run`, `npm run build`, `npm run test:style`,
    `npm run e2e:examples`, `node test/e2e/vim-*.mjs`) take the shared lock:
    - acquire: `until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done`
    - release: `rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`

    Release it also on failure. If the lock path exists as a regular file and
    no vitest or Playwright process is running, it is stale: delete it. If
    the sandbox cannot create the lock, run without it and say so.
13. Filtered checks for a single plan:
    - `cd editor && npx vitest run <files...>`
    - `cd editor && npx tsc --noEmit -p . > <log> 2>&1`, then filter the log
      with `grep -E '<owned files>'`. This is only for mid-migration waves,
      where errors in files owned by later plans are expected and listed in
      each plan.
14. **Superseded for EVM-10 and EVM-40.** Those plans use the rebuild
    contract in their round-1 amendment sections instead: a plan-owned
    `CARGO_TARGET_DIR=tmp/emaki-vim/cargo-target` plus `VACTR_WASM`, with
    generated outputs only under the declared artifact roots. The shared
    repo `target/` must not be written. Original text, kept for reference:

    Before any e2e or style script, rebuild:
    - `cd /Users/taco/gits/tacogips/vactr && CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
    - `mise run build-wasm-release`
    - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`

    If `git status --porcelain` then shows generated tracked files changed by
    the rebuild, list them in the progress log. Do not stage or revert them;
    finalization decides. If a browser runtime is missing, record the check
    as **blocked** with its log. It is not a pass.

## Progress log

15. Append to your own plan file only, under `## Progress Log`, a
    `### Session <YYYY-MM-DD HH:MM>` entry with:
    - tasks done;
    - files touched, with pre and post hashes;
    - each command, with its log path and exit value;
    - blockers by ID (VMB1 to VMB4, or new ones), each with the exact missing
      API and the failing test name;
    - drift observed.

    Tick completion checkboxes only for criteria with evidence. Do not edit
    other plans, `impl-plans/README.md` or the dispatch JSON.

## Operator closeout (2026-11-11)

The workflow run (session 839) stopped at EVM-10 on baseline gate friction (Vitest cache writes, load-induced timeouts). The operator closed VMB1-VMB4 upstream in emaki d6f8609 (hooks.isDocumentBoundary, ex `{ message, kind }` results, hooks.routeMetaKeys, normalizeKey export) and completed EVM-20..40 directly: editor/src/code/vim/** and editor/test/code/vim/** are deleted, vim-mount builds the engine with createVimController over createVimHost(CodeSurface), app actions are registered under emaki.* IDs, legacy vactr.* IDs are migrated at compile time, and keymap stores implement emaki KeymapStore. Gates on the final tree: npm run check 0; npx vitest run 0 (1246 tests, including vim-parity P1-P4 11/11); npm run build 0; vim-behavior.mjs 0; vim-keymap.mjs 0; test:style 0; e2e:examples 8/8 (a first run under load average ~29 timed out 4 checks, the rerun passed).
