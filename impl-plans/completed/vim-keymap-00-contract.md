# Vim Keymap: Core Contract (types, test host, mount stubs) Implementation Plan

**Status**: Completed
**Plan ID**: VK-00 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 1, 4.1, 4.3, 4.4, 5, 6.4, 7.2, 7.4, 11.1
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-01 notation and schema, VK-02 Tauri store, VK-03 web store and JSON files, VK-04 renderer hop overlay
- **Next (wave 1, depend on this plan)**: VK-10 mapper, VK-11 hints, VK-12 hop, VK-13 extended built-ins, VK-15 input hooks, VK-16 views
- **Then**: VK-20 controller (wave 2), VK-30 mount (wave 3), VK-40 e2e and gates, VK-41 docs (wave 4)
- **Manifest**: `impl-plans/active/vim-keymap-dispatch.json`

## Intent and Context

The optional Vim mode of the canvas code pane (`editor/src/code/vim/`,
design `design-vim-mode.md`) is implemented. This feature adds a JSON
keymap, next-key hints and hop jumps (`design-vim-keymap-hints-hop.md`).
The Vim core is pure TypeScript over `@codemirror/state`; the adapters in
`editor/src/code/` own the DOM.

This plan pins the **shared type changes** in `vim/types.ts` so that every
wave-1 plan can compile against them in parallel. It extends the test
host with the new `VimHost` members, adds minimal compile-only stubs in
`vim-mount.ts` (replaced by VK-30), enables JSON imports for the preset
files, and tightens the purity test.

## Common Execution Rules (apply to every vim-keymap plan)

1. **Ownership.** Edit only this plan's `writePaths` (and a `sharedPaths`
   entry only for the edit its note describes). Before each edit, read the
   file fresh and record `git hash-object <file>` (or `missing`) in the
   Progress Log; re-hash after the edit. If a file outside your paths
   changed under you, do not touch or restore it; note it as drift.
2. **No git writes.** Never run `git commit`, `checkout`, `stash`,
   `reset`, `restore`, or create branches or worktrees. Read-only
   `git diff`, `git status`, `git hash-object` are allowed.
3. **Logs.** Run every command in the foreground from `editor/` unless
   stated. Tee full output to
   `../tmp/vim-keymap/<planId>/attempt-<n>/<name>.log` and append the exit
   status as the last line (`echo "exit=$?" >> <log>` as a separate step).
   A truncated log is not a pass.
