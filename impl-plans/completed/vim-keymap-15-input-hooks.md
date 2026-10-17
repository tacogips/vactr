# Vim Keymap: InputController Hooks (typeText, defaultKey, flushPendingInput) Implementation Plan

**Status**: Completed
**Plan ID**: VK-15 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 4.2 (forced flush), 4.4, 4.4.1
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`VimInputHook.flushPendingInput?`)
- **Parallel (wave 1)**: VK-10, VK-11, VK-12, VK-13, VK-14, VK-16
- **Consumer**: VK-30 implements `VimHost.typeText`/`defaultKey` with these methods
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

Insert-mode mappings such as `kj` hold the first key (deferred insert)
and later insert it as typed text. That text must go through the same
path as an `insertText` `beforeinput` (including replace-mode overwrite)
so that the insert session, dot repeat, macros and the completion trigger
see `input.type`. An `rhs` may also emit `<BS> <Del> <CR> <Tab>` and
movement keys, which must use the existing `KeyboardController`
behavior. Before IME composition or native text input, a pending mapper
buffer must be flushed so the document order stays right.

## Non-Goals

- No change to the composition guard, discard latch, clipboard handling
  or the key routing order (`design-vim-mode.md` 3.3, 3.5).
- No Vim logic in `input.ts`.

## File-Level Changes

### editor/src/code/input.ts (budget: under 330 lines)

- New public `typeText(text: string): void`: if composing, return; else
  the same branch as the `insertText` `beforeinput` path: call
  `this.vim?.overwriteRange(text)` and dispatch the overwrite with
  `userEvent: 'input.type'` when it returns a range, else
  `this.replaceSelection(text, 'input.type')`. Refactor that branch of
  `beforeInput` into one private helper used by both, with identical
  behavior.
- New public `defaultKey(key: string): boolean`: map
  `<BS>`->`Backspace`, `<Del>`->`Delete`, `<CR>`->`Enter`, `<Tab>`->`Tab`,
  `<Left>`->`ArrowLeft`, `<Right>`->`ArrowRight`, `<Up>`->`ArrowUp`,
  `<Down>`->`ArrowDown`, `<Home>`->`Home`, `<End>`->`End`; other keys
  return false. Build a `KeyboardEvent('keydown', { key, cancelable: true })`
  with the container document's window `KeyboardEvent` constructor, run
  `this.surface.runKeymaps(event, 'default')` and, if that did not
  handle it, `this.keyboard.handle(event)`; return true.
- Call `this.vim?.flushPendingInput?.()` (a) at the start of the
  `compositionstart` handling, before `beginComposition` state changes,
  and (b) at the start of `beforeInput` and of the fallback `input`
  handler, before any selection is read. Only when not composing.

Pitfalls: do not call the flush inside a surface subscriber; do not
change the order of existing checks; `typeText` must not run while
composing; keep `historyUndo`/`historyRedo` handling unchanged.

## Invariants

- With no hook or a hook without `flushPendingInput`, every existing test
  in `editor/test/canvas/input.test.ts`, `input-vim-hook.test.ts` and
  `vim-input.test.ts` passes unchanged.

## Test Cases (`editor/test/canvas/input-keymap-hooks.test.ts`, imitate the setup of `input-vim-hook.test.ts`)

- `typeText('k')` with no hook -> doc gains `k` at the selection, transaction userEvent `input.type`
- `typeText('k')` with a hook whose `overwriteRange` returns `{from, to}` -> that range replaced
- `typeText` during a composition -> no change
- `defaultKey('<BS>')` on `ab|` -> `a|`; `defaultKey('<CR>')` inserts a newline; `defaultKey('<F1>')` -> false, no change
- hook with `flushPendingInput` spy: `compositionstart` -> spy called before the composition starts; a cancelable `beforeinput insertText` -> spy called before the insert; spy not called on a plain keydown
- flush that calls `typeText('k')` from inside `flushPendingInput` on `compositionstart`, then the composition commits `日本` -> doc `k日本` (order kept), composition committed once

## Verification (from editor/)

1. Scoped typecheck including `src/code/input.ts` and the new test -> exit 0
2. `./node_modules/.bin/vitest run test/canvas/input-keymap-hooks.test.ts test/canvas/input.test.ts test/canvas/input-vim-hook.test.ts test/canvas/vim-input.test.ts` -> exit 0, failureCount 0

