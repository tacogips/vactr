# Vim Mode: VimController (mode machine and command execution) Implementation Plan

**Status**: Completed (accepted in workflow session 391; archived 2026-10-17)
**Plan ID**: VIM-20 (wave 2)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 3.3.1-3.3.3, 3.4, 3.5 (hook side), 4.1-4.14, 5, 7
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Related Plans

- **Depends On**: VIM-00, VIM-10, VIM-11, VIM-12, VIM-13, VIM-14, VIM-18 (all accepted)
- **Next**: VIM-30 (mount wiring), which also depends on VIM-15, VIM-16 and VIM-17

## Intent and Context

`VimController` composes the pure modules into the Vim mode machine and
implements `VimInputHook` (from `vim/types.ts`) for `InputController`. It
is pure: it talks only to `VimHost`. It is the single place that:

- decides mode transitions;
- dispatches with `isolateHistory.of('full')` and `userEvent: 'vim.<cmd>'`;
- owns insert sessions, undo units, dot and macro recording, registers,
  marks, search state, the clipboard calls, ex execution and the snapshot.

Read every wave-0/1 module's exports first (the signatures are pinned in
`vim-mode-00-contract.md`, `vim-mode-13-search-ex.md` (`ExCommand`) and
`vim-mode-18-parser.md` (`ParsedCommand`)). Do not reimplement them. If a
module bug blocks this plan, fix it in place, list the file in the
Progress Log as a cross-plan repair, and add a regression test in that
module's test file. Those module and test files are in this plan's
`sharedPaths`. The common execution rules in `vim-mode-00-contract.md`
apply, except that this wave runs alone, so use the full `npm run check`.

## Non-Goals

- No DOM, no `mount.ts` and no `input.ts` (VIM-15 and VIM-30).
- No hlsearch, visual block or the other VQ4 exclusions.
- Do not change pinned module signatures. Add new exports only if
  unavoidable, and record them.

## File-Level Changes

The new files are `editor/src/code/vim/controller.ts` (at most 600
lines), `editor/src/code/vim/execute.ts` (command execution helpers, at
most 600 lines) and `editor/src/code/vim/session.ts` (insert/replace
sessions and replay, at most 400 lines). Split differently only if a file
would exceed its budget.

### Public API (`controller.ts`)

```ts
export class VimController implements VimInputHook {
  constructor(host: VimHost);
  get snapshot(): VimSnapshot;
  subscribe(cb: (snapshot: VimSnapshot) => void): () => void;
  handleKey(input: VimKeyInput): boolean;
  allowsTextInput(): boolean;
  overwriteRange(text: string): { from: number; to: number } | null;
  noteCompositionDiscarded(): void;
  submitPrompt(text: string): void;
  cancelPrompt(): void;
  tapMode(): void;
  dispose(): void;
}
```

### `handleKey(input)` order (design 3.3)

1. If `host.compositionActive()`, return false.
2. If `isPassThrough(input, host.platform, host.matchesDefaultKeymap)`,
   return false in every mode. This covers P1-P3. The parser's pending
   state is kept, so `d`, `Mod-Enter`, `w` still deletes a word.
3. `key = normalizeKey(input)`. If it is null (a bare modifier), return
   false.
4. **Insert and replace modes.**
   - `<Esc>`: if the key was `Ctrl-[` and `host.closePopupWithEscape()`
     returns true, return true and stay; no token is recorded, because
     only the popup closed. Otherwise the session really exits:
     1. push `{ kind: 'key', key: '<Esc>' }` to the macro recorder (if
        recording) and to the active redo record;
     2. run Exit (below);
     3. return true.
   - Replace mode `<BS>`: restore (below) and return true.
   - `<Left>`, `<Right>`, `<Up>`, `<Down>`, `<Home>`, `<End>`: push a
     `move` token to the macro recorder, close the redo record (end the
     record; the next edit starts a new record with an implicit `i`), and
     return false. `KeyboardController` moves the caret.
   - Everything else: return false.
5. **Non-insert modes** (always return true; the key is consumed):
   - `<Esc>` from `Ctrl-[`: if `host.closePopupWithEscape()` returns true,
     stop.
   - If recording, `parser.pending() === ''` and `key === 'q'`: stop
     recording and store the macro in the register. Do not push the `q`.
   - If recording, push `{ kind: 'key', key }` to the macro recorder.
   - Then `parser.feed(key, mode)`:
     - `command` -> `execute`;
     - `invalid` -> clear pending; if it has a message, set it as an error;
     - `pending` -> update the snapshot (operator-pending when
       `parser.operatorPending()`).

### Execution rules (`execute.ts`)

- **Cursor.** In normal mode, the cursor is
  `clampNormal(doc, selection.main.head)`. Never rewrite the selection to
  clamp it (design 4.3).
- **Motions** (no operator):
  - Call `runMotion` with
    `{ count: cmd.count ?? 1, explicitCount: cmd.count !== null, desiredColumn, viewport: host.viewport(), lastFind }`.
  - On null: no change and no message.
  - Otherwise: when `result.scrollLines` is set, call
    `host.scrollLines(result.scrollLines)` first; when `jump` is true, call
    `marks.setContext(oldCursor)`.
  - Dispatch the selection (normal mode `{ anchor: pos }`; visual mode
    updates the head, see below) with `userEvent: 'select.vim'`, then call
    `host.scrollCaret()`. Update `desiredColumn` and `lastFind`.