4. **Verification records.** Each gating command goes into the Progress
   Log as exactly
   `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
   Intentional negative-control runs go only in `mutationEvidence`. A
   repaired review finding goes only in `addressedFeedback` with status
   `repaired`.
5. **Scoped typecheck (waves 0 and 1).** Sibling plans may be mid-edit, so
   project-wide `npm run check` can fail on their files. Write
   `../tmp/vim-keymap/<planId>/tsconfig.json` as
   `{"extends": "../../../editor/tsconfig.json", "include": [<this plan's .ts/.tsx files relative to that tsconfig>]}`
   and run `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/<planId>/tsconfig.json`.
   Waves 2-4 run the full `npm run check`.
6. **Purity.** No file under `editor/src/code/vim/` may reference
   `document`, `window`, `navigator`, `localStorage`, `HTMLElement`,
   `setTimeout`, `clearTimeout`, `setInterval` or `@codemirror/view`.
   Timers come only from `VimHost.timer`.
7. **No-config invariant (K0).** With no keymap compiled, every existing
   Vim test must pass unchanged. Never edit an existing test file unless
   it is in your `writePaths`.
8. **Style.** English only, no emojis. Match the dense style of
   `editor/src/code/vim/controller.ts` and `keys.ts`. Every file stays
   under 1000 lines; budgets are given per plan.
9. **Licensing.** Write behavior from scratch. Do not copy code from
   hop.nvim, mini.clue, which-key, vim-surround, Vim or
   `@replit/codemirror-vim`. If permissive code would be adapted, stop and
   report it (`THIRD_PARTY_NOTICES.md` is not in any plan's paths).
10. **Heavy suites.** Full vitest, Playwright and `npm run test:style` run
    only inside the measurement lock: `mkdir
    /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock` (wait and
    retry while it exists), run, then `rmdir` it. Focused vitest runs need
    no lock.
11. **Progress Log.** Append one session entry to your own plan file
    (tasks done, hashes, verification records, drift, blockers). Do not
    edit other plans, the manifest or `impl-plans/README.md`.

## Non-Goals

- No mapper, hint, hop, parser or controller behavior (later plans).
- No real host implementations in `vim-mount.ts` beyond the compile-only
  stubs listed below (VK-30 replaces them).
- No new npm dependency. Do not touch `controller.ts`, `parser.ts`,
  `keys.ts`, `input.ts`, `renderer.ts`, `mount.ts`.

## File-Level Changes

### editor/src/code/vim/types.ts (budget: under 140 lines)

Add exactly these exports (existing exports unchanged):

```ts
export type MotionKind = /* existing members */ | 'g;' | 'g,';
export type MapMode = 'n' | 'i' | 'x' | 'o' | 'c';
export interface HintEntry { key: string; desc: string; group: boolean }
export interface HintView { prefix: string; entries: readonly HintEntry[]; summary: string }
export interface HopLabel { pos: number; text: string }
export interface HopView { labels: readonly HopLabel[]; revision: number }
```

`VimActions` gains three required members:
`diagnosticNext(from: number): number | null`,
`diagnosticPrev(from: number): number | null`, `openCompletion(): void`.

`VimHost` gains four required members:
`typeText(text: string): void`, `defaultKey(key: VimKey): boolean`,
`isVisible(pos: number): boolean`,
`timer(ms: number, cb: () => void): () => void` (returns a cancel
function).

`VimSnapshot` gains two **optional** fields: `hints?: HintView | null`
and `hop?: HopView | null` (optional so `controller.ts` compiles
unchanged until VK-20).

`VimInputHook` gains one **optional** method:
`flushPendingInput?(): void`.

Pitfall: adding `'g;' | 'g,'` to `MotionKind` must not break
`motions.ts` (`runMotion` has `default: return null`) or `record.ts`
(`Partial<Record<MotionKind, string>>`). Confirm with the scoped
typecheck including those files.

### editor/test/code/vim/host.ts

Extend `createTestHost` (keep its signature compatible, new options
optional):

- `actions` gains `diagnosticNext`, `diagnosticPrev`, `openCompletion` as
  `vi.fn()`; the two diagnostic functions return the value from a new
  option `diagnostics?: number[]` (next: the first position greater than
  `from`, wrapping; prev: mirror; `null` when the list is empty or
  absent).
- `typeText(text)` dispatches `{ changes: { from: sel.from, to: sel.to, insert: text }, selection: { anchor: from + text.length }, userEvent: 'input.type' }`
  and records the text in a new `typed: string[]` array on `TestHost`.
- `defaultKey(key)` records into `defaultKeys: string[]`, applies
  `<BS>` (delete one char before the cursor, `userEvent: 'delete.backward'`),
  `<Del>` (`delete.forward`), `<Left>`/`<Right>` (move one), `<CR>`
  (insert `\n`, `input.type`), returns true for those and false otherwise.
- `isVisible(pos)` returns the option `isVisible?.(pos) ?? true`.
- `timer(ms, cb)` uses `setTimeout`/`clearTimeout` (tests drive it with
  `vi.useFakeTimers()`), returning the cancel function.

Imitate the existing style of `host.ts` (`createTestHost`).

### editor/src/code/vim-mount.ts (shared; compile-only stubs)

In `createSurfaceHost` only, add stubs so the project compiles:
`typeText: (text) => input.replaceSelection(text, 'input.type')`,
`defaultKey: () => false`, `isVisible: () => true`,
`timer: (ms, cb) => { const id = setTimeout(cb, ms); return () => clearTimeout(id); }`,
and in `actions`: `diagnosticNext: () => null`, `diagnosticPrev: () => null`,
`openCompletion: () => undefined`. Add a one-line comment
`// Replaced by the keymap mount wiring (VK-30).` Nothing else changes.

### editor/tsconfig.json (shared)

Add `"resolveJsonModule": true` to `compilerOptions` (needed by VK-14 to
import `vim/presets/*.json`; Vite already supports JSON imports).

### editor/test/code/vim/purity.test.ts

Extend the regex check with a timer rule: a violation when a file
references `setTimeout|clearTimeout|setInterval|clearInterval` as an
identifier. Keep the DOM and `@codemirror/view` rules. The existing
`files.length >= 3` assertion stays.

## Invariants

- No runtime behavior change: `vim-mount.ts` stubs are unreachable
  because no code calls the new members yet.
- Existing tests under `editor/test/code/vim/` and `editor/test/canvas/`
  pass unchanged.

## Test Cases

- `editor/test/code/vim/host-keymap.test.ts` (new):
  - `typeText('ab')` on `x|y` -> doc `xab|y`, `typed` is `['ab']`, update `userEvent` is `input.type`
  - `defaultKey('<BS>')` on `ab|` -> `a|`, returns true; `defaultKey('<F1>')` -> false
  - `timer(100, cb)` with fake timers: cb runs after `advanceTimersByTime(100)`; cancel before -> never runs
  - `diagnostics: [3, 10]`: `diagnosticNext(3)` -> 10, `diagnosticNext(10)` -> 3 (wrap), `diagnosticPrev(3)` -> 10; no option -> null
- `purity.test.ts`: still passes on the current tree.

## Verification (from editor/)

1. Scoped typecheck including `src/code/vim/types.ts`, `src/code/vim/motions.ts`, `src/code/vim/record.ts`, `src/code/vim/controller.ts`, `src/code/vim-mount.ts`, `test/code/vim/host.ts`, `test/code/vim/host-keymap.test.ts` -> exit 0.
2. `./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts` -> exit 0, failureCount 0, testsRun at least the pre-change count plus the new cases.
3. `npm run check` -> exit 0 (wave 0 siblings do not touch files this check would fail on; if it fails only on a sibling's file, record it as drift and rely on step 1).

## Completion Criteria

- [x] types.ts exports exactly the pinned additions; file under 140 lines
- [x] host.ts implements every new member; host-keymap.test.ts has at least 8 passing cases
- [x] vim-mount.ts contains only the listed stubs
- [x] tsconfig.json has `resolveJsonModule: true`
- [x] purity.test.ts rejects timer globals and passes
- [x] Verification records 1-3 logged; full check failure is confined to concurrent sibling files and scoped typecheck passes
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-00 implementation

**Tasks Completed**: Added the pinned shared types, test-host behavior and 11 host tests, compile-only mount stubs, JSON module resolution, and the timer-global purity rule.

**Source Hashes**:
- `editor/src/code/vim/types.ts`: `f2f683bcd8ae79b9879d81edd9e551708a494687` (77 lines)
- `editor/test/code/vim/host.ts`: `687af56ce09765227c35bd918e8eb62ed9419ff9`
- `editor/test/code/vim/host-keymap.test.ts`: `423bbd8ce04a1efc48b409f959f4f5947954749d` (11 cases)
- `editor/test/code/vim/purity.test.ts`: `11642c2323020157ed371ab26b4a5ec374a2c5b8`
- `editor/src/code/vim-mount.ts`: `785a9fc69321b0177c6923fc7065ed0ef46ee5dc`
- `editor/tsconfig.json`: `8a27b24d9fe1b993d1c891fa8b7437f3ab252c86`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-00/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-00/attempt-1/scoped-typecheck.log", notes: "Included the plan-pinned Vim types, motions, record, controller, mount, test host and host-keymap test."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts", exitStatus: 0, testsRun: 841, testsPassed: 841, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-00/attempt-1/vitest-focused.log", notes: "22 files passed, including all 11 new host cases and purity."}`
- `{command: "cd editor && npm run check", exitStatus: 1, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "sibling-drift", log: "tmp/vim-keymap/VK-00/attempt-1/npm-check.log", notes: "TypeScript errors are in concurrent VK-04 test/canvas/renderer-hop.test.ts, VK-03 test/code/vim-keymap-store.test.ts, and VK-01 test/code/vim/keymap-schema.test.ts. These paths are outside VK-00 writePaths; the scoped check above passes. No sibling files were edited."}`

**Evidence**: Per-edit intentions and verification logs are under `tmp/vim-keymap/VK-00/attempt-1/`. The provided design-review concern about mapper argument routing is outside VK-00's file ownership and belongs to the mapper/controller plans; this plan adds only the contract seam.

**Drift**: The shared worktree contains concurrent edits in other plan paths. Preserved without modification.

**Blockers**: None for VK-00. Repository-wide check must be rerun after concurrent plan files are repaired.
