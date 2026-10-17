# Optional Vim Mode for the Code Pane

Status: Implemented (2026-10-17, workflow session 391, branch `wf/vim-mode`;
proposed 2026-10-16, session 374). The implementation plans are archived as
`impl-plans/completed/vim-mode-*.md`.

Extension (proposed 2026-10-17, session 395, branch `wf/vim-keymap`):
user mappings from a JSON keymap document, a `mise-darwin` preset, extra
built-ins (`gj gk gJ g; g, zz <C-e> <C-y>`, `ys cs ds`), next-key hints and
hop jumps are specified in `design-vim-keymap-hints-hop.md`. With no stored
keymap document, everything in this document applies unchanged.

This document covers an opt-in Vim modal editing mode for the canvas code
pane in the browser and Tauri editor (`editor/`). It defines the module
boundary, how the mode joins the existing input path, the Vim behavior in
scope, IME rules, the cursor-shape hook, the status and ex rows, the
settings toggle and the verification gates. Open decisions are in
`design-docs/user-qa/pending-vim-mode-questions.md` (VQ1-VQ9). Until they
are answered, implementation follows the recommendations.

## 1. Overview

The code pane today works like this (design-implementation.md 15.3):
`@codemirror/state` `EditorState` in `CodeSurface` (`editor/src/code/surface.ts`)
is the only document authority. A WebGL canvas (`renderer.ts`) draws the
text. A hidden textarea bridge (`input.ts` `InputController`) owns platform
input and IME, and it routes keys `keydown` -> `surface.runKeymaps` ->
`keyboard.ts` `KeyboardController`. There is no `EditorView`. This rules
out `@replit/codemirror-vim`, and `test/canvas/no-editor-view.test.ts`
enforces the ban.

Vim mode adds:

- a pure, DOM-free Vim core under `editor/src/code/vim/`. It turns key
  input into `TransactionSpec`s, selection changes and host actions;
- a thin adapter layer in `editor/src/code/` that connects the core to
  `InputController`, `CodeSurface`, the view host, the clipboard, the eval
  and format actions and the accessibility live region;
- a cursor-shape field on the renderer feedback (block, bar, underline);
- a mode/status row, an ex command row and a toggle in the transport bar.

Vim mode is **off by default**. When it is off, no Vim object exists, the
key routing order is the same as today, the cursor is drawn by today's
code path and the status row is not in the DOM (section 3.7).

## 2. Scope

### 2.1 In scope

