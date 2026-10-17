# Vim Mode: Input Routing and Text-Input Gate Implementation Plan

**Status**: Completed
**Plan ID**: VIM-15 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 3.3 (all subsections), 3.5
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00 (`VimInputHook`, `VimKeyInput` in `vim/types.ts`)
- **Parallel (wave 1)**: VIM-10..14, VIM-16, VIM-17, VIM-18
- **Consumers**: VIM-20 implements `VimInputHook`; VIM-30 installs it through `InputController.setVim`

## Intent and Context

`editor/src/code/input.ts` `InputController` owns the hidden textarea.
Its keydown handler (line 52) does:
`if (!this.composing && !key.isComposing && key.keyCode !== 229 && !this.surface.runKeymaps(key)) this.keyboard.handle(key);`.
`editor/src/code/surface.ts` `runKeymaps` iterates the highest groups,
then the default groups. The highest group holds the completion keys
(`completion-view.ts` registers `COMPLETION_KEYS` at `'highest'`); the
default group holds eval (`eval.ts:88-90`) and format (`format.ts:29`).
The private `matchesKey` maps `Mod` to Meta when `navigator.platform`
matches `/Mac|iP/`, and to Ctrl otherwise.

This plan adds an optional Vim hook to `InputController`. With no hook,
behavior is byte-identical. With a hook, the plan adds the 4-step routing
of design 3.3, the non-insert text-input gate of 3.5, discarded
compositions, and replace-mode overwrite. It also adds the read-only
`CodeSurface.matchesKeymap` query used for the pass-through set P1. The
common execution rules in `vim-mode-00-contract.md` apply.

## Non-Goals

- No Vim logic. The hook decides; this plan only routes and gates.
- Do not change `keyboard.ts`, `completion*.ts`, `eval.ts` or `format.ts`,
  or the precedence or bindings of any keymap.
- Do not change any existing test file. The existing
  `test/canvas/input.test.ts` must pass unchanged.

## File-Level Changes

### `editor/src/code/surface.ts`

- Change the `matchesKey` parameter type to
  `Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>`.
  Its body is unchanged.
- `runKeymaps(event: KeyboardEvent, precedence?: 'highest' | 'default'): boolean`.
  With `precedence` undefined, iterate exactly as today
  (`[...this.highestKeymaps, ...this.defaultKeymaps]`). Otherwise iterate
  only the named registry. The `preventDefault` behavior is unchanged.
- New method
  `matchesKeymap(event: Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>, precedence: 'highest' | 'default'): boolean`.
  It returns true if any binding in that registry matches. It must **not**
  call `run()`.
- Imitate the existing dense one-line style. The file is 173 lines and
  must stay under 220.

### `editor/src/code/input.ts`

Add `import type { VimInputHook } from './vim/types'`. Then:

- **Hook storage.** Add `private vim: VimInputHook | null = null`,
  `setVim(hook: VimInputHook | null): void`, and
  `private discarding = false` (the composition being discarded).
- **Keydown routing** (replace line 52's routing only; keep the
  composition guard and the `composing && Escape` branch before it as
  is):
  - If `this.vim === null`, run the exact current expression.
  - Otherwise, when the same guard passes (not composing, not
    `isComposing`, `keyCode !== 229`), run in order:
    1. `if (this.surface.runKeymaps(key, 'highest')) return;`
    2. `if (this.vim.handleKey(key)) { key.preventDefault(); return; }`
    3. `if (!this.surface.runKeymaps(key, 'default')) this.keyboard.handle(key);`
- **Composition start.** In the `compositionstart` listener, if
  `this.vim && !this.vim.allowsTextInput()`:
  - set `this.discarding = true`;
  - call `this.surface.notifyCompositionStart()` (the popup closes);
  - call `this.vim.noteCompositionDiscarded()`;
  - **do not** call `beginComposition()`;
  - schedule `queueMicrotask(() => { if (this.discarding && !this.disposed) { this.accessibility.textarea.blur(); this.accessibility.focus(); } })`
    to end the platform composition early;
  - return.

  Otherwise, run the existing path.
- **During a discarded composition.**
  - `compositionupdate` is ignored.
  - `compositionend` sets `this.discarding = false`, sets
    `this.trailingComposition = true` (so trailing `insertFromComposition`
    input is dropped by the existing branches), calls
    `this.accessibility.refresh()`, and returns. It never dispatches, even
    if the hook now allows text.
  - `beforeinput` with `isComposing` or `insert*Composition*` types is
    `preventDefault`ed when cancelable, followed by a refresh.
  - The `input` event refreshes the bridge and returns.
  - A blur during discarding clears the flag and refreshes.