- **Search targets.**
  - `n`/`N` use `lastSearch { regex, direction }` (with none:
    `E35 No previous regular expression`).
  - `*`/`#` build `wordPattern(wordUnderCursor.text)` and set
    `lastSearch`/`snapshot.search`.
  - `findMatch` from the cursor: null gives `E486 Pattern not found: <src>`;
    `wrapped` gives the info message
    `search hit BOTTOM, continuing at TOP` (or `TOP`/`BOTTOM` reversed).
  - The result is an exclusive charwise jump.
- **Mark targets.** `marks.get(name)`, or `marks.context()` for `'`/`` ` ``;
  null gives `E20 Mark not set`. `'` is linewise to the first non-blank,
  and `` ` `` is exclusive. Both are jumps.
- **Operators.**
  - Compute the range from the target: `motionRange` for motions, search
    and marks; `selectTextObject` for objects; `line` covers the cursor
    line to `+count-1`. In visual mode, use the selection (inclusive
    charwise, or linewise for visual-line or a `line` target).
  - `c` with `w`/`W` on a non-blank uses `e`/`E` (Vim's `cw`).
  - An empty charwise range (for example `x` on an empty line) is a
    no-op: no dispatch and no register write.
  - `applyOperator`, then write the register with
    `registers.write(cmd.register, yanked, op === 'y' ? 'yank' : 'delete')`
    unless `yanked` is null. For `+`/`*`, also call
    `host.clipboard.write(text)`; on rejection, set the assertive message
    `clipboard unavailable; kept in the internal register`.
  - Dispatch
    `{ changes, selection: { anchor: cursor }, annotations: isolateHistory.of('full'), userEvent: 'vim.' + name }`,
    with `name` one of `delete change yank indent outdent case`.
  - A linewise `y` with `cursor === -1` uses the rule in
    `vim-mode-12-operators-registers.md`.
  - `enterInsert` starts an insert session whose `entryDepth` was read
    **before** this dispatch.
- **Actions.**
  - `insert`, `append`, `insert-line-start`, `append-line-end`:
    - `append` moves to `nextGrapheme` unless the line is empty;
    - `append-line-end` moves to `line.to`;
    - `insert-line-start` moves to `firstNonBlank`.
  - `open-below`/`open-above`: `openLine`, dispatched inside the session
    (the entry depth is taken before it).
  - `replace-mode`: a replace session.
  - `visual`/`visual-line`: toggle per design 4.1.
  - `visual-swap`: swap the anchor and head.
  - `put-after`/`put-before`: `putRegister` with the register
    `cmd.register ?? '"'`.
    - A `+`/`*` register outside replay reads asynchronously. Capture
      `doc` (Text identity), `selection` and `mode`; on resolve, apply only
      if all three are unchanged, and also `setMirror`. Otherwise set the
      message `clipboard paste dropped: document changed`. On rejection,
      use the mirror and the clipboard-unavailable message.
    - During replay, use the mirror synchronously.
    - The resolved text is pushed into the active redo record as an edit
      token, so `.` repeats the same text.
  - `replace-char`, `toggle-case`, `join`: the VIM-12 builders. A null
    result is a failed command (abort the redo record).
  - `undo`/`redo`: `ledger.undo(host)` or `ledger.redo(host)`, count
    times.
  - `repeat`: replay `redo.last()` (below).
  - `record`: `macro.start(arg)`, and `snapshot.recording = arg`.
  - `play`: tokens from `registers.read(arg)?.tokens` or
    `textToTokens(text)`. `@@` uses the last played register; with none,
    `E748 No previously used register`. Play `count` times.
  - `set-mark`: `marks.set(arg, cursor)`.
  - `prompt`:
    - from visual: store `lastVisual` lines, collapse to normal, and set
      the prompt `{ kind: ':', initial: "'<,'>" }`;
    - otherwise: `{ kind: arg, initial: '' }`.
  - `escape`: visual -> normal, collapsing the selection to the head;
    normal -> clear pending and message.
- **Visual mode.**
  - Keep `visual = { anchor, head }` as inclusive grapheme starts. Motions
    move `head`.
  - Dispatch the CodeMirror selection per design 4.3, with
    `userEvent: 'select.vim'`. Visual-line uses whole lines.
  - Store `lastVisual = { fromLine, toLine }` when leaving visual.
  - A text object in visual sets the anchor and head to the object's
    range.
- **Redo record (dot).**
  - Changes executed from **normal mode**
    (`d c > < g~ gu gU x X s S D C r ~ J p P o O i a I A R`) call
    `redo.begin(tokens)`. The tokens are the command's keys **without**
    its leading count; store that count separately as `repeatCount`.
  - Commands that do not enter insert call `redo.end()` right after
    dispatch.
  - `y`, motions, undo/redo, marks, prompts and macros never touch the
    record.
  - **Visual-mode change rule.** A change executed from visual or
    visual-line mode (an operator with `target: null`, or the visual forms
    of `D X C S R Y J r ~ p P`) does **not** begin a redo record and leaves
    `redo.last()` unchanged. This is minimal: Vim's same-size-region repeat
    is not required by design 4.10. A visual `c` that enters insert also
    does not record that session for dot. As a result, `.` after a visual
    change repeats the previous normal-mode change, or does nothing if
    there is none, and it can never replay a bare operator key into a
    pending state.
  - `.` with count N replays with N as the count prefix, or with
    `repeatCount`.
  - The whole replay is one undo unit (`ledger.record` around it), using
    `userEvent: 'vim.repeat'` for edit tokens. Recording is suppressed
    during dot replay.

### Sessions (`session.ts`)

- **Start** (insert or replace):
  1. Read `entryDepth = undoDepth(host.state())` before any session
     dispatch.
  2. Dispatch a boundary
     `{ selection: <current>, annotations: isolateHistory.of('full'), userEvent: 'vim.insert' }`.
  3. Set the mode and announce the label.
- **While active**, the controller's `host.subscribe` handler:
  - computes `editToken(update, lastHead)` for `docChanged` updates whose
    `userEvent` starts with `input.type`, `delete`, `input.paste` or
    `input.complete`;
  - on a token, pushes it to the redo record (if no record is active,
    begins one with `[{ kind: 'key', key: 'i' }]`) and to the macro
    recorder;
  - on null, ends the redo record.

  `lastHead` is the head before the update. Keep it updated after every
  update.
- **Exit:**
  1. If the session count is greater than 1, re-apply the session's edit
     tokens `count - 1` times at the cursor.
  2. Dispatch a boundary transaction.
  3. `ledger.record({ entryDepth, exitDepth: undoDepth(state), exitDoc: state.doc })`.
     During a dot replay this is skipped; the outer replay unit covers it.
  4. Close the `<Esc>` token, then `redo.end()`:
     - a key-driven exit has already pushed the `<Esc>` token (handleKey
       step 4);
     - an exit not caused by a typed key (`tapMode()`) pushes the same
       `{ kind: 'key', key: '<Esc>' }` token to the macro recorder (if
       recording) and the active redo record before `redo.end()`, so
       records and macros stay well-formed;
     - during replay, recording is suppressed, so nothing is pushed.
  5. Move the cursor one grapheme left, but not past the line start.
  6. Set normal mode and announce.
- **Replace.**
  - `overwriteRange(text)` (replace mode only, otherwise null) returns the
    next `graphemeCount(text)` graphemes on the cursor line, bounded by
    `line.to`, and pushes the overwritten originals (possibly `''` at the
    line end) to a stack.
  - `<BS>` pops one original and dispatches a restore of the previous
    grapheme (`userEvent: 'vim.replace-restore'`), or moves left only if
    the stack is empty.
- `allowsTextInput()`: true only in insert and replace.
- `noteCompositionDiscarded()`: sets the message
  `IME input ignored in <LABEL> mode` and announces it once per call.
- **Replay** (dot and macro):
  - Count the depth and the tokens against `REPLAY_DEPTH_LIMIT` and
    `REPLAY_TOKEN_LIMIT`. Exceeding either stops with the error
    `E169 Command too recursive`.
  - `key` tokens in non-insert modes go through the same path as step 5
    (with no pass-through check).
  - In insert or replace mode:
    - a printable key inserts its text at the cursor
      (`userEvent: 'vim.repeat'`);
    - `<CR>` inserts `'\n'`, and `<BS>`/`<Del>` delete one grapheme;
    - `<Esc>` runs the full Exit path above, including the session-count
      re-apply (`3ihi<Esc>` replays as `hihihi`) and the cursor step left,
      and returns to normal mode. The following key tokens (for example the
      `j` after `A;<Esc>` in a macro) are then interpreted as normal-mode
      commands, never as text;
    - `edit` tokens apply
      `{ from: cur - deleteBefore, to: cur + deleteAfter, insert }`,
      clamped to the document;
    - `move` tokens move as `KeyboardController` would (by grapheme or
      column; `$` goes to `line.to`).
  - Stop at the first failing motion or command.
  - Defensive end-of-replay rule: when a **dot** replay finishes (normally
    or after a stop) while an insert or replace session it started is still
    active (for example a truncated record), exit that session through the
    same Exit path. When the parser is left pending, reset it. A `.`
    therefore always ends in normal mode with `pending === ''`.

    A macro replay that ends inside an insert session keeps that session
    open, because Vim does the same and the user's macro chose it. The
    parser is still reset if it is left pending.

### Subscription handler (every mode)

- If `docChanged`, call `marks.map(update.changes)` and
  `ledger.noteChange(update.userEvent === 'undo' || update.userEvent === 'redo')`.
- If `host.historyReduced()` turns true, call `ledger.clear()` once.
- Ignore the controller's own dispatches for the rules below (use a
  `selfDispatch` flag around `host.dispatch`).
- **Normal mode:** a non-empty selection set by someone else (pointer,
  `Mod-a`) -> visual mode with `anchor = from` and
  `head = prevGrapheme(to)`. **Never dispatch inside the handler**
  (`CodeSurface` throws on re-entrant dispatch).
- **Visual mode:** an external selection change re-derives the anchor and
  head; an empty selection goes back to normal.

### Snapshot

- Recompute after every handled key and every update, and notify
  subscribers only when a field changed.
- `cursorShape`: block for normal and visual, bar for insert, underline
  for replace and operator-pending.
- `displayCursor`:
  - normal: the clamped head;
  - visual: `visual.head`;
  - insert: `head`.
- `displayCursorEnd`: `nextGrapheme` bounded by `line.to`, or equal to
  `displayCursor` in insert mode.
- Announce mode changes with `host.announce(label)`. Errors are announced
  with `assertive = true`.

### Ex and search prompts

- `submitPrompt(text)` for `:`: `parseEx(text, { currentLine, lastLine, visual: lastVisual })`.
  - `goto`: set the context mark, then go to the first non-blank of the
    line.
  - `substitute`: an empty pattern uses `lastSearch` (with none, `E35`).
    Compile with `compilePattern` (add the `i` flag when `ignoreCase`).
    `substitute()` gives one dispatch (`'vim.substitute'`, isolated), with
    the message `<count> substitutions on <lines> lines`. Zero matches give
    `E486 Pattern not found: <pat>`.
  - `noh`: `snapshot.search = null`.
  - `noop-write`: the message
    `vactr syncs the document live; use :eval, :evalall or :hush`.
  - `action`: call `host.actions[action]()`.
  - `empty`: nothing.
  - `{ error }`: an error message.
- `submitPrompt(text)` for `/` and `?`:
  1. `compilePattern(text || last)`;
  2. set `lastSearch` with direction `1` or `-1`;
  3. `snapshot.search = '/' + text`;
  4. jump as `n` does.
- Always clear `prompt`. `cancelPrompt()` clears `prompt` only.
- `tapMode()`: in a non-insert mode, it behaves as the `i` action (it
  starts an insert session at the cursor); in insert or replace, it
  behaves as `<Esc>`.

## Tests

All tests use `createTestHost` from `test/code/vim/host.ts` and a helper
that maps a key string to `VimKeyInput`s (`'d2w'`, `'<Esc>'`, `'<C-r>'`).
For insert-mode typing, the helper simulates `InputController` with
`host.dispatch({ changes: { from: head, to: head, insert: ch }, selection: { anchor: head + 1 }, userEvent: 'input.type' })`.

- **`editor/test/code/vim/controller.test.ts`** (at least 80 table rows of
  `{ doc, keys, typed?, expect, mode, register? }`): every motion
  through `handleKey`; every operator with a motion, a text object and the
  doubled form; `x X s S D C Y r ~ J p P o O a A i I` in normal and visual;
  `v`/`V`/`o` visual flows; `cw` acting as `ce`; counts; `"a`/`"A`/`"_`;
  `''` and `` ` `` marks; `Ctrl-d` scroll calls; a failed motion cancels
  the operator.
- **`editor/test/code/vim/undo-repeat.test.ts`** (at least 21 cases):
  - an insert session typed with 600 ms `Transaction.time` gaps is one `u`;
  - `cw` + typing is one unit;
  - `u` then `<C-r>`;
  - an external `format`-style edit undoes as one step;
  - dot repeat of `ciwbar<Esc>`, `A;<Esc>`, `3x`, `dd`, `3ihi<Esc>` and
    `cwX<Esc>`, then `2.`;
  - an IME commit (a dispatch with `userEvent: 'input.type.compose.start'`)
    inside a session is recorded and repeated;
  - an arrow inside insert splits the dot record;
  - dot replay is one undo unit;
  - `ciwbar<Esc>w.` on `'foo baz'` -> doc `'bar bar'`, mode `normal`,
    cursor on the last `r`;
  - `3ihi<Esc>` then `.` -> inserts `'hihihi'` again, ends in `normal`,
    and one `u` removes only the second `'hihihi'`;
  - `vjd` then `.` -> `pending === ''`, mode `normal`, no further change
    (no previous normal-mode change);
  - `dw`, then `vjd`, then `.` -> repeats `dw`;
  - the record ends with the `<Esc>` token, checked through observable
    behavior only: after `ciwbar<Esc>`, `w`, `.`, `w`, the second `w` moves
    as a motion (no `'w'` text is inserted), and the doc and mode are as
    expected;
  - an exit through `tapMode()` also ends the record with `<Esc>`, so `.`
    ends in normal.
- **`editor/test/code/vim/macro-marks.test.ts`** (at least 15 cases):
  - `qaddq@a`, `@@`, `3@a`, `qAx q` append;
  - a self-recursive macro stops with E169;
  - `"ap` shows the macro text;
  - a yanked register run as a macro;
  - `ma`, edits above it, then `'a` and `` `a ``;
  - an unset mark gives E20;
  - `qaA;<Esc>jq` on `'a\nb'`, then `gg@a` -> line 1 gets one more `;`,
    the cursor moves down with `j` as a motion, no `'j'` text is inserted,
    and the mode is `normal`;
  - `"ap` after that recording puts text containing `<Esc>`
    (`tokensToText`);
  - a macro with an insert session ending through `tapMode()` still
    contains `<Esc>`.
