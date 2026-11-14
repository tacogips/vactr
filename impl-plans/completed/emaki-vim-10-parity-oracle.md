# EVM-10: Mounted parity oracle and base-state gate baseline

**Status**: Ready
**planId**: EVM-10. **Wave**: 1, which runs alone because it must observe the
unmodified base. **dependsOn**: none.
**Design reference**: `design-docs/specs/design-emaki-vim-migration.md`
sections 7 (completion rule) and 9.1 (P1 to P4), VM-D5, VM-D6, VM-D9.
**Rules**: `impl-plans/active/emaki-vim-00-rules.md` (read first).

## Intent and context

The migration must not change user-visible behavior. The existing unit
tests use stub resolvers and stub controllers, so they cannot prove parity.

This plan writes a mounted parity suite that drives the real production
wiring (`mount()`, then `createVimMode`) and encodes today's behavior at
`a998d1e`. It must pass **before** any migration edit, which proves it is a
valid oracle. After the migration, later plans run it unchanged, and any
failure names a blocker.

This plan also records the base-state results of the e2e and style gates,
so that later regressions can be told apart from pre-existing failures.
Earlier runs recorded pre-existing `test:style` and behavior e2e failures.

## Non-goals

- No edit to any `src/` file or to any existing test.
- No import of any engine module. The suite must import nothing from
  `src/code/vim/**`, `emaki/vim` or `src/code/vim-keymap-store`, so that it
  survives the migration with zero edits.

## Deliverables

- New `editor/test/canvas/vim-parity.test.ts`, with
  `// @vitest-environment jsdom`.
- `tmp/emaki-vim/EVM-10/` logs for the base-state gates (task 2).

### Task 1: write the parity suite

Copy the rig from `editor/test/canvas/vim-mount.test.ts`: `setup()`,
`rafHost()`, `key()`, and the `afterEach` cleanup, including
`localStorage.clear()`. Copy it locally; do not import it.

- Turn Vim on with
  `rig.root.querySelector<HTMLButtonElement>('.vact-vim-toggle')!.click()`.
- The surface is `rig.deps.code!.surface`.
- Mode is read from `.vact-vim-status` `data-mode`.
- Prompt keys go to `.vact-vim-ex`: set `.value`, then dispatch a
  `KeyboardEvent('keydown', { key, ctrlKey, bubbles: true, cancelable: true })`
  on it.
- The status message is `.vact-vim-message`, with text content and a
  `data-kind` attribute.
- To dispatch a file boundary, copy `editor/src/documents/session.ts:32` and
  `:45`: replace the full document with
  `annotations: [Transaction.addToHistory.of(false), Transaction.userEvent.of('input.load.document')]`
  (and `'input.replace.document'` for the second case). `Transaction` comes
  from `@codemirror/state`.
- After each key or dispatch, flush with `await Promise.resolve()` or
  `vi.waitFor`, because snapshot delivery and keymap loading are async.

Test cases (`situation -> expected`):

- **P1a**, for each of `input.load.document` and `input.replace.document`.
  The doc is `abc` and Vim is on. Press `x` in normal mode, so the doc is
  `bc` and a dot-repeatable change exists. Press `i`, so `data-mode` is
  `insert`. Dispatch the boundary that sets the doc to `new text`. Then:
  - `data-mode` is `normal`;
  - pressing `.` leaves the doc `new text`;
  - pressing `u` leaves the doc `new text`.
- **P1b**. Vim is on and `v` makes `data-mode` `visual`. Dispatch
  `input.load.document`. `data-mode` is `normal`.
- **P2a**. Before the toggle click, set `localStorage` `vactr.vimKeymap` to
  a minimal valid keymap: `version: 1` and `maps.c` set to
  `[{ "lhs": "<C-g>", "rhs": "<Esc>" }]`. Copy the top-level shape (leader,
  timeoutlen, hints, hop) from `editor/src/code/vim/presets/default.json` as
  a JSON literal; do not import it. Turn Vim on and wait until loaded. Press
  `:` so `.vact-vim-ex` exists. Ctrl-g keydown on `.vact-vim-ex` removes
  `.vact-vim-ex`, and `event.defaultPrevented` is `true`.
- **P2b**. Same setup with `maps.c` set to
  `[{ "lhs": "<C-j>", "rhs": "<CR>" }]`. Press `:`, set the input value to
  `eval`, and send Ctrl-j keydown. The prompt closes and
  `rig.transport.of('eval')` has length 1. Copy the eval assertion style from
  `vim-mount.test.ts` "routes registered Mod shortcut".
- **P3**, for each of `w`, `q`, `wq`, `x` and `w!`. Press `:`, set the value,
  then send Enter keydown on `.vact-vim-ex`. `.vact-vim-message` text is
  exactly `vactr syncs the document live; use :eval, :evalall or :hush`,
  `data-kind` is `info`, and the doc is unchanged.