- **Text-input gate.** In `beforeInput`, after the `trailingComposition`
  branch and the `composing || event.isComposing` branch: if
  `this.vim && !this.vim.allowsTextInput()` and `kind` starts with
  `insert` or `delete`, then `preventDefault` when cancelable, call
  `this.accessibility.refresh()`, and return. `historyUndo`/`historyRedo`
  keep today's handling.

  In `input()`: if `this.vim && !this.vim.allowsTextInput()`, call
  `this.accessibility.refresh()` and return, before the diff
  reconciliation.
- **Replace overwrite.** In `beforeInput`'s `insertText` branch only (not
  line breaks), if `this.vim` is set, call `this.vim.overwriteRange(insert)`.
  When it returns a range, dispatch
  `{ changes: { from: range.from, to: range.to, insert }, selection: { anchor: range.from + insert.length }, userEvent: 'input.type' }`
  instead of `replaceSelection`. Then call `this.options.scrollCaret?.()`.
- **Clipboard.** The `paste`/`copy`/`cut` handling is unchanged in every
  mode (P3 in design 3.3.1).
- The file is 228 lines and must stay under 300.

### Tests: `editor/test/canvas/input-vim-hook.test.ts` (new)

Copy the `setup()`/`key()`/`before()`/`composition()` helpers pattern
from `test/canvas/input.test.ts` (do not import from it; it is not a
module export). Use a stub hook:

```ts
{ handleKey: vi.fn(() => consume), allowsTextInput: () => allow, overwriteRange: vi.fn(() => range), noteCompositionDiscarded: vi.fn() }
```

Required cases (at least 18):

- **No hook.** Typing via `beforeinput insertText`, `Ctrl-z` undo,
  `Tab`, the Japanese composition commit and `Mod-Enter` -> the same
  documents, revisions and `evalSelection` calls as without the hook code
  (assert the concrete values).
- **Hook consumes** `j` -> `defaultPrevented`, the document is unchanged,
  `KeyboardController` is not reached (Tab consumed -> no tab inserted).
- **Hook declines** `Ctrl-Enter` on platform `Linux x86_64` -> an eval
  binding registered on the surface at `'default'` runs. On platform
  `MacIntel`, `Meta-Enter` does the same. Stub with
  `Object.defineProperty(navigator, 'platform', { value, configurable: true })`
  and restore it in `afterEach`.
- **Highest before hook.** With a highest binding for `Escape` returning
  true, the hook's `handleKey` is never called.
- **Gate closed.** `beforeinput insertText` -> prevented, doc unchanged;
  `deleteContentBackward` -> prevented; `historyUndo` -> still undoes; a
  non-cancelable insert followed by `input` with a mutated textarea value
  -> doc unchanged and the textarea restored.
- **Discarded composition.** With the gate closed, `compositionstart`,
  then `compositionupdate('に')`, then `compositionend('日本')` -> doc and
  revision unchanged, `presentation.annotations` empty,
  `noteCompositionDiscarded` called once. The gate flips to open between
  update and end -> still discarded. After the microtask, the textarea was
  blurred and refocused (spy on `blur`/`focus`). A trailing
  `beforeinput insertFromComposition` -> prevented.
- **Gate open.** The existing Japanese commit assertions hold (one undo
  group, committed once).
- **Overwrite.** `overwriteRange` returns `{ from: 0, to: 1 }`, then
  `beforeinput insertText 'x'` on `'abc'` -> `'xbc'`, cursor 1.
  `insertLineBreak` ignores `overwriteRange`.
- **`matchesKeymap`.** It returns true without running the binding
  (the run spy is not called), for both precedences.
  `runKeymaps(event, 'default')` does not run highest bindings.

## Invariants

- With `setVim` never called, every existing `test/canvas/*.test.ts`,
  `test/code/*.test.ts` and `test/ui/*.test.ts` passes unchanged.
- `runKeymaps(event)` with one argument behaves exactly as before.

## Acceptance Criteria

- [x] `setVim`, the routing, the gate, discarding and overwrite are
      implemented as specified.
- [x] `matchesKeymap` exists, and the `runKeymaps` precedence filter
      exists.
- [x] `input-vim-hook.test.ts` has at least 18 passing cases, and
      `input.test.ts` passes unmodified.
- [x] Session-381 integration-review finding 1 is repaired: closed-gate
      composition remains discarded through Chromium blur/refocus and later
      composition events, with document and revision unchanged.