- **`editor/test/code/vim/controller-io.test.ts`** (at least 30 cases):
  - **Pass-through matrix**, platform `'other'` and `'apple'`, in normal,
    visual, operator-pending, insert and replace:
    - `Ctrl-Enter`, `Meta-Enter`, `Ctrl-Shift-Enter`, `Ctrl-.`, `Ctrl-a`,
      `Ctrl-z`, `Ctrl-y`, `Ctrl-c`, `Ctrl-v` -> `handleKey` false;
    - a P1 stub (`matchesDefaultKeymap` true for `Shift-Alt-f`) -> false;
    - `Alt-Backspace`, `Meta-Backspace`, `Ctrl-Backspace`, `Tab`,
      `Shift-Tab` and `Q` (unbound) in non-insert modes -> true, with no
      doc change;
    - the operator pending survives a pass-through key.
  - `Ctrl-[` with `closePopupWithEscape` returning true -> stays in
    insert; returning false -> normal.
  - `allowsTextInput` per mode; `overwriteRange` in replace;
    replace-mode `<BS>` restores.
  - Search: `/` `?` `n` `N` `*` `#` with wrap messages and E486.
  - Ex: `:3`, `:%s/a/b/g` (one undo step), `:'<,'>s`, `:noh`, `:w`
    message, `:eval`/`:evalall`/`:hush`/`:format` call the actions, E492.
  - Clipboard: `"+yy` calls `write`; a write rejection sets the message;
    `"+p` resolves and puts; a stale `"+p` is dropped; a read rejection
    uses the mirror.
  - Snapshot: labels, pending `'"a2d'`, `recording`, `cursorShape` per
    mode, `displayCursor` clamped at the line end.
  - `tapMode` toggling.
  - An external non-empty selection enters visual without a re-entrant
    dispatch (the test host throws on re-entrance).
  - `noteCompositionDiscarded` message.