## Completion Criteria

- [x] `typeText`, `defaultKey` public; flush call sites added; file under 330 lines
- [x] At least 8 new passing cases; existing input tests unchanged and passing
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 — VK-15 implementation

**Tasks Completed**: VK-15 implementation and plan-scoped verification.

**Changes**:
- Added public `InputController.typeText` using the shared insert-text/overwrite path, with a composition guard.
- Added public `defaultKey` for the ten contracted Vim notation keys, routed through default keymaps and `KeyboardController`; synthetic Enter falls back to the existing text insertion path because synthetic keydown does not produce browser `beforeinput`.
- Flushed pending Vim input before compositionstart, beforeinput and fallback input, while honoring controller and platform composition state. When a fallback input flush changes the document, apply the captured native text mutation at the post-flush selection to preserve event order.
- Added 12 cases in `input-keymap-hooks.test.ts`. Existing `input.test.ts`, `input-vim-hook.test.ts` and `vim-input.test.ts` are unchanged.

**Source Hashes (git blob)**:
- `editor/src/code/input.ts`: `7745ad239b1a08220368dfd41cb455cb786caa48` (329 lines)
- `editor/test/canvas/input-keymap-hooks.test.ts`: `789616aef0ef00c8622de6c5bdcaea422758c1c5`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-15/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-15/attempt-5/typecheck.log", notes: "Scoped to InputController and the new input keymap hook suite."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/input-keymap-hooks.test.ts test/canvas/input.test.ts test/canvas/input-vim-hook.test.ts test/canvas/vim-input.test.ts", exitStatus: 0, testsRun: 100, testsPassed: 100, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-15/attempt-5/vitest.log", notes: "4 files passed, including 12 new hook cases; existing input test files remain unchanged."}`
- `{command: "cd editor && git diff --check -- src/code/input.ts test/canvas/input-keymap-hooks.test.ts", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-15/attempt-5/diff-check.log", notes: "No whitespace errors in owned source/test paths."}`
- `wc -l editor/src/code/input.ts` — 329 lines; existing input test files remain byte-identical to HEAD.

**Resolved During Implementation**:
- Prior failed run `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/input-keymap-hooks.test.ts test/canvas/input.test.ts test/canvas/input-vim-hook.test.ts test/canvas/vim-input.test.ts", exitStatus: 1, testsRun: 98, testsPassed: 97, failureCount: 1, outcome: "failed-resolved", log: "tmp/vim-keymap/VK-15/attempt-1/vitest.log", notes: "Test caret started at document position 0; the Backspace case now places it at the end."}`.
- Prior failed run `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/input-keymap-hooks.test.ts test/canvas/input.test.ts test/canvas/input-vim-hook.test.ts test/canvas/vim-input.test.ts", exitStatus: 1, testsRun: 100, testsPassed: 99, failureCount: 1, outcome: "failed-resolved", log: "tmp/vim-keymap/VK-15/attempt-2/vitest.log", notes: "Synthetic keydown does not generate browser beforeinput for Enter; defaultKey now applies the newline fallback when keymaps and KeyboardController decline it."}`.
- Prior failed run `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/input-keymap-hooks.test.ts", exitStatus: 1, testsRun: 12, testsPassed: 11, failureCount: 1, outcome: "failed-resolved", log: "tmp/vim-keymap/VK-15/attempt-3/fallback-probe.log", notes: "Reproduced native fallback insertion preceding deferred text (`axkb`); fallback reconciliation now applies it at the post-flush selection."}`.
- Prior failed run `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/input-keymap-hooks.test.ts test/canvas/input.test.ts test/canvas/input-vim-hook.test.ts test/canvas/vim-input.test.ts", exitStatus: 1, testsRun: 100, testsPassed: 99, failureCount: 1, outcome: "failed-resolved", log: "tmp/vim-keymap/VK-15/attempt-4/vitest.log", notes: "Same strengthened native fallback ordering regression as the focused probe; fixed before attempt 5."}`.

**Pending**: Independent test-integrity/adversarial review and serial combined-tree integration/finalization remain downstream workflow steps.
