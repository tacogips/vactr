# Vim Mode: Redo Records, Macros, Undo Ledger and Marks Implementation Plan

**Status**: Ready
**Plan ID**: VIM-14 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 4.9, 4.10, 4.11
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00
- **Parallel (wave 1)**: VIM-10, VIM-11, VIM-12, VIM-13, VIM-15, VIM-16, VIM-17, VIM-18
- **Consumer**: VIM-20. It decides when units begin and end, and when records start, stop and replay.

## Intent and Context

This plan adds three small pure modules. Their exports are pinned in
`vim-mode-00-contract.md`:

- `editor/src/code/vim/undo.ts`: the undo-unit ledger over the existing
  CodeMirror history (design 4.9). It uses only `undoDepth`/`redoDepth`
  from `@codemirror/commands`; history internals are never read.
- `editor/src/code/vim/record.ts`: the dot-repeat redo record, the macro
  recorder, and token/text conversion (design 4.10).
- `editor/src/code/vim/marks.ts`: the mark table mapped through changes
  (design 4.11).

Background that was verified for the design: in `@codemirror/commands`
6.11.1, `HistoryState.addChanges` joins only joinable user events, within
500 ms, when adjacent, and when there are no `selectionsAfter`. The
`undo`/`redo` commands dispatch transactions with `userEvent` `'undo'`
and `'redo'`. The common execution rules in `vim-mode-00-contract.md`
apply.

## Non-Goals

- No replay execution. VIM-20 feeds tokens back through its own
  interpreter.
- No boundary-transaction dispatch. VIM-20 dispatches the
  `isolateHistory('full')` boundaries.
- No editing of `surface.ts`, `history.ts` or `@codemirror/commands`
  internals.

## File-Level Changes

### `editor/src/code/vim/undo.ts` (new, at most 160 lines)

`UndoLedger` keeps two stacks: `done: { entryDepth; exitDepth; exitDoc }[]`
and `undone: { redoEntry; redoExit; doc; unit }[]`.

- `record(unit)`: ignore it when `exitDepth <= entryDepth` (no history
  event was made). Otherwise push it to `done`.
- `undo(host)`: let `s = host.state()` and `d = undoDepth(s)`, with
  `top = done.at(-1)`.
  - If `top` exists, `top.exitDepth === d`, and
    `(s.doc === top.exitDoc || s.doc.eq(top.exitDoc))`: let
    `r0 = redoDepth(s)`, then call `host.undo()` up to
    `top.exitDepth - top.entryDepth` times, stopping at the first false.
    Pop `top` and push
    `{ redoEntry: r0, redoExit: redoDepth(host.state()), doc: host.state().doc, unit: top }`
    to `undone`.
  - Otherwise call `host.undo()` once. This is the fallback for external
    edits and for a stale ledger.
- `redo(host)`: the mirror image, using `redoDepth` and the `undone`
  stack. On a ledger redo, push the unit back to `done` with
  `exitDoc = host.state().doc` and `exitDepth = undoDepth(host.state())`.
  Otherwise call `host.redo()` once.
- `noteChange(fromHistory)`: when `fromHistory` is false, clear `undone`.
  The controller calls it for every `docChanged` update, with
  `fromHistory = userEvent === 'undo' || userEvent === 'redo'`.
- `clear()`: empties both stacks. The controller calls it when
  `host.historyReduced()` becomes true.

### `editor/src/code/vim/record.ts` (new, at most 300 lines)

- **`editToken(update, cursorBefore)`**:
  - Only a `docChanged` update whose `changes` has exactly one changed
    range `[fromA, toA)` -> `[fromB, toB)` with
    `fromA <= cursorBefore <= toA` produces a token:
    `{ kind: 'edit', deleteBefore: cursorBefore - fromA, deleteAfter: toA - cursorBefore, insert: update.state.doc.sliceString(fromB, toB) }`.
  - Anything else (several ranges, or a change not touching the cursor)
    returns null, which the controller treats as "close the record".