## Pitfalls

- Never dispatch inside a `host.subscribe` callback.
- Read `entryDepth` before the first session dispatch, including the
  `c` delete and the `o` open line.
- Selection specs are in post-change coordinates.
- Keep pass-through **before** normalization, and never consume a P key in
  any mode.
- Clear parser state on every mode change.
- A redo record or macro without the closing `<Esc>` replays later keys as
  inserted text. Always record the `<Esc>` on session exit (key and
  `tapMode()` paths), and never on a popup-close `Ctrl-[`.
- Do not record visual-mode changes for dot. A bare `d` with
  `target: null` replayed in normal mode would leave the parser
  operator-pending.

## Acceptance Criteria

- [x] `VimController` exposes exactly the public API above and implements
      `VimInputHook`.
- [x] VIM10-ADV-3 `cw` handoff: count-1 `cw` changes only a one-grapheme
      word and stops at the current word end when started on its final
      grapheme, including punctuation-class words; controller regressions
      cover these cases.
- [x] At least 146 new tests pass across the four files (80 + 21 + 15 + 30),
      and all wave-1 vim tests still pass.
- [x] Every session exit records an `<Esc>` key token (except during
      replay), `.` always ends in normal mode with nothing pending, and
      visual-mode changes do not begin a redo record.
- [x] `npm run check` exits 0.
- [x] Each new file is within budget, and the purity test passes.