- [x] `input.ts` is under 300 lines and `surface.ts` under 220.

## Verification

1. A scoped typecheck per common rule 5 (`src/code/input.ts`,
   `src/code/surface.ts`, `test/canvas/input-vim-hook.test.ts`):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts`
   must exit 0 with 0 failed. Record the counts.
3. `git diff --stat -- test/canvas/input.test.ts` must show no change.

## Progress Log

### Session: 2026-10-16

**Tasks Completed**: All VIM-15 implementation and verification criteria.
**Notes**: Added the optional input hook, highest/hook/default routing, text-input gate, discarded composition handling, and insertText overwrite dispatch. Extended `CodeSurface` keymap matching/filtering while preserving the original unfiltered registry snapshot behavior. Added `input-vim-hook.test.ts` with 22 cases. Existing `input.test.ts` remains unmodified.

**Changed Source Hashes**:
- `editor/src/code/input.ts`: `af36c47b1ddf082838d7f95d033c8220e51fad0599dce5c2d7d29cc6ed818ca7`
- `editor/src/code/surface.ts`: `07f92699faed318ec41f29083e03bfcd0cbcc40086d5482bcd8e7ef29c42cdf7`
- `editor/test/canvas/input-vim-hook.test.ts`: `a90c39c37099a33eb921bd052df4b31f98e71ab437db002d311f509a0910b46d`

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json` — exit 0, no diagnostics; log `tmp/vim-mode/VIM-15/typecheck-final-02.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts` — exit 0, 6 files and 102/102 tests passed; log `tmp/vim-mode/VIM-15/focused-vitest-final-02.log`.
- `git diff --stat -- editor/test/canvas/input.test.ts` — empty, confirming the existing test is unchanged.
- Line counts: `input.ts` 270; `surface.ts` 179.

**Recovered Setup Issue**: The first typecheck invocation found no VIM-15 scoped `tsconfig.json`. Regenerated the verification-only scaffold per the VIM-00 common rule 5 and reran the required typecheck successfully.

### Session: 2026-10-17 Step 6 rerun

**Tasks Completed**: Revalidated the VIM-15 source and behavioral gates against the current shared tree; no source changes were needed.
**Notes**: The VIM-15 input hook, highest/hook/default routing, non-insert text gate, IME discard path, overwrite behavior, and keymap query remain unchanged from the recorded source identities. The integration tests include Linux `Ctrl-Enter` and Mac `Meta-Enter` default-keymap routing through `InputController`, addressing this plan's non-Apple routing seam for the prior design finding. Other pass-through normalization and Vim-mode behavior remain in their owning VIM-00/VIM-20/VIM-30 plans.

**Changed Source Hashes**: Unchanged from the 2026-10-16 entry; current hashes recorded in `tmp/vim-mode/VIM-15/source-hashes.log`.

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json` — exit 0, no diagnostics; log `tmp/vim-mode/VIM-15/typecheck-step6-rerun.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts` — exit 0, 6 files and 102/102 tests passed; log `tmp/vim-mode/VIM-15/focused-vitest-step6-rerun.log`.
- `git diff --stat -- editor/test/canvas/input.test.ts` — empty; the existing test remains unmodified.
- Line counts remain `input.ts` 270 and `surface.ts` 179. The hook test contains 22 cases.

### Session: 2026-10-17 Step 6 IME repair rerun

**Tasks Completed**: Repaired the session-381 Chromium normal-mode IME finding and reran VIM-15 verification on the repaired source.
**Notes**: The discard latch now survives the blur/refocus termination attempt until `compositionend`; composing `beforeinput`, direct `beginComposition`/`updateComposition`/`endComposition`, and composition event paths check the Vim gate. If the gate closes during an active composition, it is finished without dispatch. Added a regression replay after blur/refocus, including the gate opening before `compositionend`; document and revision remain unchanged and no composition annotations appear. Existing `input.test.ts` remains untouched.

**Changed Source Hashes**:
- `editor/src/code/input.ts`: `f77078bccde921f3fc64aa47442a0413703ba4ebb436c61c2294a599d171c8f2`
- `editor/src/code/surface.ts`: `07f92699faed318ec41f29083e03bfcd0cbcc40086d5482bcd8e7ef29c42cdf7` (unchanged)
- `editor/test/canvas/input-vim-hook.test.ts`: `7be73c00252eb96d6be4d361f34bfa2690f900a84c7ceea41ff5c1e917f9998a`

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json` — exit 0; complete log `tmp/vim-mode/VIM-15/typecheck-step6-ime-repair.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts` — exit 0, 6 files and 103/103 tests passed; complete log `tmp/vim-mode/VIM-15/focused-vitest-step6-ime-repair.log`.
- `git diff --stat -- editor/test/canvas/input.test.ts` — empty; the existing test remains unmodified.
- Hook test case count: 23. Line counts: `input.ts` 290; `surface.ts` 179.