- **`RedoRecorder`**:
  - `begin(first)` starts a new pending record seeded with `first`, for
    example the key tokens `c`, `w`. `push` appends only while active.
  - `end()` promotes the pending record to `last()`.
  - `abort()` drops the pending record and keeps the previous `last()`.
    The controller uses it when a change command fails.
  - A `begin` while already active implicitly calls `end()` first.
- **`MacroRecorder`**:
  - `start(reg)` begins recording. While recording, `push` appends.
  - `stop()` returns `{ reg, tokens }` and drops a trailing `q` key token
    (the stop key), or returns null if it was not recording.
  - `register` is the register being recorded, or null.
- **`tokensToText(tokens)`**:
  - Key tokens become their key text (named keys stay as `<Name>`).
  - Edit tokens become `'<BS>'.repeat(deleteBefore) + '<Del>'.repeat(deleteAfter) + insert`.
  - Move tokens become `h`/`j`/`k`/`l`/`0`/`$` for
    `h`/`j`/`k`/`l`/`0`/`$`.
- **`textToTokens(text)`**: splits text into key tokens. A `<Name>` run
  that matches a known `VimKey` name becomes one key; every other
  character (grapheme) is one key. Line breaks become `<CR>`.
- These are key tokens. When they are replayed in insert mode, the
  controller inserts printable keys as text. This is how a yanked register
  is run with `@a`.

### `editor/src/code/vim/marks.ts` (new, at most 100 lines)

- `isMarkName(ch)`: `/^[a-z]$/`.
- `MarkTable`:
  - `set` and `get` store positions; `get` returns null for an unset mark.
  - `setContext` and `context` store the `''` context mark.
  - `map(changes)` maps every stored position, including the context mark,
    with `changes.mapPos(pos, -1)`.

### Tests

**`editor/test/code/vim/undo.test.ts`** (at least 10 cases). Use
`createTestHost` from `test/code/vim/host.ts` with a real `history()`.
Simulate long sessions with `Transaction.time.of(...)` annotations so
that events split.

- Three typing transactions 600 ms apart (3 history events) are recorded
  as one unit -> one `undo` restores the entry doc; `redo` re-applies all
  three.
- An external single edit after the unit (depth mismatch) -> `undo` undoes
  only that edit; the next `undo` undoes the whole unit, using the
  content-equality guard.
- `noteChange(false)` clears redo, so a later `redo` falls back to a single
  `host.redo()`.
- `clear()` -> single-step behavior.
- A unit with `exitDepth === entryDepth` is ignored.
- An `undo` that returns false early stops the loop without throwing.

**`editor/test/code/vim/record.test.ts`** (at least 12 cases):

- `editToken` for a typed char, a Backspace (`deleteBefore: 1`), a Delete,
  a completion that replaces a prefix (`deleteBefore: 3`, `insert: 'kick'`),
  a change away from the cursor -> null, and two ranges -> null.
- `RedoRecorder` begin/push/end/abort semantics.
- `MacroRecorder` stop drops the trailing `q`.
- `tokensToText` round trip with `textToTokens` for keys, and the edit
  encoding.

**`editor/test/code/vim/marks.test.ts`** (at least 6 cases): set and get;
mapping through an insert before and after a mark; deleting a range that
contains a mark maps to the deletion start; the context mark; an unset mark
-> null.

## Pitfalls

- Compare depths with `undoDepth`/`redoDepth` of the **current** state at
  call time. Do not cache them.
- `Text.eq` is content equality. Use it only together with the depth match,
  as specified.
- Do not mutate token arrays after `end()`. Store copies.

## Acceptance Criteria

- [ ] The pinned exports exist with those signatures.
- [ ] At least 28 tests pass across the three files.
- [ ] Each file is within budget, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (the 6 files):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-14/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/undo.test.ts test/code/vim/record.test.ts test/code/vim/marks.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 28 tests and 0 failed.

## Progress Log

### Session: (not started)
