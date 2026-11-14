# EVM-40: Delete the local engine and its tests, rewire session.test, run final gates and report blockers

**Status**: Ready
**planId**: EVM-40. **Wave**: 4. **dependsOn**: EVM-30.
**Design reference**: `design-docs/specs/design-emaki-vim-migration.md`
section 5 (parity table), VM-D10, section 6 (validation), section 7
(completion rule), section 9 (gates) and section 10 (rollout).
**Rules**: `impl-plans/active/emaki-vim-00-rules.md`.

## Intent and context

After EVM-30, nothing in `editor/src` imports the local engine. This plan
deletes `editor/src/code/vim/` and `editor/test/code/vim/`. Their coverage
moves to emaki and to the retained vactr integration tests, as listed in the
design's section 5 parity table.

The plan then rewires the one outside user of the test helper,
`test/documents/session.test.ts`. Finally it runs every gate and writes the
blocker report.

## Non-goals

- No commit and no push. Commit is finalization's job, and only when the
  completion rule holds. With VMB1, VMB2 or VMB4 open, the tree stays
  uncommitted (design section 10).
- No edits to `vim-mount.ts`, the views, the store, the parity suite or the
  design docs.

## File-level changes

1. Delete the directory `editor/src/code/vim`, including `presets/`.
2. Delete the directory `editor/test/code/vim`: 32 entries, including
   `host.ts` and `purity.test.ts`.
3. Rewrite `editor/test/documents/session.test.ts`:
   - Replace the `VimController` and `createTestHost` imports with
     `createVimController` from `'emaki/vim'` and
     `createVimHost, forwardSurfaceUpdates` from `'../../src/code/vim-mount'`.
   - Build the host with `createVimHost({ surface, viewHost, layout })`.
     `viewHost` and `layout` are minimal stubs cast via `as unknown as`:
     - `viewHost`: `viewport {scrollTop: 0, width: 800, height: 400}`, plus
       no-op `scrollBy` and `scrollCaret`;
     - `layout`: `font.lineHeight 16` and `coordsAtPos: () => null`.
   - Create the controller with `createVimController(host, { timer })`,
     where `timer` uses `setTimeout`/`clearTimeout`, so that
     `vi.useFakeTimers()` controls it. Call
     `forwardSurfaceUpdates(surface, vim)` and dispose both.
   - The key helper keeps the same `ModalKeyInput` objects.
   - **Every** assertion in both "prevents Vim redo across a file load" cases
     and in the other cases stays byte-identical.

Pitfall: emaki `handleKey` returns `false` for meta input. The helper
already sends `metaKey: false`, so do not change it.

## Final gates (rules 12 and 14; log each under `tmp/emaki-vim/EVM-40/`)

Run in `editor/` unless noted:

1. `npm run check`
2. `npx vitest run`
3. `npm run build`
4. `npm run test:style`
5. `npm run e2e:examples`
6. `node test/e2e/vim-behavior.mjs`
7. `node test/e2e/vim-keymap.mjs`
8. `grep -rnE "code/vim/|['\"]\./vim/" src test`. Expect no output (exit 1
   from grep).
9. `test ! -e src/code/vim && test ! -e test/code/vim`. Expect exit 0.
10. From the repo root:
    `git diff --stat a998d1e -- '*.rs' Cargo.toml Cargo.lock editor/package.json editor/package-lock.json`.
    Expect empty output.
11. `git -C /Users/taco/gits/tacogips/vactr log -1 --format=%H`. It must
    still be `a998d1eff1229a445dfb264b082edabfe1047c98`.

## Blocker report (write it in the progress log)

For each of VMB1, VMB2, VMB3 and VMB4, give:

- **Status**: `confirmed-failing`, `not-reproduced` or `n/a`.
- **Failing test names**: from gates 2, 6 and 7. For example, `P1a ...`, or
  the `session.test.ts` file-load redo cases for VMB1.
- **Log path and exit value.**
- **The exact missing emaki API**, as worded in design section 7.

Also list:
- any failure not in EVM-10's BASE-FAIL list and not mapped to a VMB, as a
  `NEW-REGRESSION`;
- any check blocked for environment reasons.

The overall result is `blocked` if any VMB is `confirmed-failing` or any
NEW-REGRESSION exists. Otherwise it is `ready-for-finalization`. Do not
write `completed`.

## Invariants

- `test/documents/session.test.ts` keeps every `expect(...)` line. Verify
  with `git diff -U0 -- test/documents/session.test.ts | grep -E "^[-+].*expect\("`,
  which must print nothing.
- `src/code/file-boundary.ts` and `src/code/surface.ts` are untouched.

## Acceptance criteria

- [ ] Gates 8, 9, 10 and 11 pass as described.
- [ ] Gates 1 and 3 exit 0.
- [ ] Gates 2 and 4 to 7 are run, each with an exit value and log path. Each
      failure is classified as VMB1 to VMB4, BASE-FAIL, NEW-REGRESSION or
      environment-blocked.
- [ ] The session.test invariant holds.
- [ ] The blocker report is complete, and the overall result is stated as
      `blocked` or `ready-for-finalization`.
- [ ] The progress log is updated.

## Amendment 2026-11-11 (blocked-wave-scope-or-contract, round 1)

This plan runs the same generated-output gates that blocked EVM-10, so it
takes the same decision in advance. The four artifact roots below are added
to `writePaths` and `artifactRoots`:

| Path | Written by |
|---|---|
| `tmp/emaki-vim/cargo-target` | cargo wasm build |
| `editor/dist` | `npm run build` |
| `tmp/ui-style/after` | `npm run test:style` |
| `tmp/examples-e2e` | `npm run e2e:examples` |

EVM-40 runs in wave 4 and EVM-10 in wave 1, so there is no same-level clash
over these paths.

**Rebuild contract.** Use the EVM-10 amendment's rebuild contract unchanged:
1. `CARGO_TARGET_DIR=tmp/emaki-vim/cargo-target mise run build-wasm-release`
2. `VACTR_WASM=../tmp/emaki-vim/cargo-target/wasm32-unknown-unknown/release/vactr.wasm VACTR_REQUIRE_SESSION_ABI=1 npm run build`

The shared repo `target/` is not authorized.

**Readback.** Run `git status --porcelain --ignored -- target editor tmp`.
Every new or changed path must be under:
- one of the four artifact roots;
- `tmp/emaki-vim/EVM-40`; or
- this plan's other `writePaths`.

Any other path is a new blocker. Report it and do not delete it.

## Progress Log