**Final-Source Follow-Up**: The self-check also canceled a composing `beforeinput` that reaches the gate-closed branch before the discard latch is set. Final `editor/src/code/input.ts` SHA-256: `68e94bb21ab210a2d995ad52ccf77d7bc86e353baac303e269360dafbb74a069`.
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json` — exit 0; complete final-source log `tmp/vim-mode/VIM-15/typecheck-step6-ime-repair-02.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts` — exit 0, 6 files and 103/103 tests passed; complete final-source log `tmp/vim-mode/VIM-15/focused-vitest-step6-ime-repair-02.log`.

### Session: 2026-10-17 Step 6 current-source revalidation

**Tasks Completed**: Revalidated the assigned VIM-15 contract on the current shared tree; no source or test edits were needed.
**Notes**: Current `input.ts`, `surface.ts`, and hook-test hashes match the `before:step6-implement` source snapshot. The closed-gate regression replays Chromium's discard blur/refocus sequence, later composition events, composing `beforeinput`, direct update/end calls, and a gate opening before `compositionend`; document and revision stay unchanged. `input.test.ts` remains unchanged. The hook test has 23 cases; `input.ts` is 290 lines and `surface.ts` is 179 lines.

**Changed Source Hashes**:
- `editor/src/code/input.ts`: `68e94bb21ab210a2d995ad52ccf77d7bc86e353baac303e269360dafbb74a069` (matches the pre-step snapshot)
- `editor/src/code/surface.ts`: `07f92699faed318ec41f29083e03bfcd0cbcc40086d5482bcd8e7ef29c42cdf7` (matches the pre-step snapshot)
- `editor/test/canvas/input-vim-hook.test.ts`: `7be73c00252eb96d6be4d361f34bfa2690f900a84c7ceea41ff5c1e917f9998a` (matches the pre-step snapshot)

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json` — exit 0; complete log `tmp/vim-mode/VIM-15/typecheck-step6-current-02.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts` — exit 0, 6 files and 103/103 tests passed; complete log `tmp/vim-mode/VIM-15/focused-vitest-step6-current.log`.
- `git diff --stat -- editor/test/canvas/input.test.ts` — empty; existing input tests remain unmodified.
- Line counts: `input.ts` 290; `surface.ts` 179. Hook test cases: 23.

### Session: 2026-10-17 Step 6 dispatch revalidation

**Tasks Completed**: Re-ran the assigned VIM-15 typecheck and focused behavioral suite against the current source after the session-381 IME repair; no implementation edits were needed.
**Notes**: The closed-gate discard latch remains active through the browser blur/refocus attempt until `compositionend`; the regression exercises later composition events, composing `beforeinput`, direct composition updates, and a gate opening before `compositionend`, while asserting unchanged document and revision. `input.test.ts` remains unmodified. The VIM-30 evidence finding remains assigned to downstream VIM-30 and integration review.

**Current Source Hashes**:
- `editor/src/code/input.ts`: `68e94bb21ab210a2d995ad52ccf77d7bc86e353baac303e269360dafbb74a069`
- `editor/src/code/surface.ts`: `07f92699faed318ec41f29083e03bfcd0cbcc40086d5482bcd8e7ef29c42cdf7`
- `editor/test/canvas/input-vim-hook.test.ts`: `7be73c00252eb96d6be4d361f34bfa2690f900a84c7ceea41ff5c1e917f9998a`

**Verification**:
- `cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-15/tsconfig.json` — exit 0; complete log `tmp/vim-mode/VIM-15/typecheck-step6-dispatch-20261017-rerun.log`.
- `cd editor && ./node_modules/.bin/vitest run test/canvas/input-vim-hook.test.ts test/canvas/input.test.ts test/canvas/state.test.ts test/code/completion-view.test.ts test/code/eval.test.ts test/code/format.test.ts` — exit 0, 6 files and 103/103 tests passed; complete log `tmp/vim-mode/VIM-15/focused-vitest-step6-dispatch-20261017.log`.
- `git diff --stat -- editor/test/canvas/input.test.ts` — empty; the existing input tests remain unmodified.
- Line counts remain `input.ts` 290 and `surface.ts` 179. The hook test contains 23 cases.