- **P4**. Set the platform to `MacIntel` with the
  `Object.defineProperty(navigator, 'platform', ...)` pattern from
  `vim-mount.test.ts` (lines 130-140), and restore it in `finally`. Vim is
  on, in normal mode, with doc `abc def`. Cmd-Backspace and Cmd-Delete
  keydowns on the textarea leave the doc unchanged.

Pitfalls:

- Set up the keymap in localStorage **before** clicking the toggle. Loading
  happens once, in `enable()`.
- The default store is used because `EditorDeps` has no `vimKeymap`. Do not
  pass a store.
- Do not assert on internal snapshot objects. Use the DOM and
  `surface.state.doc` only.
- The P1, P2 and P3 expectations are fixed by the design. If one of them
  fails at base, that is a design defect. Stop and report it as
  `ORACLE-INVALID-P<n>`; do not change the expectation.
- P4 is the only case whose expected outcome may be adjusted to the observed
  base behavior. If Cmd-Backspace edits the doc at base, assert the observed
  base result and record the observation. P4 encodes base behavior.

### Task 2: base-state gate baseline

Run on the unmodified tree (rules 12 and 14), logging each command under
`tmp/emaki-vim/EVM-10/`:

1. `npx vitest run test/canvas/vim-parity.test.ts`
2. `npm run check`
3. `npx vitest run`
4. `npm run build`
5. `npm run test:style`
6. `npm run e2e:examples`
7. `node test/e2e/vim-behavior.mjs`
8. `node test/e2e/vim-keymap.mjs`

For every non-zero exit, list the failing test or check names in the
progress log as `BASE-FAIL` entries. Later plans compare against this list.

## Invariants

- At the end of this plan, `git status --porcelain -- editor/src editor/test`
  shows only `?? editor/test/canvas/vim-parity.test.ts`.

## Acceptance criteria

- [x] `editor/test/canvas/vim-parity.test.ts` exists and contains the cases
      P1a (x2), P1b, P2a, P2b, P3 (x5) and P4.
- [x] `grep -nE "code/vim/|emaki/vim|vim-keymap-store" editor/test/canvas/vim-parity.test.ts`
      returns nothing.
- [x] Gate 1 exits 0 at base.
- [x] Gates 2 to 8 are run at base, with their exit values and BASE-FAIL
      lists recorded.
- [x] The progress log is updated.

## Verification commands (run in `editor/`)

| Command | Evidence it must establish |
|---|---|
| `npx vitest run test/canvas/vim-parity.test.ts` | exit 0 at base; the oracle is valid |
| `npm run check` | base exit value recorded |
| `git status --porcelain -- src test` | only the new test file |

## Amendment 2026-11-11 (blocked-wave-scope-or-contract, round 1)

**Blocker.** The required rebuild and the gates write generated outputs that
the plan had not authorized: `target/`, `editor/dist/`, `tmp/ui-style/after`
and `tmp/examples-e2e/`. See checkpointAmendment for EVM-10, round 0.

**Decision.** Generated outputs are authorized as plan-owned **artifact
roots**. They are tracked as digest manifests, not content. These exact paths
are added to both `writePaths` and `artifactRoots`:

| Path | Written by |
|---|---|
| `tmp/emaki-vim/cargo-target` | cargo wasm build |
| `editor/dist` | `npm run build` |
| `tmp/ui-style/after` | `npm run test:style`, via `editor/test/style/ui-style.mjs:6` |
| `tmp/examples-e2e` | `npm run e2e:examples`, via `editor/test/e2e/examples.mjs:8` |

**Rebuild contract.** This replaces rules item 14 for EVM-10 and EVM-40.

The shared repo `target/` is **not** authorized. It is a large pre-existing
cache, and writing into it is out of scope. Build the release wasm into the
plan-owned target dir, then point the editor build at it:

1. From the repo root:
   `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/emaki-vim/cargo-target mise run build-wasm-release`.
   The task is `mise.toml:44-46`. The output is
   `tmp/emaki-vim/cargo-target/wasm32-unknown-unknown/release/vactr.wasm`.
2. From `editor/`:
   `VACTR_WASM=../tmp/emaki-vim/cargo-target/wasm32-unknown-unknown/release/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`.
   `editor/vite.config.ts:6-7,70` reads `$VACTR_WASM`.
3. Skip the debug `cargo build --lib --target wasm32-unknown-unknown` step
   from rule 14. No gate consumes its output.

**Readback.** After the gates, run
`git status --porcelain --ignored -- target editor tmp`. Every new or changed
path must lie under:
- one of the four artifact roots;
- `tmp/emaki-vim/EVM-10`;
- `editor/test/canvas/vim-parity.test.ts`; or
- this plan file.