| Area | Items |
|------|-------|
| Modes | normal, insert, replace (`R`), visual charwise (`v`), visual linewise (`V`), operator-pending |
| Motions (all take a count) | `h j k l`, `w b e W B E ge`, `0 ^ $`, `gg G {n}G`, `f F t T ; ,`, `%`, `{ }`, `H M L`, `Ctrl-d Ctrl-u Ctrl-f Ctrl-b`, `* #`, `/ ? n N`; `j`/`k` keep the desired column; arrows, `Home`/`End`, `Backspace` (as `h`) and `Enter` (as `+`) in non-insert modes |
| Operators | `d c y > < g~ gu gU` with a motion or text object, doubled forms `dd cc yy >> << g~~ guu gUU` |
| Shortcut commands | `D C Y x X s S r{c} ~ J p P o O a A i I`, `u`, `Ctrl-r`, `.`, `v V` toggling, `o` in visual (swap ends) |
| Text objects | `iw aw iW aW`, `i" a" i' a'` (and `` i` a` ``), `i( a( ib ab i) a)`, `i[ a[ i] a]`, `i{ a{ iB aB i} a}`, `i< a< i> a>` |
| Registers | unnamed `"`, `a-z` (`A-Z` appends), `0` (last yank), `_` (black hole), `+` and `*` (system clipboard) |
| Repeat | counts, dot repeat (including insert sessions), macros `q{reg}` / `@{reg}` / `@@` |
| Marks | `m{a-z}`, `'{a-z}`, `` `{a-z} ``, plus the context mark `''` / ``` `` ``` |
| Undo | `u` / `Ctrl-r` through the existing history; one insert session is one undo step |
| Ex row | `:{n}`, `:$`, `:[range]s/pat/rep/[gi]`, `:noh`, `:w :q :wq :x :q!` (no-op with a message), `:eval :evalall :run :hush :format` |
| UI | toggle (default off, saved per viewer), mode indicator with pending count/operator/register and recording state, live-region announcements, block/bar/underline cursor |
| Input | live-coding shortcuts in every mode; Escape closes the completion popup first; `Ctrl-[` is an alias for Escape; IME rules in section 3.5 |

### 2.2 Excluded (each has a user-qa entry with a recommendation)

- Visual block mode `Ctrl-v` (VQ1). It is left unbound, so the platform
  default stays (paste on Windows/Linux, nothing on macOS).
- Search-match highlighting (`hlsearch`). Drawing it needs a new renderer
  annotation kind, which goes beyond the cursor-only renderer rule
  (VQ3). `:noh` is accepted and clears the active-search state shown in
  the status row.
- Numbered registers `1-9`, the small-delete register `-`, read-only
  registers (`.`, `%`, `:`), insert-mode `Ctrl-r`/`Ctrl-o`/`Ctrl-w`/`Ctrl-d`/`Ctrl-t`,
  `gv`, `gi`, tag text objects `it`/`at`, `:g`, `:normal`, a vimrc, ex
  history, incremental search, the `c` substitute flag (VQ4). User
  mappings are now covered by the JSON keymap document of
  `design-vim-keymap-hints-hop.md`.
- Folding, windows, buffers and tabs. They do not apply to a single
  document.

## 3. Architecture

### 3.1 Data flow

```
keydown (textarea) --InputController--> [composition gate] --> highest keymaps (completion)
        --> VimController.handleKey (only when enabled) --> default keymaps (eval, format)
        --> KeyboardController.handle
beforeinput/input/composition --InputController--> VimInputGate (only when enabled)
VimController --VimHost--> CodeSurface.dispatch / undo / redo, view scroll,
        clipboard, eval/format/hush actions, announce
VimController --snapshot--> status view (mode, pending, message) and mount (cursor shape)
```

### 3.2 Module layout

The core is pure TypeScript. It imports only `@codemirror/state` and
`@codemirror/commands` (`undoDepth`, `redoDepth`, `isolateHistory`) and
must not use `document`, `window`, `navigator`, `localStorage` or DOM
types. A new purity test enforces this (section 8). Every file stays well
under the 1000-line limit; the budgets below are targets.

| File | Responsibility | Target lines |
|------|----------------|--------------|
| `vim/types.ts` | `VimMode`, `CursorShape`, `VimKeyInput`, `VimKey`, `Register`, `VimSnapshot`, `VimHost`, `ClipboardAdapter` | 150 |
| `vim/keys.ts` | normalize a key input to a `VimKey` (`<Esc>`, `<C-r>`, printable); pass-through rules (3.3) | 120 |
| `vim/parser.ts` | pending-command grammar: `["x][count]op[count](motion|textobject)`, `f{c}`, `r{c}`, `m{a}`, `q{r}`, `@{r}`, `g` prefixes | 250 |
| `vim/words.ts` | character classes and grapheme stepping on a line | 120 |
| `vim/motions.ts` | motion functions `(doc, pos, count, ctx) -> {pos, linewise, inclusive}` | 400 |
| `vim/textobjects.ts` | word, quote and bracket objects | 250 |
| `vim/operators.ts` | operator -> range -> `TransactionSpec` plus register write and next mode | 350 |
| `vim/registers.ts` | register store, append, `0`/`_`, clipboard mirror | 150 |
| `vim/search.ts` | pattern compile, `/ ? n N * #`, `:s` replacement translation | 250 |
| `vim/marks.ts` | mark table mapped through changes | 100 |
| `vim/record.ts` | redo record (dot) and macro tokens, replay limits | 300 |
| `vim/undo.ts` | undo-unit ledger over the CodeMirror history (4.9) | 150 |
| `vim/ex.ts` | ex parser and dispatcher | 250 |
| `vim/controller.ts` | `VimController`: mode machine, snapshot, subscriptions, composition of the above | 500 |

Adapters (they use the DOM and live outside `vim/`):

| File | Responsibility |
|------|----------------|
| `code/vim-mount.ts` | `attachVim(...)`: builds the `VimHost` from `CodeSurface`, `CodeViewHost`, `EvalController`, `formatDocument`, the clipboard and `AccessibilityBridge`; installs the controller into `InputController`; returns `{ controller, dispose }` |
| `code/vim-clipboard.ts` | `ClipboardAdapter` over `navigator.clipboard` with a fallback (4.8) |
| `code/vim-settings.ts` | `readVimEnabled` / `writeVimEnabled` on key `vactr.vimMode` (3.7) |
| `code/vim-status-view.tsx` | Solid status row and ex/search input row |
| `code/cursor-shape.ts` | pure `cursorRect(shape, caret, next, fallbackWidth)` used by the renderer (3.6) |

Existing files touched, kept small:

- `input.ts`: an optional `VimInputHook` (`setVim(hook | null)`), the
  split key routing and the text-input gate.
- `surface.ts`: `runKeymaps(event, precedence?)` gains an optional
  `'highest' | 'default'` filter. With no argument it behaves as today.
  It also gains a read-only `matchesKeymap(event, precedence)` query that
  uses the same `matchesKey` and runs no binding (3.3.1).
- `renderer.ts`: two optional `RenderFeedback` fields, and the cursor quad
  is computed through `cursorRect`.
- `mount.ts`: create or dispose the Vim attachment when the setting
  changes, and pass the cursor shape and display cursor to `render()`.
- `code.css`: status-row rules scoped to `.vact-code[data-vim='on']`.
- `ui/transport-view.tsx`, `code/transport.ts`: the toggle button.

### 3.3 Key routing and the default-off invariant

`InputController` keydown keeps its composition guard unchanged
(`composing`, `isComposing`, `keyCode 229`). When the guard passes:

1. `surface.runKeymaps(event, 'highest')`: the completion popup keys.
2. When Vim is enabled: `vim.handleKey(event)`. If it returns true, the
   event is `preventDefault`ed and routing stops.
3. `surface.runKeymaps(event, 'default')`: eval (`Mod-Enter`,
   `Mod-Shift-Enter`, `Mod-.`) and format (`Shift-Alt-f`).
4. `keyboard.handle(event)`.

With Vim disabled, steps 1 and 3 run in the same order as today's single
`runKeymaps` loop, so behavior is identical.

#### 3.3.1 Pass-through set (every mode)

Pass-through is decided by explicit shortcut bindings, not by modifier
class. "Mod" follows the existing platform rule in `surface.ts`
`matchesKey`: `Meta` when `navigator.platform` matches `/Mac|iP/`, and
`Ctrl` everywhere else (Windows, Linux, and the jsdom default). In every
mode, `handleKey` returns false (it does not consume and does not
`preventDefault`) for a key in the pass-through set **P**:

- **P1, registered surface keymaps at `'default'` precedence.** These are
  matched with the same `matchesKey` semantics through a new read-only
  query on `CodeSurface`: `matchesKeymap(event, precedence): boolean`. It
  reports whether any binding in that precedence group matches, and it
  runs nothing. Today the group holds eval (`Mod-Enter`,
  `Mod-Shift-Enter`, `Mod-.`, `eval.ts:88-90`) and format (`Shift-Alt-f`,
  `format.ts:8`). The query reads the live registry, so bindings added
  later are passed too. The `'highest'` group (completion) runs before
  Vim at step 1 and needs no query.
- **P2, the `KeyboardController` shortcuts (`keyboard.ts:86-92`).**
  `KeyboardController` treats `metaKey || ctrlKey` as mod on every
  platform, so these are listed for both modifiers: (`Meta` or `Ctrl`)
  with `Enter`, `Shift-Enter`, `.`, `a`, `z` and `Shift-z`, plus `Ctrl-y`.
  This covers the fallbacks the keymaps do not match. Examples are
  `Ctrl-Enter` and `Ctrl-.` on macOS, and `Meta-Enter` on Linux.
- **P3, the platform clipboard and shell chords.** (`Meta` or `Ctrl`)
  with `c`, `x` and `v`, without `Alt` (native `copy`/`cut`/`paste`
  events, handled by `InputController.clipboard` as today), and
  (`Meta` or `Ctrl`) with `\` (the side-pane fold in `ui/shell.tsx`).
  On Windows/Linux, `Ctrl-v` is therefore paste (VQ1).

The controller never calls `stopPropagation`, so window-level listeners
such as the shell's `Mod-\` handler see every key in any case. Chords the
OS or browser intercepts before the page sees them (for example
`Cmd-Tab`, `Cmd-q`, `Cmd-w`, `Ctrl-w`, `Ctrl-t`, `Ctrl-n`) never reach
`handleKey`, and Vim mode does not affect them.

#### 3.3.2 Vim's own control chords

Apart from P, the controller handles only these `ctrlKey` chords:
`Ctrl-r`, `Ctrl-d`, `Ctrl-u`, `Ctrl-f`, `Ctrl-b` (non-insert modes) and
`Ctrl-[` (all modes). None of them is in P. They are not bound by a
surface keymap or by `KeyboardController` on any platform. On macOS they
are also free of the `Meta`-based platform shortcuts. On Windows and
Linux, consuming them in non-insert modes also suppresses the browser's
own `Ctrl-r` (reload), `Ctrl-f` (find), `Ctrl-d`, `Ctrl-u` and `Ctrl-b`,
but only while the code bridge has focus. `Ctrl-Space` (completion) is
handled at step 1 before Vim. A completion accepted in a non-insert mode
is an explicit user edit and is allowed.

#### 3.3.3 Consumption by mode

- **Non-insert modes** (normal, visual, visual-line, operator-pending):
  every keydown that is not in P is **consumed**. While the ex/search row
  is open, keys go to its own input and never reach the bridge. This covers Vim commands, unbound printable keys,
  `Dead`, `Unidentified`, and every other `Meta`, `Alt` and `Ctrl` chord.
  In particular, `Backspace`, `Delete` and `Tab` are consumed with any
  modifier. Plain `Backspace` is the `h` motion and plain `Delete` is
  `x`. `Alt-`, `Ctrl-` and `Meta-Backspace`/`Delete`, `Shift-Tab` and
  `Alt-Tab` do nothing. So `KeyboardController`'s modifier-agnostic
  delete branch (`keyboard.ts:93`) and its tab insert are unreachable
  from a non-insert mode. Arrows and `Home`/`End`, with or without
  modifiers, are Vim motions or are consumed, so `KeyboardController`
  never moves or extends the selection in these modes. The only keys that
  can change the document from a non-insert mode are Vim commands, P1
  live-coding bindings (format), P2 undo/redo, and P3 cut/paste.
- **Insert and replace modes:** the controller consumes only `Escape`,
  `Ctrl-[` and (in replace mode) `Backspace`. Everything else continues
  to steps 3 and 4 and to native text input, as today.

Escape precedence: when the completion popup is open, step 1 consumes
`Escape` and closes the popup. Insert mode is left on the next Escape.
For `Ctrl-[`, the controller first runs the highest keymaps with a
synthetic `Escape` `KeyboardEvent`, using the same matcher. If a popup
consumed it, the controller stops; otherwise it acts as Escape.

### 3.4 Host contract

`VimHost` is the only thing the core sees. Tests use an in-memory host
over a real `EditorState`.

- `state(): EditorState`, `dispatch(spec)`, `undo(): boolean`, `redo(): boolean`
- `viewport(): { topLine: number; rows: number }`. The first fully visible
  1-based line and the number of visible rows, from `CodeViewHost.viewport`
  and the layout line height.
- `scrollLines(n)`: scrolls the view by n lines (`CodeViewHost.scrollBy`)
  and `scrollCaret()`.
- `clipboard: ClipboardAdapter` (4.8).
- `actions: { evalAtCursor(); evalAll(); hush(); format() }`, mapped to
  `EvalController` and `formatDocument`.
- `closePopupWithEscape(): boolean` (3.3) and `announce(message, assertive?)`.
- `platform: 'apple' | 'other'`, from the same `/Mac|iP/` test as
  `matchesKey`, and `matchesDefaultKeymap(input): boolean`, backed by
  `surface.matchesKeymap(event, 'default')`. `keys.ts` uses these to
  compute the pass-through set P (3.3.1). P2 and P3 are pure tables in
  `keys.ts`.

Every change the controller makes is one `dispatch` with
`isolateHistory.of('full')` and a `userEvent` of `vim.<command>` (for
example `vim.delete`, `vim.put`, `vim.repeat`). The `vim.` prefix does
not match the completion popup's typing trigger
(`input.type*`/`delete*`), and it does not match CodeMirror's joinable
user events.

### 3.5 Text-input gate and IME

`InputController` asks the hook whether the current mode accepts native
text: `allowsTextInput()` is true only in insert and replace modes.

When it is false (normal, visual, visual-line, operator-pending, and
during an ex/search prompt):

- A cancelable `beforeinput` of any `insert*` or `delete*` type is
  `preventDefault`ed and dropped. `historyUndo`/`historyRedo` keep today's
  handling.
- A non-cancelable native `input` mutation is not dispatched. The bridge
  is refreshed from the document instead.
- A composition that **starts** in a non-insert mode is marked
  *discarded* for its whole life. The preedit is not shown in the
  presentation, `compositionend` text is never dispatched (even if the
  mode changed in the meantime), and the bridge is refreshed afterwards.
  After `compositionstart`, the controller also ends the platform
  composition early by blurring and refocusing the bridge, so the
  candidate window does not stay open. Text committed by that refocus is
  discarded under the same rule. The status row shows
  `IME input ignored in NORMAL mode` (announced once per composition).
- `keyCode 229` / `isComposing` keydowns never reach `handleKey` (the
  existing guard), so they are never read as commands.

Insert-mode composition is unchanged. The code path is the same as
today: `beginComposition`, `updateComposition`, `endComposition` and the
`isolateHistory('full')` commit. While a preedit is active, an Escape is
consumed by the existing `cancelComposition` branch and does not leave
insert mode. A later Escape leaves it. In replace mode, an IME commit
**inserts** rather than overwrites (VQ9).

In replace mode, `allowsTextInput()` is true, and the hook also supplies
`overwriteRange(text)`. `InputController.replaceSelection` then
replaces the next `n` graphemes of the current line, where `n` is the
grapheme count of `text`, and it never crosses the line end. The
controller keeps the overwritten originals so that `Backspace` in replace
mode restores them.

Japanese users: leaving insert mode does not switch the OS IME off (the
web platform has no API for that). In normal mode, keys typed with the IME
on are ignored, and no text is inserted. VQ5 records the alternative of
reading `event.code` as commands. The README documents this.

### 3.6 Cursor-shape hook

`RenderFeedback` (`renderer.ts`) gains two optional fields:
`cursorShape?: 'bar' | 'block' | 'underline'` and `cursorEnd?: number | null`.
`cursorEnd` is the next grapheme boundary after the cursor; `mount.ts`
computes it with the existing boundary helpers. The overlay cache key
includes both fields.

`cursorRect` in `code/cursor-shape.ts` maps the caret rect from
`layout.coordsAtPos(cursor)` and the next rect `coordsAtPos(cursorEnd)` to
the quad:

- `bar` (or field absent): today's rect, 1px wide. This keeps the
  default-off path pixel-identical.
- `block`: from the caret's left edge to the next boundary on the same
  line, full line height. At the line end, on an empty line or across a
  line break, it is 8px wide (the same convention as `rangeRects`). It is
  filled with the palette cursor color at 45% alpha so the glyph stays
  readable.
- `underline`: same width as block, 2px tall at the bottom of the cell.

Shapes by mode: normal and visual use block, insert uses bar, replace
and operator-pending use underline. Visual selections are sent as the
existing `selection` annotation, so they need no renderer change. No
other part of `renderer.ts` or `atlas.ts` changes; a sibling branch is
editing those files.

### 3.7 Status row, ex row, toggle and setting

- **Setting.** The key is `vactr.vimMode` with value `'true'`/`'false'`.
  It is read and written through `pageStorage` (`ui/shell.tsx`), and every
  access is wrapped in try/catch; if storage is blocked, the state lasts
  for the page load. "Per viewer" means per browser profile and origin,
  and the Tauri webview has its own store. A missing or invalid value
  means off.
- **Toggle.** One square text button labelled `vim` in the transport
  toolbar (`.vact-vim-toggle`, `aria-pressed`, title
  `Vim mode: on|off`). It uses the existing icon-button token styles, so
  it is reachable by touch on iPad. Turning it on attaches the controller
  in normal mode. Turning it off disposes the controller: the mode, the
  pending command, the macro recording and the registers are dropped, the
  status row is removed, and the cursor returns to a bar. The document
  and the current selection are not changed. Focus then returns to the
  code bridge.
- **Status row.** It exists only when Vim is on. `.vact-code` gets
  `data-vim='on'` and becomes a two-row grid: the canvas, then the row.
  The row (`.vact-vim-status`, `data-mode=normal|insert|visual|visual-line|replace|operator-pending`)
  shows, from left to right, the mode label (`NORMAL`, `INSERT`,
  `VISUAL`, `VISUAL LINE`, `REPLACE`; operator-pending shows `NORMAL`),
  the pending keys (for example `"a2d`), `recording @q` while a macro is
  recording, the active search (`/pat`) and the last message. Styling is
  flat and square, uses the monospace and spacing tokens, and keeps 8px
  between separate controls (design-ui-style.md).
- **Mode label as a button.** The label is a `<button>`. Tapping it in a
  non-insert mode enters insert mode, and tapping it in insert or replace
  mode acts as Escape. On-screen keyboards have no Escape key, so this
  keeps on-screen typing usable with Vim on.
- **Ex and search row.** Pressing `:`, `/` or `?` replaces the message
  area with a flat `<input type="text">` (`.vact-vim-ex`) that has the
  prefix as a label and receives focus. `Enter` runs the command,
  `Escape` or `Ctrl-[` cancels, and blur cancels. In every case focus
  returns to the bridge. `:` from visual mode pre-fills `'<,'>`. The
  input is a native text field, so IME works in it.

## 4. Vim semantics

The behavior is written from scratch, with Vim used only as a behavioral
reference. No code from Vim (Vim license) or GPL sources is used.
`@replit/codemirror-vim` (MIT) is neither a dependency nor a source. If
an implementer adapts any permissive code, it must be recorded in
`THIRD_PARTY_NOTICES.md`.

### 4.1 Mode transitions

| From | Keys | To |
|------|------|----|
| normal | `i a I A o O s S C` and `c{motion}`, `cc` | insert (a change starts) |
| normal | `R` | replace |
| normal | `v` / `V` | visual / visual-line |
| normal | an operator key | operator-pending |
| operator-pending | a motion or text object | normal, or insert for `c` |
| operator-pending | Escape, an invalid key | normal (pending cleared) |
| visual(-line) | `v`/`V` (the same key) or Escape | normal |
| visual(-line) | `v` <-> `V` | the other visual mode |
| visual(-line) | an operator, `x X D C S Y J r ~ p P` | normal or insert |
| insert / replace | Escape or `Ctrl-[` | normal (the cursor moves one grapheme left, but not past the line start) |
| any | an external non-empty selection (pointer drag, `Mod-a`) seen in normal mode | visual |

Every mode change updates the snapshot and announces the new mode label
politely through `AccessibilityBridge.announce`. Pending-key changes are
not announced.

### 4.2 Keys and the command grammar

`keys.ts` reduces a key input to a `VimKey` using `event.key`, which
follows the keyboard layout. `Shift` is part of the produced character.
`Ctrl-[` is detected by `ctrlKey` together with `key === '['` or
`code === 'BracketLeft'`, and it becomes `<Esc>`. A count is
`[1-9][0-9]*`, where a leading `0` is the `0` motion. Counts multiply
(`2d3w` = `d6w`) and are clamped to 10,000. A register prefix is
`"{a-zA-Z0-9_+*"}`; any other register character cancels the pending
command with a message.

### 4.3 Position model

- **Normal mode.** The cursor is the character at `selection.main.head`.
  If the head is at the end of a non-empty line, the cursor is shown on
  the last grapheme, and commands read that clamped position. The stored
  selection is not rewritten, so no dispatch happens inside a surface
  subscriber (`CodeSurface` forbids re-entrant dispatch).
- **Visual mode.** The controller keeps an inclusive `anchor` and `head`
  as grapheme start positions. It dispatches the CodeMirror selection as
  `{anchor, head + graphemeLength}` when the head is at or after the
  anchor, and as `{anchor + graphemeLength, head}` otherwise. Visual-line
  dispatches from `line(min).from` to `line(max).to`. The display cursor
  is the inclusive Vim head.
- **Desired column.** It is measured in graphemes from the line start,
  with a tab counting as one column (VQ7 records the visual-column
  alternative). It is kept across `j k Ctrl-d Ctrl-u Ctrl-f Ctrl-b H M L G gg`
  and reset by every horizontal motion. `$` sets it to infinity.
- All positions step by grapheme with the existing `boundary` helpers, so
  surrogate pairs and combining marks are never split.

### 4.4 Word classes

A *keyword* character is a Unicode letter or number, or `_`. A `-`
between two keyword characters also counts, so kebab-case `.vact`
identifiers such as `sound-kit` are one word. A *punctuation* run is any
other non-blank character. Blanks are space and tab. `w b e ge` use the
three classes; `W B E` split on blanks only. An empty line counts as a
word for `w`/`b`.

### 4.5 Motions

| Motion | Type |
|--------|------|
| `h l 0 ^ $ f F t T ; , w b e ge W B E` | charwise. `e E f t $ %` are inclusive, the others exclusive. `cw` on a non-blank acts as `ce` |
| `j k gg G {n}G H M L Ctrl-d Ctrl-u Ctrl-f Ctrl-b + Enter` | linewise |
| `{ }` | charwise exclusive, to blank-line paragraph boundaries |
| `%` | inclusive; it jumps to the matching `()[]{}` from the first bracket at or after the cursor on the line |
| `* #` | exclusive; it searches for the keyword under the cursor as a whole word, forward or back |
| `/ ? n N` | exclusive; section 4.12 |

`Ctrl-d`/`Ctrl-u` scroll by half the viewport rows (or by count) and
move the cursor by the same number of lines. `Ctrl-f`/`Ctrl-b` scroll by
`rows - 2`. `H M L` use `viewport()`. Bracket and paragraph scans are
bounded to 65,536 characters in each direction. A motion that fails
(for example `f` with no match) cancels the whole command. Like Vim, an
exclusive charwise motion that ends at column 0 of a later line is
adjusted to end at the end of the previous line for operators.

### 4.6 Operators and commands

- `d` deletes. `c` deletes, then starts insert. A linewise `c` keeps one
  empty line with the first line's indent. `y` yanks and leaves the
  cursor at the start of the range.
- `>`/`<` shift the covered lines by one indent unit: two spaces, which
  matches the formatter's minimum indent unit (completion C5). `<` removes
  up to one unit of leading spaces or one tab. Blank lines are not
  indented.
- `g~ gu gU` toggle, lower or upper the case with `String` case mapping
  per grapheme.
- `x X s S D C Y ~ J r{c}` follow Vim. `Y` is linewise, like `yy`. `J`
  joins lines with one space and drops the next line's leading blanks.
  `r{c}` with a count replaces that many graphemes, but only if they
  exist on the line.
- `p`/`P` put after or before the cursor (charwise) or below or above the
  line (linewise). A count repeats the text. The cursor ends where Vim
  puts it.
- `o`/`O` open a line with the current line's leading whitespace.
- Each operator application is one dispatch and one undo step. The
  operator writes its register (4.8) before dispatching.

### 4.7 Text objects

- `iw aw iW aW`: a run of one word class, plus trailing blanks for `a`
  (or leading blanks when there are no trailing ones). A count extends
  over that many words.
- Quotes `" ' `` ` ``: within the cursor line only. The pair is the
  closest quote pair containing the cursor, or the first pair after it
  on the line. A backslash escapes a quote. `a` includes the quotes and
  trailing blanks.
- Brackets: the innermost pair enclosing the cursor, found with nesting
  across lines within the 65,536-character bound. Strings and comments are
  not treated specially (VQ4). `i` on a multi-line pair excludes the
  newline after the opening bracket and the indent before the closing
  bracket. A count selects the n-th enclosing pair.
- In visual mode, a text object sets the selection; in operator-pending
  mode it gives the range.

### 4.8 Registers and clipboard

- A register holds `{ text, linewise }`, in memory only, for the
  controller's life.
- A yank writes `"` and `0`. A delete or change writes `"` only (the
  numbered registers are excluded, VQ4). `"x` writes `x`, and `"X`
  appends to `x` (a linewise append adds a line break). `_` discards.
- `+` and `*` are the same system clipboard. A write is
  `clipboard.write(text)` (async, not awaited by the command); a failure
  is announced as `clipboard unavailable; kept in the internal register`.
  Both registers also keep an internal mirror. A read (`"+p`) calls
  `clipboard.read()` and applies the put when the promise resolves, but
  only if the document (`Text` identity), the selection and the mode are
  unchanged. Otherwise it is dropped with a message. While a macro or dot
  repeat is replaying, a `+`/`*` read uses the mirror synchronously so the
  replay stays deterministic. The text of a resolved `"+p` is stored in the
  redo record.
- The adapter (`vim-clipboard.ts`) uses `navigator.clipboard.readText` and
  `writeText`. They are called from the keydown handler, so the browser
  sees a user gesture (WKWebView may show its paste confirmation). If the
  API is missing or rejects, the result is a rejected promise and the
  mirror is used. No Tauri clipboard plugin is added (VQ8). The platform
  `copy`/`cut`/`paste` events in `InputController` are unchanged.

### 4.9 Undo units

CodeMirror history (`@codemirror/commands` 6.11.1, `HistoryState.addChanges`)
joins a joinable `input.type*`/`delete*` change into the previous event
only when three conditions hold: the change comes within `newGroupDelay`
(500 ms) of that event, it is adjacent to it, and that event has no
recorded selections after it. Bridge `selectionchange` dispatches and
pauses in typing therefore split one insert session into several events.
Without isolation, the first typed text could also join an earlier event. Vim keeps the existing history field and does not
read or edit its internals. Instead `vim/undo.ts` keeps a ledger:

- Entering and leaving insert or replace mode dispatches a boundary
  transaction (selection only, `isolateHistory.of('full')`). This stops
  joining across the session boundary. Vim commands are isolated already
  (3.4).
- On leaving the session, the ledger stores
  `{ entryDepth, exitDepth, exitDoc }`, where the depths are
  `undoDepth(state)` and `exitDoc` is the `Text` identity. A `c`
  operator's entry depth is taken before its delete. A dot repeat or a
  counted insert is also one unit.
- `u` (with a count n, repeated n times): if the top unit's
  `exitDepth === undoDepth(state)` and `exitDoc === state.doc`, it calls
  `host.undo()` `exitDepth - entryDepth` times, stopping early if an
  undo returns false. It then moves the unit to the redo ledger with the
  resulting `Text` and the redo depth. Otherwise it
  undoes one history event, which covers edits from outside Vim such as
  format, completion in normal mode or `Mod-z`. `Ctrl-r` mirrors this
  with `redoDepth`.
- Any document change that is not an undo or redo clears the redo
  ledger. If the history bound trims events (`retentionStatus.reducedDepth`
  becomes true), the whole ledger is cleared, and later undos fall back to
  one event at a time.
- `Mod-z` / `Mod-Shift-z` / `Ctrl-y` remain single-event platform undo.

### 4.10 Dot repeat and macros

Both are stored as token lists in `vim/record.ts`:

- `key` token: a `VimKey` handled in a non-insert mode.
- `edit` token: `{ deleteBefore, deleteAfter, insert }` relative to the
  cursor. It is captured from surface updates while insert or replace mode
  is active, for changes with user events `input.type*`, `delete*`,
  `input.paste` and `input.complete`. Typed text, IME commits, Enter,
  Tab, Backspace, Delete, paste and accepted completions are therefore all
  recorded the same way.
- `move` token: an arrow, `Home` or `End` key in insert mode, recorded as
  the matching motion.

**Dot.** The redo record starts with the key that starts a change (an
operator, or `x p J r ~ > <`, or an insert entry) and ends when control
returns to normal mode. In insert mode, a `move` token or a change that
does not touch the cursor (for example a pointer click followed by
typing) closes the record. Typing continues a new record that starts as
an implicit `i` at the new place, as in Vim. `.` replays the record, and
a count replaces the original count. Replay is one undo unit with
`userEvent: 'vim.repeat'`.

**Macros.** `q{a-z}` starts recording into that register as a token
list, and `q` stops. The register's text also holds the keys, so `"ap`
shows them. `@{reg}` replays and `@@` repeats the last one. `{n}@a`
replays n times. Replay stops at the first failing motion. Each register
keeps its token list next to its text; a register written by a yank holds
only text, which is replayed as keys. Limits: nesting depth 100 and
100,000 tokens per top-level replay. Going over the limit stops the replay
with a message.

The completion popup is not replayed, because accepted completions are
recorded as edits.

### 4.11 Marks

`m{a-z}` stores a position. Every document change maps it with
`ChangeSet.mapPos(pos, -1)`. `'{a}` jumps to the first non-blank of the
mark's line (linewise), and `` `{a} `` jumps exactly (exclusive). Both
work as operator motions. Jumps (`G gg % / ? n N * #`, mark jumps, `:{n}`)
set the context mark, which `''` and ``` `` ``` use. An unset mark gives
`E20 Mark not set`.

### 4.12 Search

- The pattern dialect is JavaScript `RegExp` source with the `u` flag
  (VQ6). Search is case-sensitive. A trailing `\c` makes it
  case-insensitive. If the pattern does not compile, it is searched as a
  literal string, with a status note. An empty pattern reuses the last
  one.
- `/` searches forward and `?` backward, wrapping around the end of the
  document. On a wrap, `search hit BOTTOM, continuing at TOP` is shown;
  with no match, `E486 Pattern not found`. `n` repeats in the same
  direction and `N` in the opposite. `*`/`#` use the escaped keyword
  wrapped in keyword-boundary lookarounds built from the 4.4 class.
- Matches are found on the whole document string. The string is cached
  per document `Text` identity, so a repeated `n` on an unchanged
  document does not rebuild it.

### 4.13 Ex commands

The range forms are: none (the current line), `%`, `{n}`, `{n},{m}`, `.`,
`$`, `.+{n}`, and `'<,'>` (the last visual selection's lines).

| Command | Effect |
|---------|--------|
| `:{n}`, `:$` | go to line n or the last line, first non-blank; sets the context mark |
| `:[range]s/pat/rep/[g][i]` | substitute on the range. Any non-alphanumeric delimiter works. In `rep`, `&` and `\0` are the whole match, `\1`-`\9` are groups, and `\&`, `\\` and `\{delim}` are literals; a JS `$` is literal. It is one dispatch (one undo step), with the message `N substitutions on M lines` |
| `:noh`, `:nohlsearch` | clear the active-search indicator (no highlight is drawn; VQ3) |
| `:w :q :wq :x :q!` | no-op, with the message `vactr syncs the document live; use :eval, :evalall or :hush` (VQ2) |
| `:eval` | `evalAtCursor` (same as `Mod-Enter`) |
| `:evalall`, `:run` | `evalAll` (same as `Mod-Shift-Enter`) |
| `:hush` | `hush` (same as `Mod-.`) |
| `:format` | `formatDocument` (same as `Shift-Alt-f`) |
| anything else | `E492 Not an editor command: <text>` |

### 4.14 Live-coding shortcut matrix

`Mod` is `Ctrl` on Windows and Linux (and in the jsdom test default) and
`Meta` on Apple platforms, as in `matchesKey`. "Passed" means
`handleKey` returns false and the existing handler runs (3.3.1).

| Shortcut | normal | visual | op-pending | insert | replace |
|----------|--------|--------|------------|--------|---------|
| `Mod-Enter`, `Mod-Shift-Enter`, `Mod-.` (P1: Ctrl on Windows/Linux, Meta on Apple) | eval / eval all / hush | same | same; the pending command is kept | same | same |
| KeyboardController fallbacks (P2): `Ctrl-Enter`, `Ctrl-Shift-Enter`, `Ctrl-.` on Apple; `Meta-Enter`, `Meta-Shift-Enter`, `Meta-.` on Windows/Linux | eval / eval all / hush | same | same; the pending command is kept | same | same |
| `Shift-Alt-f` (P1) | format | format | format | format | format |
| (`Meta` or `Ctrl`) `a`, `z`, `Shift-z`; `Ctrl-y` (P2) | select all (enters visual, 4.1) / undo / redo | same | same | same | same |
| (`Meta` or `Ctrl`) `c`, `x`, `v`; (`Meta` or `Ctrl`) `\` (P3) | copy / cut / paste; side-pane fold | same | same | same | same |
| `Backspace`/`Delete`/`Tab` with `Alt`, `Ctrl` or `Meta` | consumed, no edit | consumed | consumed | passed (today's behavior) | passed, except plain `Backspace` (restore) |
| `Ctrl-Space` | opens the popup | opens the popup | opens the popup | opens the popup | opens the popup |
| popup open: arrows, PageUp/PageDown, Enter, Tab | popup | popup | popup | popup | popup |
| popup open: Escape / `Ctrl-[` | closes the popup | closes the popup | closes the popup | closes the popup (insert is left on the next one) | same as insert |

Evaluation uses the form at `selection.main.head`. In visual mode that is
the selection's exclusive end.

## 5. Accessibility

- Mode changes and messages go to the existing polite live region
  (`AccessibilityBridge.announce`). Errors (`E...` messages, clipboard
  failures) are assertive.
- The textarea selection keeps mirroring the CodeMirror selection, so a
  screen reader reads visual selections.
- The toggle and the mode-label button have accessible names and
  `aria-pressed` / `aria-live` where they apply. The ex input is labelled
  `vim command` or `vim search`.

## 6. iPad and touch

Vim mode is for hardware keyboards. With Vim on and only the on-screen
keyboard, the user taps the mode-label button to enter insert mode and to
leave it (there is no Escape key on screen). The transport toggle stays
reachable to turn Vim off. Pointer behavior (tap to place the caret,
selection handles, numeric drag) is unchanged in every mode. A non-empty
handle selection made in normal mode enters visual mode (4.1).

## 7. Errors and limits

- The core never throws on user input. A failing motion, an unknown key,
  an unset mark or register, or a bad pattern cancels the pending command
  and sets a status message.
- Counts are capped at 10,000. Loops stop early when a motion stops
  moving. Scans are bounded (4.5). Replay is limited (4.10).
- When `surface.compositionRange` is set (a composition is active), the
  controller makes no changes, and a keydown that reaches it is ignored.

## 8. Testing and verification

### 8.1 Unit tests (vitest, pure core, `editor/test/code/vim/`)

The tests are table-driven. Each row is
`{ doc with | cursor or [anchor|head] marks, keys, expected doc, expected cursor, expected mode, expected register? }`
and runs through an in-memory `VimHost` over a real `EditorState` that
has `history()`.

- `keys.test.ts`, `parser.test.ts`: normalization, counts, registers,
  the pass-through set P for `platform: 'apple'` and `'other'`, with a
  stub `matchesDefaultKeymap` (P1 passed; P2 Meta and Ctrl variants
  passed on both platforms; P3 passed); consumption in non-insert modes
  of modifier `Backspace`/`Delete`/`Tab`, unbound `Ctrl`, `Alt` and `Meta`
  chords and printable keys; and Vim's control chords
  `Ctrl-r/d/u/f/b/[`.
- `motions.test.ts`: every motion in 4.5 with counts, grapheme and
  surrogate lines, the desired column, empty lines, viewport motions with
  a fake viewport.
- `operators.test.ts`: every operator by motion class, the doubled forms,
  the shortcut commands, indent shifting.
- `textobjects.test.ts`: word, quote and bracket objects, counts,
  multi-line brackets.
- `registers.test.ts`: named, append, `0`, `_`, clipboard success,
  failure, a stale async read, the replay mirror.
- `undo-repeat.test.ts`: one insert session is one `u` (including typing
  with more than 500 ms between keys, simulated with `Transaction.time`),
  `cw` + typing is one unit, external edits are single-step, redo clears,
  dot repeat of `ciw`, `A;`, `3x`, `dd`, a counted insert, IME-commit
  edit tokens.
- `macro-marks.test.ts`: recording and replay, `@@`, nesting limits,
  marks mapped through edits, `''`.
- `search-ex.test.ts`: `/ ? n N * #`, wrap, a literal fallback, `\c`, the
  `:s` ranges, flags and replacement escapes, `:{n}`, the action
  commands, unknown commands.
- `purity.test.ts` (node environment): no file under `src/code/vim/`
  references `document`, `window`, `navigator`, `localStorage`,
  `HTMLElement` or `@codemirror/view`.

### 8.2 Integration tests (`editor/test/canvas/`)

- `vim-input.test.ts`, through `InputController` with jsdom:
  - default off: with no hook, the existing typing, beforeinput,
    composition and keymap sequences give the same documents and history
    depths as in `input.test.ts`;
  - normal mode: printable keys, Tab and `beforeinput insertText` do not
    change the document;
  - normal mode: a `keyCode 229` keydown plus a full
    `compositionstart`/`update`/`end` sequence leaves the document and
    revision unchanged and shows no preedit; a composition started in
    normal mode that ends after `i` is still discarded;
  - insert mode: the Japanese composition commits exactly once as one
    undo group (the same assertions as the existing IME test);
  - Escape with the popup open closes the popup and stays in insert, and
    the next Escape gives normal; `Ctrl-[` does the same;
  - live-coding shortcuts, parameterized over the platform (stub
    `navigator.platform`) and over every mode. On a non-Apple platform
    (`Linux x86_64`): `Ctrl-Enter`, `Ctrl-Shift-Enter` and `Ctrl-.` reach
    evalAtCursor, evalAll and hush. `Ctrl-a` selects all, `Ctrl-z`
    undoes and `Ctrl-y` redoes. On an Apple platform (`MacIntel`): the
    `Meta` variants do the same, and the `Ctrl-Enter` / `Ctrl-.` fallbacks
    also reach evalSelection and hush. On both, `Shift-Alt-f` reaches the
    formatter and `Ctrl-Space` opens the popup. Each case checks that
    `handleKey` returned false and that the default was not prevented by
    Vim;
  - non-insert modes on both platforms: `Alt-Backspace`,
    `Meta-Backspace`, `Ctrl-Backspace`, `Alt-Delete`, `Meta-Delete`,
    `Shift-Tab` and plain `Tab` leave the document and revision
    unchanged; the same chords in insert mode behave as with Vim off;
  - Vim control chords: `Ctrl-r` redoes a unit and `Ctrl-d` scrolls in
    normal mode on both platforms; in insert mode they are passed;
  - replace mode overwrites and `Backspace` restores.
- `vim-mount.test.ts`: the toggle writes and reads `vactr.vimMode`, a
  throwing storage is tolerated, the status row is absent when off and
  shows `data-mode` when on, `render()` gets `cursorShape` and the clamped
  cursor per mode, toggling off restores the bar cursor and removes the
  row.
- `cursor-shape.test.ts`: `cursorRect` for bar, block and underline at
  mid-line, the line end, an empty line and a wide grapheme.
- Mechanical updates: existing tests that count transport buttons or
  snapshot the toolbar (for example `test/ui/controls.test.ts`) are
  updated for the new toggle and listed in the plan.

### 8.3 End-to-end (Playwright)

`editor/test/e2e/vim-behavior.mjs` exports `runVimChecks(page, check, name)`.
`behavior.mjs` calls it after its existing checks. The checks, run in
Chromium and WebKit, are:

- `vim-toggle`: click `.vact-vim-toggle`, the status row shows `NORMAL`,
  the setting persists across a reload, and toggling off removes the row;
- `vim-dcyp`: on a seeded document, `dw`, `cwfoo<Esc>`, `yyp` and `P`
  give the expected text (read through `window.__vactrPerf` or the bridge
  and the surface API);
- `vim-visual`: `vjd` and `Vjy` then `p`;
- `vim-dot`: `ciwbar<Esc>`, `w`, `.`;
- `vim-ime-normal`: in normal mode, Chromium uses CDP
  `Input.imeSetComposition` / `Input.insertText` and WebKit dispatches
  synthetic composition events plus `keyCode 229` keydowns; the document
  is unchanged and the mode is still normal.

Run (requires the built `dist`; the debug wasm is used for non-gating
runs):
`cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build && npm run e2e -- --browser all --profile behavior --run-id vim-001 --out ../tmp/vim-mode/e2e/vim-001`.
Use `--out` under `tmp/` so the tracked canvas-cutover evidence is not
touched.

### 8.4 Gates

Every gate is run in the foreground, and its log is kept under
`tmp/vim-mode/`. Heavy suites run inside the measurement lock: `mkdir
/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock` before, `rmdir`
after, and wait and retry while it exists.

1. `cd editor && npm run check`
2. `cd editor && ./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/cursor-shape.test.ts`
3. Locked: `cd editor && ./node_modules/.bin/vitest run` (full)
4. Locked: `cd editor && npm run test:style`. The probe pages must include
   the toggle and, with Vim on, the status row and the ex input (square,
   non-native, spaced).
5. `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`. This uses
   the release wasm by default. When
   `target/wasm32-unknown-unknown/release/vactr.wasm` is absent, prefix the
   command with `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm`
   and record that in the notes.
6. Locked: the e2e command in 8.3
7. Rust is not touched. `CARGO_TERM_QUIET=true cargo nextest run` is
   needed only if a plan touches Rust.

Verification records use exactly
`{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
Negative-control runs (for example, temporarily removing the gate and
showing the IME test fail) are reported only as `mutationEvidence`.

## 9. Rollout constraints

- Vim mode is off by default. The disabled path must keep every existing
  test unchanged except the listed mechanical toolbar updates.
- The renderer diff is limited to the `RenderFeedback` fields, the
  overlay key and the `cursorRect` call (about 10 lines in `renderer.ts`).
  `atlas.ts` is not touched. This limits conflicts with `wf/code-render`.
- No new npm or cargo dependencies, and no Tauri capability changes.
- Every touched source file stays under 1000 lines.
- Every plan declares `artifactRoots`: `target/`, `tree-sitter-vact.wasm`,
  `editor/node_modules`, `tmp/` and the Playwright outputs.
- Documentation: the README.md `## Editor` section describes the toggle,
  the modes, the ex mappings, the IME behavior and the excluded features.
  `editor/test/e2e/README.md` describes the vim checks.
  `THIRD_PARTY_NOTICES.md` changes only if permissive code is adapted
  (none is planned).

## 10. Implementation partition guidance

Dependencies allow these waves (the plan author decides the final
split):

- Wave 0: `vim/types.ts`, `vim/keys.ts`, `vim/words.ts`, and the test host
  helper (`test/code/vim/host.ts`).
- Wave 1, in parallel: (a) motions and text objects; (b) operators and
  registers; (c) search and ex parsing; (d) record, undo and marks.
- Wave 2: `vim/parser.ts` and `vim/controller.ts`.
- Wave 3, in parallel: (a) `input.ts` and `surface.ts` routing plus the
  gate; (b) `cursor-shape.ts` and the `renderer.ts` hook; (c)
  `vim-settings.ts`, `vim-clipboard.ts` and `vim-status-view.tsx`, with
  the CSS and the toggle.
- Wave 4: `vim-mount.ts` and the `mount.ts` wiring, the integration tests,
  e2e, docs, and the full gates.

## 11. Open questions

See `design-docs/user-qa/pending-vim-mode-questions.md`:

- VQ1: visual block
- VQ2: the `:w`/`:q` mapping
- VQ3: search highlighting
- VQ4: the excluded Vim features
- VQ5: IME keys in normal mode
- VQ6: the search dialect
- VQ7: the desired-column unit
- VQ8: the Tauri clipboard
- VQ9: replace-mode IME

Keymap, hint and hop questions (KQ1-KQ7) are in
`design-docs/user-qa/pending-vim-keymap-questions.md`.

## References

- `design-docs/specs/design-implementation.md` 15.3: the canvas editor
  and input bridge
- `design-docs/specs/design-completion.md`: the popup keys and the
  `CompletionSurface`
- `design-docs/specs/design-ui-style.md`: the style rules for the toggle,
  the status row and the ex row
- `design-docs/references/README.md`: external references. Vim's
  `:help` is used only as a behavioral reference.