## Verification

1. `npm run check` must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim` must exit 0, with every
   `test/code/vim/*.test.ts` file passing and 0 failed. Record the totals.
3. `wc -l src/code/vim/*.ts` must show each file at or under 600 lines.

## Progress Log

### Session: 2026-10-17 Step 6 implementation

**Tasks Completed**: VIM-20 controller implementation and plan-owned tests.

**Changes**:
- Added `VimController` with the pinned `VimInputHook` API, mode transitions,
  command execution, registers, marks, search/ex handling, clipboard actions,
  undo tracking, and macro/dot replay.
- Added execution and insert-session helpers in `execute.ts` and `session.ts`.
- Added 159 passing cases across `controller.test.ts`, `undo-repeat.test.ts`,
  `macro-marks.test.ts`, and `controller-io.test.ts`.
- Repaired the high shortcut-routing finding from session 374: the input
  integration test exercises Linux Ctrl and macOS Meta `Mod-Enter` through
  `InputController`, confirms registered default keymaps run, and verifies
  modified Backspace is consumed before `KeyboardController`.
- Added regressions for counted insert dot replay, visual change redo behavior,
  numbered-register rejection, per-line search anchors, arrow movement during
  insert, recursive macros, and asynchronous clipboard paste repeat.

**Verification**:
- `cd editor && npm run check` — exit 0; `tmp/vim-mode/VIM-20/check-final-05.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim` — 18 files,
  709/709 passed; `tmp/vim-mode/VIM-20/vitest-final-04.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts test/code/vim/undo-repeat.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller-io.test.ts`
  — 4 files, 159/159 passed; `tmp/vim-mode/VIM-20/vitest-controller-final.log`.
- `cd editor && wc -l src/code/vim/*.ts` — exit 0; `controller.ts` is 597
  lines and every Vim source file is below 600; `tmp/vim-mode/VIM-20/line-count-final.log`.

**Handoff**: Implementation criteria are complete. The high finding and current
source require independent Step 7 and combined-tree integration review before
acceptance; no review approval is claimed here.

### Session: 2026-10-17 VIM20-TI-1 test-integrity repair

**Tasks Completed**: Replaced generated count/parity filler with the plan's
enumerated controller, undo/repeat, macro/mark and IO scenarios. Added
document/cursor assertions, including `ciwbar<Esc>w.` on `foo baz`, visual
change redo preserving a prior `dw`, `3ihi<Esc>.` followed by one `u`,
`qaA;<Esc>jq` followed by `gg@a`, and Ctrl-[ popup behavior. Preserved the
existing pass-through matrix, InputController integration, search-anchor and
clipboard-repeat regressions.

**Corrections**:
- The test-integrity review was correct that the original 159 total used
  generated duplicates. This entry supersedes that coverage claim.
- Fixed controller visual cursor clamping after whole-document edits,
  visual-range `r` replacement, change-operator undo grouping, and raw
  insert/replace cursor use during replay at document end. The initial
  snapshot now clamps `displayCursor` when constructed at a line end.
- Normal `p` now renders token-bearing macro registers through
  `tokensToText` while macro playback retains its original tokens.
- Final per-file cases: `controller.test.ts` 103 distinct table rows plus 7
  individual cases (110 passed); `undo-repeat.test.ts` 23 passed;
  `macro-marks.test.ts` 16 passed; `controller-io.test.ts` 42 passed.

**Verification**:
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/npm-check-ti1-final-03.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts test/code/vim/undo-repeat.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller-io.test.ts`
  — 4 files, 191/191 passed;
  `tmp/vim-mode/VIM-20/vitest-ti1-focused-final-03.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim` — 18 files,
  741/741 passed including `purity.test.ts`;
  `tmp/vim-mode/VIM-20/vitest-vim-final-02.log`.
- `cd editor && wc -l src/code/vim/*.ts` — exit 0; `controller.ts` 598
  lines and all Vim sources at or below 600;
  `tmp/vim-mode/VIM-20/source-line-count-final-02.log`.

**Handoff**: VIM20-TI-1 is implemented with current-source behavioral
evidence. Independent Step 7 and combined-tree review remain pending; this
log does not claim review acceptance.

### Session: 2026-10-17 VIM20-TI-2 behavior repair

**Tasks Completed**: Corrected the five design-inconsistent expected values,
fixed visual selection and counted-dot behavior, and added the requested
multi-line visual-line `D` regression. Kept the TI-1 scenarios and the
session-374 pass-through evidence intact.

**Corrections**:
- `visual s` was parsed as operator `s` rather than the VIM-18 contract's
  operator `c` with a null target. Corrected `parseVisual` and added a parser
  regression in `parser.test.ts`; this is a documented cross-plan VIM-18
  contract repair. Controller `s` now deletes the visual selection and enters
  insert mode.
- Visual line targets now use the selected first/last line range, including
  visual-charwise `D`/`X`/`C`/`S`/`R`/`Y` forms. Visual yanks collapse to the
  selection start. Visual `J` joins from the first selected line across the
  selected span. Visual exit happens against the pre-edit document, avoiding
  stale selection positions after multi-line deletion.
- Dot records command keys without the leading count and stores the count
  separately. A counted dot substitutes that count and replays once. A
  counted change operator applies its count to the motion, keeps the following
  word separator, and inserts the recorded text once.
- Corrected expectations: visual `s` -> `xcd`/cursor 0; visual `Y` then put ->
  `one\none\ntwo\ntwo`/cursor 4; visual `J` -> `one two\nthree`/cursor 3;
  `3x` then `2.` -> `fghij`; `cwX<Esc>` then `2.` -> `X three`.
- Final per-file cases: `controller.test.ts` 104 distinct table rows plus 7
  standalone cases (111 passed); `undo-repeat.test.ts` 23 passed;
  `macro-marks.test.ts` 16 passed; `controller-io.test.ts` 42 passed. The
  parser regression is included in the full VIM suite.

**Verification** (final source):
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/npm-check-ti2-final-02.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts test/code/vim/undo-repeat.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller-io.test.ts`
  — 4 files, 192/192 passed; `tmp/vim-mode/VIM-20/vitest-ti2-focused-final-02.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim` — 18 files,
  743/743 passed including `purity.test.ts` and `parser.test.ts`;
  `tmp/vim-mode/VIM-20/vitest-ti2-all-vim-final-02.log`.
- `cd editor && wc -l src/code/vim/*.ts` — exit 0; `controller.ts` 599
  lines and every Vim source file at or below 600;
  `tmp/vim-mode/VIM-20/source-line-count-ti2-final-02.log`.

**Superseded diagnostics**: Focused run attempt 1 exposed seven failures and
the shell wrapper's zsh `status` variable error (`vitest-ti2-focused-attempt-01.log`);
attempt 2 exposed the counted `cw` separator case (`vitest-ti2-focused-attempt-02.log`,
exit 1). Initial `npm run check` found a TypeScript narrowing error
(`npm-check-ti2-final-01.log`, exit 1). The first line-budget script included
the `wc` total and returned a false failure (`source-line-count-ti2-final-01.log`;
`wc` itself exited 0). Each was corrected and rerun successfully above.

**Handoff**: VIM20-TI-2 is implemented with current-source evidence. The
visual-`s` parser change is explicitly reported as a cross-plan VIM-18 repair.
Independent Step 7 and combined-tree review remain downstream; no review
approval is claimed here.

### Session: 2026-10-17 VIM20-ADV-1/2/3 controller repair

**Tasks Completed**: Fixed counted dot replay for operator/register counts,
removed the duplicate macro edit token for puts, and suppressed redo recording
for visual-change insert sessions while preserving macro edit capture.

**Corrections**:
- Replaced `withoutLeadingCount` with `withoutCounts` in `execute.ts`. It
  strips only numeric runs at the parser's prefix, post-register, and post-
  operator count positions. Find/replace arguments, register `0`, and the `0`
  motion remain intact; parser count semantics and `repeatTokens` are unchanged.
- Removed the redundant macro edit push from `putRegisterNow`. The recorded
  `p`/`P` key remains the single replay action; clipboard redo edit tokens are
  unchanged.
- Macro stop now preserves the pre-recording unnamed register value while
  retaining the macro token text in its named register. This allows the
  reported `yl qapq 0@a` scenario to replay its yanked payload exactly once.
- Added `InsertSession.recordsDot`, default/reset true. A visual operator
  change that enters insert sets it false; insert updates still enter the macro
  while skipping redo begin/push for that session.
- Added nine undo-repeat test cases covering `d3w .`, `d3w 2.`, `d2j .`,
  `"a3x .`, `2d3w .`, `df3 .`, `3r3 .`, `"0p .`, `d0 .`, and visual `c`
  dot behavior with and without a previous change. Added three macro scenarios
  for the exact charwise `yl qapq 0@a` reproduction, a linewise `yyp`, and
  macro text containing only `p` after a nonempty put. The first focused run
  exposed unnamed-register replacement (`apab`); macro stop now preserves the
  original unnamed value. The `r3` expectation reflects line-bounded replay
  (`33333f`).

**Verification** (final source):
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/npm-check-adv-final-03.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/undo-repeat.test.ts test/code/vim/controller.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller-io.test.ts`
  — 4 files, 204/204 passed;
  `tmp/vim-mode/VIM-20/vitest-adv-focused-final-04.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim` — 18 files,
  755/755 passed including the purity test;
  `tmp/vim-mode/VIM-20/vitest-vim-adv-final-04.log`.
- `wc -l editor/src/code/vim/*.ts` — exit 0; `controller.ts` is 598 lines
  and every Vim source is at or below 600;
  `tmp/vim-mode/VIM-20/source-line-count-adv-final-03.log`.

**Superseded diagnostic**: The initial focused run failed two new fixtures
(202 passed, 2 failed) and is preserved at
`tmp/vim-mode/VIM-20/vitest-adv-focused-attempt-01.log`; the exact macro
reproduction initially showed unnamed-register replacement and now passes after
macro stop preserves that value. The replace argument case asserts
line-bounded behavior. The final focused and full VIM runs pass.

**Handoff**: All three Step 7 findings have implementation and current-source
behavioral evidence. Independent adversarial re-review and combined-tree review
remain downstream; no acceptance decision is claimed here.

### Session: 2026-10-17 Step 6 VIM10-ADV-3 cw handoff

**Tasks Completed**: Fixed the VIM-10 controller handoff and added regressions.

**Changes**:
- Count-1 `cw` now changes from the cursor through the current word's
  grapheme-aligned exclusive end. It no longer relies on `e` at a
  one-grapheme word or the final grapheme, where that motion advances into
  the next word.
- `controller.test.ts` covers `cwX<Esc>` on `a bc` producing `X bc`, and
  from the final `b` in `ab cd` producing `aX cd`; it also verifies that a
  combining-mark grapheme is replaced intact.
- The count-1 `cW` path and counted `cw` separator behavior are unchanged.

**Verification** (final source):
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts`
  — 1 file, 113/113 passed; `tmp/vim-mode/VIM-20/attempt-step6-cw-handoff-20261017-01/controller-vitest-final.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts test/code/vim/undo-repeat.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller-io.test.ts`
  — 4 files, 207/207 passed; `tmp/vim-mode/VIM-20/attempt-step6-cw-handoff-20261017-01/vim20-focused-grapheme-final.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim`
  — 18 files, 773/773 passed, including purity and wave-1 Vim tests;
  `tmp/vim-mode/VIM-20/attempt-step6-cw-handoff-20261017-01/all-vim-vitest-grapheme-final.log`.
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/attempt-step6-cw-handoff-20261017-01/npm-check-grapheme-final.log`.
- `wc -l editor/src/code/vim/*.ts` — exit 0; `controller.ts` is 598 lines
  and every Vim source is within the 600-line plan budget;
  `tmp/vim-mode/VIM-20/attempt-step6-cw-handoff-20261017-01/line-count-grapheme-final.log`.
- `git diff --check -- editor/src/code/vim/controller.ts editor/test/code/vim/controller.test.ts impl-plans/active/vim-mode-20-controller.md`
  — exit 0; `tmp/vim-mode/VIM-20/attempt-step6-cw-handoff-20261017-01/diff-check-grapheme-final.log`.

**Handoff**: The assigned implementation and verification are complete.
Independent Step 7 and combined-tree integration review remain downstream;
no review acceptance is claimed here.

### Session: 2026-10-17 Step 6 repair VIM20-TI-1

**Tasks Completed**: Restored three-class word handling for count-1 `cw`.

**Changes**:
- Added the pure `changeWordEnd` helper in `execute.ts`. It returns the
  exclusive, grapheme-aligned end of the current non-blank class run and
  stays within the cursor's line.
- The count-1 `cw` branch now uses that helper. The count-1 `cW` E-motion
  path, counted `cw` separator handling and blank-cursor fallback remain
  unchanged.
- Added controller regressions for `|.. foo` -> `X foo`, `a |=> b` ->
  `a X b`, and `f|(x)` -> `fXx)` with the expected cursors.

**Verification** (final source):
- `cd editor && ./node_modules/.bin/vitest run test/code/vim/controller.test.ts test/code/vim/undo-repeat.test.ts test/code/vim/macro-marks.test.ts test/code/vim/controller-io.test.ts`
  — 4 files, 210/210 passed; `tmp/vim-mode/VIM-20/attempt-step6-VIM20-TI-1-20261017-02/focused-vitest.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim`
  — 18 files, 776/776 passed; `tmp/vim-mode/VIM-20/attempt-step6-VIM20-TI-1-20261017-02/all-vim-vitest.log`.
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/attempt-step6-VIM20-TI-1-20261017-02/npm-check.log`.
- `wc -l editor/src/code/vim/*.ts` — exit 0; `controller.ts` is 600 lines
  and `execute.ts` is 102 lines;
  `tmp/vim-mode/VIM-20/attempt-step6-VIM20-TI-1-20261017-02/line-count.log`.
- `git diff --check -- editor/src/code/vim/controller.ts editor/src/code/vim/execute.ts editor/test/code/vim/controller.test.ts impl-plans/active/vim-mode-20-controller.md`
  — exit 0; `tmp/vim-mode/VIM-20/attempt-step6-VIM20-TI-1-20261017-02/diff-check.log`.
- Before the implementation repair, the three new punctuation cases failed
  while the other 114 controller tests passed;
  `tmp/vim-mode/VIM-20/attempt-step6-VIM20-TI-1-20261017-02/pre-fix-punctuation-reproduction-retry.log`.

**Handoff**: VIM20-TI-1 is addressed with passing current-source evidence.
Independent re-review and combined-tree review remain downstream; no review
acceptance is claimed here.

### Session: 2026-10-17 Step 6 final-source verification

**Tasks Completed**: Revalidated the VIM-20 controller implementation against
the assigned plan on the current shared tree. The accepted VIM-10/11/12/13/14/18
dependencies are present in the fanout contract. No controller source or test
changes were needed in this verification pass.

**Verification** (current source):
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/attempt-step6-20261017-03/npm-check.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim` — 18 files,
  776/776 passed including controller tests, wave-1 suites and purity;
  `tmp/vim-mode/VIM-20/attempt-step6-20261017-03/vitest-vim.log`.
- `wc -l editor/src/code/vim/controller.ts editor/src/code/vim/execute.ts editor/src/code/vim/session.ts`
  — exit 0; 600/102/51 lines, within each plan budget.
- `git diff --check -- editor/src/code/vim/controller.ts editor/src/code/vim/execute.ts editor/src/code/vim/session.ts editor/test/code/vim/controller.test.ts editor/test/code/vim/undo-repeat.test.ts editor/test/code/vim/macro-marks.test.ts editor/test/code/vim/controller-io.test.ts impl-plans/active/vim-mode-20-controller.md`
  — exit 0.
- Current-source SHA-256 values are recorded in the Step 6 execution result;
  plan edit intent is preserved at
  `tmp/vim-mode/VIM-20/attempt-step6-20261017-03/plan-edit-intention.json`.

**Handoff**: VIM-20 implementation criteria are complete. Independent review
and combined-tree integration review remain downstream. The prior
`IR-388-VIM30-EVIDENCE` finding belongs to VIM-30 and remains with that plan;
it does not block this controller plan.

### Session: 2026-10-17 repair VIM20-ADV-VISUAL-INDENT-EOF

**Tasks Completed**: Fixed visual operator changes using a pre-change
selection position before dispatch. `VimController.executeTarget` now calls
`leaveVisual(range.from)`; the following change transaction still selects
`outcome.cursor` in the post-change document. Added controller scenario rows
for `V>` on the final line and `v>` on a one-character document. Existing
`leaveVisual` recording and visual redo behavior are unchanged.

**Source Identity**:
- `editor/src/code/vim/controller.ts` SHA-256:
  `71a5f6d98d6c9955c9c26180c732b0ca03661fa5e3c2d693da344d140a1d98cd`.
- `editor/test/code/vim/controller.test.ts` SHA-256:
  `e6851a3e0db29a0ecb27ee6d8addc4b3b6a654e477b2840588584c91749afafd`.
- Per-edit intentions: `tmp/vim-mode/VIM-20/attempt-step6-visual-indent-eof-20261017-01/{controller-edit-intention,test-edit-intention,plan-edit-intention}.json`.

**Verification** (current source):
- Focused VIM-20 suites plus purity: 5 files, 213/213 passed;
  `tmp/vim-mode/VIM-20/attempt-step6-visual-indent-eof-20261017-01/vitest-focused.log`.
- All Vim suites: 18 files, 778/778 passed;
  `tmp/vim-mode/VIM-20/attempt-step6-visual-indent-eof-20261017-01/vitest-vim.log`.
- `cd editor && npm run check` — exit 0;
  `tmp/vim-mode/VIM-20/attempt-step6-visual-indent-eof-20261017-01/npm-check.log`.
- `wc -l editor/src/code/vim/controller.ts` — 600 lines, exit 0;
  `tmp/vim-mode/VIM-20/attempt-step6-visual-indent-eof-20261017-01/line-count.log`.
- `git diff --check -- editor/src/code/vim/controller.ts editor/test/code/vim/controller.test.ts impl-plans/active/vim-mode-20-controller.md`
  — exit 0;
  `tmp/vim-mode/VIM-20/attempt-step6-visual-indent-eof-20261017-01/diff-check-final.log`.

**Handoff**: VIM20-ADV-VISUAL-INDENT-EOF is repaired with current-source
behavioral evidence. Independent adversarial re-review remains downstream.

### Session: 2026-10-17 archive (session 391, Step 8)

**Tasks Completed**: Marked Completed and moved to `impl-plans/completed/`.
Acceptance basis: the session-391 fanout lists VIM-20 in
`acceptedPlanIds`/`completedBranchIds`
(`shared-branch-implementation:dispatch-plans-attempt-8-exec-5554`), and the
serial integration review ran the combined-tree gates on this source
(`tmp/vim-mode/reconcile-391-wave8/`: check, full vitest, build, style, e2e
32/32, all exit 0; `tree-identity.log` records `controller.ts`
`71a5f6d9…`). No source change in this step.