If any other path appears (for example `target/`, `editor/node_modules/.vite`
or `tmp/canvas-cutover`), stop. Record the exact path as a new blocker for
amendment round 2. Do not delete it.

**Unchanged.**
- Gates 3 to 8 still run in the order of Task 2, logged under
  `tmp/emaki-vim/EVM-10/`.
- Gates 1 and 2 and the oracle file are already done and are not redone,
  except that gate 1 may be rerun for confirmation.
- A missing browser runtime is recorded as environment-blocked, not as a
  pass.
- BASE-FAIL lists are recorded for every non-zero exit.

## Progress Log


### Session 2026-11-11 14:26

- Added the mounted P1a (two), P1b, P2a/b, P3 (five), and P4 oracle. No source or existing test changed.
- Oracle pre-hash: absent; post-hash: `5252b492bf0643eded361caaabfd5aa3b85e7f3c7e171f37887b862d8302ccf9`. Plan pre-hash: `c6602ae26f939f0f7659c2622b7e542b1f10307a0e22ae187451c586d9e7b239`; post-hash recorded in `tmp/emaki-vim/EVM-10/sha256.txt`. No drift observed.
- `npx vitest run test/canvas/vim-parity.test.ts`: exit 0, 11 passed; `tmp/emaki-vim/EVM-10/1-oracle.log`.
- `npm run check`: exit 0; `tmp/emaki-vim/EVM-10/2-check.log`.
- Forbidden import grep: exit 1, no matches; `tmp/emaki-vim/EVM-10/import-check.log`.
- Gates 3–8 and required rebuilds remain unrun: artifactRoots is empty, but the required build writes `target/` and `editor/dist/`, style writes `tmp/ui-style/after`, and examples writes `tmp/examples-e2e/`. A checkpoint amendment authorizing generated output paths is required before continuing; do not count these as passes or BASE-FAIL.
- VMB1, VMB2 and VMB4 are design-confirmed missing emaki APIs; this base oracle passes and does not establish post-migration failures. Formal review and downstream migration remain pending.

### Session 2026-11-11 14:36 — amendment round 1

- Baseline source remained unchanged; oracle SHA-256 `5252b492bf0643eded361caaabfd5aa3b85e7f3c7e171f37887b862d8302ccf9`. Plan pre-edit SHA-256 `ae9b8773ac20b24e53bef24c96d8ad1e416aa22ec8dd82c7444c46432907fd09`; post-edit hash is in `tmp/emaki-vim/EVM-10/round1-post-edit-sha256.txt`.
- Gate 3 `npx vitest run`: exit 1, 2400 passed, 2 failed; `tmp/emaki-vim/EVM-10/3-vitest-round1.log`. BASE-FAIL: `test/wasm/canvas-clock.test.ts > canvas transport telemetry (real host-wasm artifact) > removes commented-out playback on whole-document Update while preserving other files` and `test/wasm/criteria.test.ts > criterion 1: playing events and highlighting on the audio clock > stamps every event with the eval revision, within one lookahead of the tick that emitted it`; both timed out after 5000 ms.
- `CARGO_TERM_QUIET=true CARGO_TARGET_DIR=tmp/emaki-vim/cargo-target mise run build-wasm-release`: exit 0; `tmp/emaki-vim/EVM-10/rebuild-wasm-round1.log`.
- Gate 4 `VACTR_WASM=../tmp/emaki-vim/cargo-target/wasm32-unknown-unknown/release/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`: exit 0; `tmp/emaki-vim/EVM-10/4-build-round1.log`.
- Gate 5 `npm run test:style`: exit 0, 16 engine/viewport combinations; `tmp/emaki-vim/EVM-10/5-style-round1.log`. Gate 6 `npm run e2e:examples`: exit 0, 8/8 checks; `tmp/emaki-vim/EVM-10/6-examples-round1.log`. Gate 7 `node test/e2e/vim-behavior.mjs`: exit 0; `tmp/emaki-vim/EVM-10/7-vim-behavior-round1.log`. Gate 8 `node test/e2e/vim-keymap.mjs`: exit 0; `tmp/emaki-vim/EVM-10/8-vim-keymap-round1.log`.
- Readback: `tmp/emaki-vim/EVM-10/readback-round1.log`; source status shows only the new oracle. Vitest wrote `editor/node_modules/.vite/vitest/da39a3ee5e6b4b0d3255bfef95601890afd80709/results.json` at 14:35:13, outside authorized artifact roots; see `tmp/emaki-vim/EVM-10/cache-write-round1.log`. Do not remove it. Scope reconciliation or checkpoint amendment is required. No source drift observed.
- VMB1, VMB2 and VMB4 remain design-level downstream blockers; this baseline does not establish post-migration failures. Formal review remains pending.
