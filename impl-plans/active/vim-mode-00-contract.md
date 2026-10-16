# Vim Mode: Core Contract (types, keys, words, test host) Implementation Plan

**Status**: Ready
**Plan ID**: VIM-00 (wave 0)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 3.2, 3.3.1-3.3.3, 3.4, 4.2-4.4, 8.1
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Next (wave 1, all depend on this plan)**: VIM-10 motions, VIM-11 text objects, VIM-12 operators and registers, VIM-13 search and ex, VIM-14 record/undo/marks, VIM-15 input routing, VIM-16 cursor shape, VIM-17 UI, VIM-18 parser
- **Then**: VIM-20 controller (wave 2), VIM-30 mount (wave 3), VIM-40 e2e and verification, VIM-41 docs (wave 4)
- **Manifest**: `impl-plans/active/vim-mode-dispatch.json`

## Intent and Context

We are adding an opt-in Vim mode to the canvas code pane (`editor/`, Vite +
TypeScript + Solid.js). `@codemirror/state` `EditorState` in
`editor/src/code/surface.ts` is the only document authority. There is no
`EditorView`, and `test/canvas/no-editor-view.test.ts` enforces that. The
Vim core in `editor/src/code/vim/` must be pure and DOM-free.

This plan pins every **shared type and data shape** so that nine wave-1
plans can implement their modules in parallel without coordinating. It
also delivers two small leaf modules every other module needs: key
normalization with the pass-through set (`keys.ts`) and character classes
with grapheme stepping (`words.ts`). Finally, it adds the in-memory test
host used by every core test.

## Common Execution Rules (apply to every vim-mode plan)

1. **Ownership.** Edit only this plan's `writePaths`. Before each edit,
   read the file fresh and record `git hash-object <file>` (or `missing`)
   in the Progress Log. Re-hash after the edit. If a file outside
   `writePaths` changed under you, do not touch or restore it. Note it as
   drift in the Progress Log.
2. **No git writes.** Never run `git commit`, `checkout`, `stash`,
   `reset`, `restore`, or create branches or worktrees. Read-only
   `git diff`, `git status` and `git hash-object` are allowed.
3. **Logs.** Every command runs in the foreground, from `editor/` unless
   stated, with its full output tee'd to `tmp/vim-mode/<planId>/<name>.log`
   and the exit status appended as the last line (`echo "exit=$?" >> log`).
   A truncated log is not a pass.
