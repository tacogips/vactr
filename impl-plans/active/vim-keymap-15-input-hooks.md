# Vim Keymap: InputController Hooks (typeText, defaultKey, flushPendingInput) Implementation Plan

**Status**: Ready
**Plan ID**: VK-15 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 4.2 (forced flush), 4.4, 4.4.1
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`VimInputHook.flushPendingInput?`)
- **Parallel (wave 1)**: VK-10, VK-11, VK-12, VK-13, VK-14, VK-16
- **Consumer**: VK-30 implements `VimHost.typeText`/`defaultKey` with these methods
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

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

- [ ] `typeText`, `defaultKey` public; flush call sites added; file under 330 lines
- [ ] At least 8 new passing cases; existing input tests unchanged and passing
- [ ] Progress Log entry added

## Progress Log

(none yet)