4. **Verification records.** Each gating command goes into the Progress
   Log as exactly
   `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
   Intentional negative-control runs go only in a separate
   `mutationEvidence` list. A repaired finding goes only in
   `addressedFeedback` with status `repaired`.
5. **Scoped typecheck for parallel waves.** While sibling plans are
   mid-edit, the project-wide `npm run check` can fail on their files. Each
   wave-0/1 plan therefore writes
   `tmp/vim-mode/<planId>/tsconfig.json` with
   `{"extends": "../../../editor/tsconfig.json", "include": [<this plan's .ts/.tsx files as paths relative to that tsconfig>]}`
   and runs `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/<planId>/tsconfig.json`.
   This checks the plan's files plus everything they import. Waves 2-4 run
   the full `npm run check`.
6. **Purity.** No file under `editor/src/code/vim/` may reference
   `document`, `window`, `navigator`, `localStorage`, `HTMLElement` or
   `@codemirror/view`. Imports are limited to `@codemirror/state`,
   `@codemirror/commands` and sibling `vim/` files.
7. **Style.** English only, no emojis. Match the dense one-line style of
   `editor/src/code/keyboard.ts` and `surface.ts`. Every file stays under
   1000 lines; the design gives targets (motions 400, controller 500).
8. **Licensing.** Write behavior from scratch, using Vim only as a
   behavioral reference. Do not copy code from Vim, GPL projects or
   `@replit/codemirror-vim`. If permissive code is adapted, stop and
   report it; `THIRD_PARTY_NOTICES.md` is not in any plan's `writePaths`.
9. **Progress Log.** Append one session entry to this plan file (tasks
   done, verification records, drift, blockers). Do not edit other plans or
   `impl-plans/README.md`.

## Non-Goals

- No behavior beyond `keys.ts` and `words.ts`. Motions, operators and the
  rest belong to wave 1.
- No edits outside `writePaths`. In particular, do not touch `input.ts`,
  `surface.ts`, `renderer.ts`, `mount.ts` or any existing test.
- No new npm dependency.

## File-Level Changes

### `editor/src/code/vim/types.ts` (new, about 180 lines)

This is a types-only module plus a few `as const` tables. Every exported
name below is a contract that later plans rely on. Do not rename fields.

- `export type VimMode = 'normal' | 'insert' | 'replace' | 'visual' | 'visual-line' | 'operator-pending'`
- `export type CursorShape = 'bar' | 'block' | 'underline'`
- `export type Platform = 'apple' | 'other'`
- `export interface VimKeyInput { key: string; code?: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }`.
  A DOM `KeyboardEvent` satisfies it structurally.
- `export type VimKey = string`. A normalized key is one of:
  - a single printable character (`'d'`, `'D'`, `':'`, `'"'`, or a
    non-ASCII character);
  - one of the names `'<Esc>' '<CR>' '<BS>' '<Del>' '<Tab>' '<Left>' '<Right>' '<Up>' '<Down>' '<Home>' '<End>' '<C-r>' '<C-d>' '<C-u>' '<C-f>' '<C-b>' '<Other>'`.
    `'<Other>'` stands for any modifier chord or unprintable key that is
    not a Vim key. It is never a command.
- `export type OperatorKind = 'd' | 'c' | 'y' | '>' | '<' | 'g~' | 'gu' | 'gU'`
- `export type MotionKind`, a union of:
  `'h' 'l' 'j' 'k' '0' '^' '$' 'w' 'b' 'e' 'ge' 'W' 'B' 'E' 'gg' 'G' 'f' 'F' 't' 'T' ';' ',' '%' '{' '}' 'H' 'M' 'L' 'C-d' 'C-u' 'C-f' 'C-b' '+' 'BS'`.
  `'+'` is also `<CR>` and `'BS'` is `<BS>` as `h`. Arrows map to
  `h j k l`, and `<Home>`/`<End>` map to `0`/`$`.
- `export interface MotionSpec { kind: MotionKind; char?: string }`. The
  `char` is the target of `f F t T`.
- `export interface FindState { kind: 'f' | 'F' | 't' | 'T'; char: string }`
- `export interface VimViewport { topLine: number; rows: number }`. The
  first fully visible line is 1-based and `rows` is at least 1.
- `export interface MotionContext { count: number; explicitCount: boolean; desiredColumn: number | null; viewport: VimViewport; lastFind: FindState | null }`.
  `desiredColumn` is `null` (unset), a grapheme column, or
  `Number.POSITIVE_INFINITY` (after `$`).
- `export interface MotionResult { pos: number; linewise: boolean; inclusive: boolean; desiredColumn: number | null; jump: boolean; scrollLines?: number; lastFind?: FindState }`
- `export type TextObjectKind = 'w' | 'W' | '"' | "'" | '`' | '(' | '[' | '{' | '<'`
- `export interface TextObjectSpec { around: boolean; kind: TextObjectKind }`
- `export interface TextRange { from: number; to: number; linewise: boolean }`.
  The range is half-open in UTF-16 offsets. For a linewise range, `from`
  is the first line's start and `to` is the last line's end, without its
  line break.
- `export interface Register { text: string; linewise: boolean; tokens?: readonly RecordToken[] }`
- `export type RecordToken = { kind: 'key'; key: VimKey } | { kind: 'edit'; deleteBefore: number; deleteAfter: number; insert: string } | { kind: 'move'; motion: MotionKind }`
- `export interface ClipboardAdapter { read(): Promise<string>; write(text: string): Promise<void> }`
- `export interface VimActions { evalAtCursor(): void; evalAll(): void; hush(): void; format(): void }`
- `export interface VimUpdate { state: EditorState; changes: ChangeSet; docChanged: boolean; selectionSet: boolean; userEvent: string | null }`.
  This is the same shape as `CodeSurfaceUpdate` in `editor/src/app/apis.ts`.
  Do not import from `app/apis.ts` (that file is not pure); declare the
  type here.
- `export interface VimHost`, with members in this order:
  - `state(): EditorState`, `dispatch(spec: TransactionSpec): void`,
    `subscribe(cb: (update: VimUpdate) => void): () => void`
  - `undo(): boolean`, `redo(): boolean`, `historyReduced(): boolean`.
    The last one mirrors `CodeSurface.retentionStatus.reducedDepth`.
  - `viewport(): VimViewport`, `scrollLines(lines: number): void`,
    `scrollCaret(): void`
  - `clipboard: ClipboardAdapter`, `actions: VimActions`
  - `closePopupWithEscape(): boolean`,
    `announce(message: string, assertive?: boolean): void`
  - `platform: Platform`, `matchesDefaultKeymap(input: VimKeyInput): boolean`
  - `compositionActive(): boolean`. True while
    `surface.compositionRange` is set.
- `export type PromptKind = ':' | '/' | '?'`
- `export interface VimSnapshot`, with these fields:
  - `mode: VimMode`
  - `label: string`, one of `'NORMAL' 'INSERT' 'VISUAL' 'VISUAL LINE' 'REPLACE'`
    (operator-pending shows `'NORMAL'`)
  - `pending: string`, the typed keys of the incomplete command, for
    example `'"a2d'`
  - `recording: string | null`, the macro register being recorded
  - `search: string | null`, the active search pattern; `:noh` clears it
  - `message: string | null`, `messageKind: 'info' | 'error' | null`
  - `prompt: { kind: PromptKind; initial: string } | null`
  - `cursorShape: CursorShape`
  - `displayCursor: number`, `displayCursorEnd: number`. The cursor
    grapheme is `[displayCursor, displayCursorEnd)`. In insert mode
    `displayCursorEnd === displayCursor`.
- `export interface VimInputHook { handleKey(input: VimKeyInput): boolean; allowsTextInput(): boolean; overwriteRange(text: string): { from: number; to: number } | null; noteCompositionDiscarded(): void }`.
  `InputController` (VIM-15) uses this, and `VimController` (VIM-20)
  implements it.
- `export const COUNT_LIMIT = 10_000`, `export const SCAN_LIMIT = 65_536`,
  `export const REPLAY_DEPTH_LIMIT = 100`,
  `export const REPLAY_TOKEN_LIMIT = 100_000`, `export const INDENT_UNIT = '  '`.

Use `import type` for `EditorState`, `ChangeSet` and `TransactionSpec`
from `@codemirror/state`.

### Module signatures pinned for wave 1

These are listed here so that VIM-20 can call them. Each owning plan
implements exactly these exports; extra internal helpers are fine.

| Module (plan) | Exports |
|---|---|
| `vim/words.ts` (VIM-00) | `charClass(ch: string): 0 \| 1 \| 2` (0 blank, 1 keyword, 2 punctuation); `classAt(line: string, index: number): 0 \| 1 \| 2` (applies the `-`-between-keyword rule); `nextGrapheme(doc: Text, pos: number): number`; `prevGrapheme(doc: Text, pos: number): number`; `graphemeColumn(doc: Text, pos: number): number`; `posAtColumn(doc: Text, lineNumber: number, column: number): number` (clamped to the last grapheme start on the line; `Infinity` means the last grapheme); `clampNormal(doc: Text, pos: number): number`; `firstNonBlank(doc: Text, lineNumber: number): number` |
| `vim/keys.ts` (VIM-00) | `normalizeKey(input: VimKeyInput): VimKey \| null` (null for bare modifier keys: `Shift`, `Control`, `Alt`, `Meta`, `CapsLock`); `isPassThrough(input: VimKeyInput, platform: Platform, matchesDefaultKeymap: (input: VimKeyInput) => boolean): boolean`; `isVimControlChord(input: VimKeyInput): boolean` |
| `vim/motions.ts` (VIM-10) | `runMotion(doc: Text, pos: number, motion: MotionSpec, ctx: MotionContext): MotionResult \| null` |
| `vim/textobjects.ts` (VIM-11) | `selectTextObject(doc: Text, cursor: number, spec: TextObjectSpec, count: number, visual: { anchor: number; head: number } \| null): TextRange \| null` |
| `vim/operators.ts` (VIM-12) | `motionRange(doc: Text, from: number, result: MotionResult, op: OperatorKind \| null): TextRange`; `applyOperator(state: EditorState, op: OperatorKind, range: TextRange): OperatorOutcome`; `putRegister(state: EditorState, cursor: number, reg: Register, before: boolean, count: number): EditOutcome`; `joinLines(state: EditorState, cursor: number, count: number): EditOutcome \| null`; `replaceChars(state: EditorState, cursor: number, ch: string, count: number): EditOutcome \| null`; `toggleCaseChars(state: EditorState, cursor: number, count: number): EditOutcome \| null`; `openLine(state: EditorState, cursor: number, above: boolean): EditOutcome`. Here `EditOutcome = { changes: ChangeSpec; cursor: number }` and `OperatorOutcome = { changes: ChangeSpec \| null; cursor: number; yanked: Register \| null; enterInsert: boolean }`. Both types are exported from operators.ts. `changes` uses start-document offsets, and `cursor` is an offset in the **resulting** document, the same convention as `TransactionSpec.selection`. The caller dispatches `{ changes, selection: { anchor: cursor } }` directly. A linewise yank returns `cursor: -1` to mean "the controller decides" (see VIM-12) |
| `vim/registers.ts` (VIM-12) | `isRegisterName(ch: string): boolean`; `class RegisterStore { read(name: string): Register \| null; write(name: string \| null, value: Register, kind: 'yank' \| 'delete'): void; isClipboard(name: string): boolean; setMirror(text: string, linewise: boolean): void }` |
| `vim/search.ts` (VIM-13) | `compilePattern(source: string): { regex: RegExp; literal: boolean; note: string \| null }`; `findMatch(doc: Text, from: number, regex: RegExp, direction: 1 \| -1): { from: number; to: number; wrapped: boolean } \| null`; `wordUnderCursor(doc: Text, pos: number): { from: number; to: number; text: string } \| null`; `wordPattern(word: string): RegExp`; `substitute(doc: Text, fromLine: number, toLine: number, regex: RegExp, replacement: string, global: boolean): { changes: ChangeSpec[]; count: number; lines: number }` |
| `vim/ex.ts` (VIM-13) | `parseEx(text: string, ctx: ExContext): ExCommand \| { error: string }`, where `ExContext = { currentLine: number; lastLine: number; visual: { fromLine: number; toLine: number } \| null }` and `ExCommand` is a union (VIM-13 pins it) |
| `vim/marks.ts` (VIM-14) | `isMarkName(ch: string): boolean`; `class MarkTable { set(name: string, pos: number): void; get(name: string): number \| null; setContext(pos: number): void; context(): number \| null; map(changes: ChangeSet): void }` |
| `vim/undo.ts` (VIM-14) | `class UndoLedger { record(unit: { entryDepth: number; exitDepth: number; exitDoc: Text }): void; undo(host: Pick<VimHost, 'state' \| 'undo'>): void; redo(host: Pick<VimHost, 'state' \| 'redo'>): void; noteChange(fromHistory: boolean): void; clear(): void }` |
| `vim/record.ts` (VIM-14) | `editToken(update: VimUpdate, cursorBefore: number): RecordToken \| null`; `class RedoRecorder { begin(first: RecordToken[]): void; push(token: RecordToken): void; end(): void; abort(): void; get active(): boolean; last(): readonly RecordToken[] \| null }`; `class MacroRecorder { start(reg: string): void; push(token: RecordToken): void; stop(): { reg: string; tokens: RecordToken[] } \| null; get register(): string \| null }`; `tokensToText(tokens: readonly RecordToken[]): string`; `textToTokens(text: string): RecordToken[]` |
| `vim/parser.ts` (VIM-18) | `class CommandParser { feed(key: VimKey, mode: VimMode): ParseResult; reset(): void; pending(): string; operatorPending(): boolean }`, with `ParseResult`/`ParsedCommand` pinned in VIM-18 |

### `editor/src/code/vim/keys.ts` (new, about 120 lines)

- `normalizeKey`:
  - Takes `input.key`. Named keys map as `Escape` -> `<Esc>`,
    `Enter` -> `<CR>`, `Backspace` -> `<BS>`, `Delete` -> `<Del>`,
    `Tab` -> `<Tab>`, `ArrowLeft/Right/Up/Down` -> `<Left>/<Right>/<Up>/<Down>`,
    `Home`/`End` -> `<Home>`/`<End>`.
  - With `ctrlKey && !metaKey && !altKey`: `[`, or `code === 'BracketLeft'`,
    becomes `<Esc>`, and `r d u f b` (lowercase `key`, or `code` `KeyR`
    and so on) become `<C-r>` and so on.
  - Any other key with `ctrlKey`, `metaKey` or `altKey` returns the
    unprintable marker `'<Other>'`. Callers treat it as consumed in
    non-insert modes; it is never a command.
  - A single printable `key` (one grapheme, not `Dead`/`Unidentified`/`Process`)
    is returned as is. `Dead`, `Unidentified` and `Process` return
    `'<Other>'`.
- `isPassThrough(input, platform, matchesDefaultKeymap)` returns true if
  any of these holds (design 3.3.1):
  - **P1**: `matchesDefaultKeymap(input)`.
  - **P2**: (`metaKey || ctrlKey`) with `key.toLowerCase()` in
    {`enter`, `.`, `a`, `z`}, where `Shift-z` and `Shift-Enter` are
    included through the same check; or `ctrlKey && key.toLowerCase() === 'y'`.
  - **P3**: (`metaKey || ctrlKey`) && `!altKey` with `key.toLowerCase()`
    in {`c`, `x`, `v`, `\\`}.
  - `platform` is accepted for clarity and for tests. P2 and P3 accept both
    modifiers on both platforms because `KeyboardController` does
    (`keyboard.ts` `handle`: `const mod = event.metaKey || event.ctrlKey`).
- `isVimControlChord(input)`: `ctrlKey && !metaKey && !altKey` and the key
  is one of `r d u f b [`.
- Pitfalls:
  - `Shift-Alt-f` on macOS produces `key === 'Ï'`. Only P1, through the
    host's `matchesKeymap` (which has the `event.code` fallback), catches
    it, so never decide P1 locally.
  - `Ctrl-y` is P2 on both platforms, but `Ctrl-r` is a Vim chord and not
    P2.
  - Do not treat a bare `Shift` keydown as a command (return `null`).

### `editor/src/code/vim/words.ts` (new, about 140 lines)

- Classes per design 4.4:
  - keyword = `/[\p{L}\p{N}_]/u`;
  - `-` is keyword only when both neighbors on the line are keyword
    characters (`classAt` handles this);
  - blank = space or tab;
  - everything else is punctuation.
- Grapheme stepping uses a module-level
  `new Intl.Segmenter(undefined, { granularity: 'grapheme' })`, scanning
  only the current line (as `keyboard.ts` `wordRangeAt` bounds its scan).
  `nextGrapheme` at a line end returns the next line's start; at the
  document end it returns `doc.length`. `prevGrapheme` at 0 returns 0.
- `graphemeColumn` counts graphemes from the line start, with a tab as one
  column (design 4.3, VQ7).
- `clampNormal(doc, pos)`: on a non-empty line where `pos >= line.to`, it
  returns the start of the last grapheme; otherwise it returns `pos`.

### `editor/test/code/vim/host.ts` (new test helper; it does not match `*.test.ts`)

- `createTestHost(text: string, opts?: { platform?: Platform; viewport?: VimViewport; defaultKeymaps?: (input: VimKeyInput) => boolean; clipboard?: ClipboardAdapter }): TestHost`.
  - It builds a real `EditorState.create({ doc, extensions: [history()] })`
    (imitate `surface.ts` constructor).
  - The cursor fixture marker is `|`: `'ab|c'` puts the cursor at 2.
  - `dispatch` applies `state.update(spec)`, stores the new state and
    calls subscribers with a `VimUpdate` built like `surface.ts` `dispatch`
    publishes. It throws on re-entrant dispatch, as `CodeSurface` does.
  - `undo`/`redo` use `undo`/`redo` from `@codemirror/commands` with
    `{ state, dispatch }`.
  - `actions` are `vi.fn()` spies. `announce` pushes to `messages[]`.
  - The default clipboard is an in-memory `ClipboardAdapter` with a
    `fail` switch.
  - The default viewport is `{ topLine: 1, rows: 10 }`. `scrollLines`
    records calls.
  - `historyReduced()` returns false. `compositionActive()` returns false.
- Also export `show(state: EditorState): string`, which renders the doc
  with `|` at `selection.main.head`, for table assertions.

### Tests

- `editor/test/code/vim/keys.test.ts`, table-driven:
  - `{key:'Escape'}` -> `<Esc>`; `{key:'[',ctrlKey:true}` -> `<Esc>`;
    `{key:'r',ctrlKey:true}` -> `<C-r>`; `{key:'d'}` -> `d`; `{key:'Shift'}`
    -> `null`; `{key:'Dead'}` -> `<Other>`; `{key:'ƒ',altKey:true}` -> `<Other>`.
  - Pass-through for platform `'other'` and `'apple'`: `Ctrl-Enter`,
    `Meta-Enter`, `Ctrl-Shift-Enter`, `Ctrl-.`, `Meta-.`, `Ctrl-a`,
    `Meta-z`, `Ctrl-Shift-z`, `Ctrl-y`, `Ctrl-c`, `Meta-v`, `Ctrl-\` ->
    true on both platforms. `Ctrl-r`, `Ctrl-d`, `Alt-Backspace`,
    `Meta-Backspace`, `Ctrl-Backspace`, `Shift-Tab` and `j` -> false.
    A stub `matchesDefaultKeymap` returning true for one input -> true.
- `editor/test/code/vim/words.test.ts`:
  - `classAt('sound-kit', 5)` -> 1 and `classAt('a -b', 2)` -> 2;
  - grapheme stepping over `'é'`, `'😀'` and `'日本'`;
  - `clampNormal` on `'abc|'` -> 2 and on an empty line -> unchanged;
  - `posAtColumn` with `Infinity`.
- `editor/test/code/vim/purity.test.ts` (`// @vitest-environment node`).
  Imitate the directory walk in `test/canvas/no-editor-view.test.ts`. For
  every `.ts` file under `src/code/vim/`, assert that no line matches
  `/\b(document|window|navigator|localStorage|HTMLElement)\b/` outside
  comments, and that no import of `@codemirror/view` exists. Assert that at
  least 3 files were scanned. This keeps the test meaningful once the
  directory grows.

## Invariants

- `types.ts` has no runtime code except the `as const` limit constants.
- No existing file is modified.

## Acceptance Criteria

- [ ] `types.ts`, `keys.ts` and `words.ts` export exactly the names in
      the tables above, with the stated signatures.
- [ ] `keys.test.ts`, `words.test.ts` and `purity.test.ts` pass, with at
      least 30 test cases in total.
- [ ] The scoped typecheck passes.

## Verification

1. `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-00/tsconfig.json`
   must exit 0 with no output.
2. `./node_modules/.bin/vitest run test/code/vim/keys.test.ts test/code/vim/words.test.ts test/code/vim/purity.test.ts`
   must exit 0 with 3 files passed and at least 30 tests, 0 failed.
3. `wc -l src/code/vim/*.ts test/code/vim/host.ts` must show every file
   under 300 lines.

## Progress Log

### Session: (not started)
