# Vim Mode: JSON Keymap, Next-Key Hints and Hop Jumps

Status: Implemented (2026-10-17, workflow session 401, branch `wf/vim-keymap`;
proposed 2026-10-17, session 395). The implementation plans are archived as
`impl-plans/completed/vim-keymap-*.md`.
Extends `design-vim-mode.md`, which is implemented and stays the baseline.
Open decisions are in `design-docs/user-qa/pending-vim-keymap-questions.md`
(KQ1-KQ7). Until they are answered, implementation follows the
recommendations.

This document adds five things to the optional Vim mode of the code pane:

1. a versioned JSON keymap and settings document, with validation, live
   apply, web `localStorage` storage, Tauri app-config storage and file
   import/export;
2. a bundled `mise-darwin` preset that reproduces the user's Neovim setup
   as closely as the editor allows, plus the small set of extra built-ins
   that the preset needs (`gk gj gJ g; g, zz <C-e> <C-y>`, `ys cs ds`);
3. a which-key style next-key hint panel at the bottom left of the code
   pane;
4. hop / easymotion jumps (`hop.char1`, `hop.char1Line`, `hop.word`,
   `hop.line`) drawn through a narrow renderer overlay hook;
5. the rule that without a stored keymap document, behavior is exactly
   the merged Vim mode.

Everything in `design-vim-mode.md` (key routing, pass-through set P, IME
gate, undo ledger, registers, dot repeat, macros, ex row, status row)
still applies. This document only states what changes.

## 1. Baseline and the no-config invariant

The merged Vim mode (`editor/src/code/vim/*`, `design-vim-mode.md` section
3) routes a keydown as: completion keymaps, then `VimController.handleKey`
(only when Vim is on), then the eval/format keymaps, then
`KeyboardController`. `handleKey` checks the composition gate and the
pass-through set P, normalizes the key with `keys.ts` `normalizeKey`, and
feeds `parser.ts` `CommandParser` or the insert-session handler.

**Invariant K0.** When no keymap document is stored (web key absent, or
the Tauri file absent), or the stored text is not valid JSON or has no
`"version": 1`, the controller runs with *no keymap*:

- no mapping layer, no hint state, no hop state;
- `normalizeKey` and `CommandParser` run in their merged form (the
  `extended` option of section 5 is off);
- the renderer gets no hop labels, and no hint panel exists in the DOM.

Every existing Vim unit, integration and e2e test therefore passes
unchanged, and new parity tests pin this (section 11.1). Hints and hop are
never on without a stored document (KQ1).

When a document is stored and loads (even with skipped entries), the
controller runs with a *compiled keymap*: the mapping layer is active and
the `extended` built-ins of section 5 are on (KQ6). The settings panel's
`Reset` removes the stored document and returns to K0.

## 2. Keymap document (schema version 1)

### 2.1 Shape

The document is a JSON object. Every field except `version` is optional.

| Field | Type | Default | Rule |
|-------|------|---------|------|
| `version` | integer | required | must be `1`; otherwise the whole document is rejected (K0) |
| `leader` | string, Vim notation | `"\\"` | exactly one key after notation parsing (section 3) |
| `timeoutlen` | integer ms | `1000` | 50 to 10000 |
| `maps` | object | `{}` | keys `n`, `i`, `x`, `v`, `o`, `c`; each an array of map entries. `v` is an alias of `x` (entries are merged into `x` in document order) |
| `hints.enabled` | boolean | `false` | |
| `hints.delayMs` | integer ms | `200` | 0 to 5000 |
| `hints.triggers` | object | `{}` | keys `n`, `x`, `o` (and `v` = `x`); each an array of lhs strings in Vim notation, at most 32 per mode |
| `hop.enabled` | boolean | `false` | |
| `hop.keys` | string | `"fdjhklsagqwertyuiopzxcvbnm"` | 2 to 64 printable ASCII characters, no spaces, no duplicates (case-insensitive when `uppercaseLabels` is true) |
| `hop.uppercaseLabels` | boolean | `false` | labels are drawn in upper case; typing matches either case |
| `unsupported` | array | `[]` | documentation only: `{ mode, lhs, reference, reason }` entries listing reference mappings that vactr cannot reproduce. Not validated beyond being an array of objects; never applied |

A map entry is an object:

| Field | Type | Rule |
|-------|------|------|
| `lhs` | string | Vim notation, 1 to 16 keys after parsing, `<leader>` allowed |
| `rhs` | string | Vim notation key sequence, 1 to 256 keys, or exactly `<Nop>`. Exactly one of `rhs` and `action` is present |
| `action` | string | an action id from section 4.3 |
| `remap` | boolean | default `false` (noremap). Only valid with a key-sequence `rhs` |
| `desc` | string | at most 80 characters, no control characters; shown in hints |

A later entry with the same mode and normalized lhs replaces an earlier
one (last wins, as in a vimrc). Unknown fields in an entry or at the top
level produce a warning and are ignored.

### 2.2 Validation and errors

`vim/keymap-schema.ts` exports a pure `compileKeymap(text: string)` that
returns `{ keymap: CompiledKeymap | null, errors: KeymapIssue[] }`, where
`KeymapIssue` is `{ path, severity: 'error' | 'warning', message }` and
`path` is a JSON path such as `maps.n[3].rhs`. It never throws.

- Text over 262,144 UTF-8 bytes, non-JSON text, a non-object root or a
  missing/other `version` gives `keymap: null` and one error (K0 applies).
- Otherwise each invalid entry or field is reported and skipped, the
  rest is compiled, and `keymap` is non-null. Invalid scalar settings fall
  back to their defaults with an error.
- More than 2,000 map entries in total: the entries after the 2,000th are
  skipped with one error.
- Per-entry errors include: bad notation (section 3), an unsupported
  modifier, both or neither of `rhs`/`action`, `remap` with `action` or
  `<Nop>`, an unknown action id, an action not allowed in that mode, a
  reserved lhs (2.3), a `c`-mode entry that breaks the 4.5 rules, and a
  `:` segment in `rhs` that breaks the 4.2 rules.
- The editor always starts. Load-time issues are counted in one status
  message `keymap: N entries skipped` (info) and listed in full in the
  settings panel (section 8.1).

### 2.3 Reserved keys

The pass-through set P (`design-vim-mode.md` 3.3.1) stays first and
absolute: P keys never reach the mapping layer. So that a config never
silently fails, an lhs whose first key is one of the P2/P3 chords that
the notation can express (`<C-a>`, `<C-z>`, `<C-y>`, `<C-c>`, `<C-x>`,
`<C-v>`) is a per-entry error
`reserved for a platform or live-coding shortcut` in modes `n i x o`.
In mode `c` (the ex row is a native input outside the code bridge) only
`<C-c> <C-x> <C-v>` are reserved. The other P chords (`Ctrl`/`Meta` with
`Enter`, `.`, `\`, `Shift-z`, and the P1 surface keymaps, which use
`Mod`, `Shift` or `Alt`) cannot be written in the notation of section 3,
so they need no check. Keys produced by an `rhs` are not physical events and are
not subject to P (so `zk -> <C-y>` works, section 5).

## 3. Vim notation

`vim/notation.ts` exports `parseNotation(text, leader): VimKey[] | error`
and `formatNotation(keys): string`. It is case-insensitive inside `<...>`.

| Notation | VimKey |
|----------|--------|
| a printable character other than `<` | itself |
| `<Space>` | `' '` (the form `normalizeKey` already produces) |
| `<lt>`, `<Bar>`, `<Bslash>` | `<`, `\|`, `\` |
| `<CR>`, `<Enter>`, `<Return>` | `<CR>` |
| `<Esc>`, `<C-[>` | `<Esc>` |
| `<BS>`, `<Del>`, `<Tab>`, `<Left>`, `<Right>`, `<Up>`, `<Down>`, `<Home>`, `<End>` | the same names as `keys.ts` |
| `<C-{letter}>` | `<C-{lowercase letter}>` |
| `<leader>` | the leader key (re-expanded when `leader` changes) |
| `<Nop>` | only as the whole `rhs` |

Any other `<...>` (including `M-`, `A-`, `D-`, `S-` modifiers and
function keys) is an error with the offending token. A lone `<` that
does not start a valid token is the literal `<`, as in Vim.

When a keymap is compiled, `normalizeKey` also returns `<C-{letter}>` for
every `Ctrl`+letter chord without `Meta`/`Alt` (the merged form returns
`<Other>` for letters other than `r d u f b`). Under K0 this extension is
off (section 5).

## 4. Mapping layer

### 4.1 Placement

`vim/mapper.ts` exports a `KeyMapper` that sits between `normalizeKey`
and the existing command handling inside `VimController`:

```
handleKey: composition gate -> P check -> normalizeKey -> routeKey(key)
routeKey(key):
  -> [hop session active?  -> HopSession.feed]           (section 7)
  -> [parser awaiting an argument character?  -> processKey (built-in, unmapped)]
  -> KeyMapper.feed(key, mapMode) -> emits (key | action)* -> processKey / runAction
```

`processKey` is today's body of `handleKey` after normalization
(insert-session handling, macro stop, parser feed), unchanged. With K0 the
mapper does not exist and `handleKey` calls `processKey` directly.
`routeKey` is the single entry for every key that may be mapped: physical
keys, keys left over after a mapper resolution, remap-emitted keys and
macro replay keys (4.2, 4.6). Each key is routed only after the previous
key has been fully processed, so the parser state that `awaitingArgument`
reads is current.

The map mode is `i` in insert and replace mode, `o` in operator-pending
mode, `x` in visual and visual-line mode, and `n` otherwise.

`CommandParser` gains a read-only `awaitingArgument(): boolean`. It is
true whenever the parser's next key is a command argument or a second
character, which Vim reads without mapping (`map.txt`: only language maps
apply there). That is:

- the argument of `f F t T r m q @ ' \``;
- the register name after `"`;
- the second key after a parser-pending `g` or `z` prefix (including the
  `g` in an operator such as `g~`, and after an operator, as in `dg`);
- the text-object character after `i`/`a` in visual mode or after an
  operator;
- in the extended grammar (section 5), the pair characters of `ys`
  (after its target), `cs` and `ds`.

A key consumed as an argument never starts or extends a mapper buffer.
Operator-pending alone is not an argument state: the motion or text
object key after an operator (and the target after `ys`) stays mappable
in `o` mode. A parser prefix is reached only when the mapper has already
let the prefix key through (it starts no lhs, or the buffer resolved to
it), so `g`, `z` and `m` themselves can still begin user mappings.

### 4.2 Resolution

The mapper keeps a buffer of typed keys for the current map mode.

1. After each key, it looks up the buffer in the mode's lhs trie.
2. If the buffer is a proper prefix of at least one lhs, it waits. A timer
   of `timeoutlen` ms starts (restarted per key). While the hint panel is
   visible, the timer is suspended (section 6.3).
3. If the buffer exactly equals an lhs and is not a prefix of a longer one,
   the mapping runs at once.
4. On a timeout, or when the next key makes the buffer match nothing, the
   mapper clears its buffer, takes the longest lhs that is a prefix of the
   buffered keys and runs it. If no lhs is a prefix, the first buffered key
   goes to `processKey` (built-in). Each remaining key then re-enters the
   full 4.1 ordering through `routeKey` (hop session, then
   `awaitingArgument`, then the mapper), one at a time, not mapper step 1.
   So with the section 9.2 preset, `gg` typed in a burst runs at once: the
   buffer `gg` matches nothing, the first `g` becomes a parser-pending
   `g`, and the second `g` is its argument. In the same way `zz` runs at
   once, `mk` sets mark `k` without the `k -> gk` map, and `mgj` sets mark
   `g` and then moves down with `j` (no hop session). Neovim behaves the
   same way.
5. Running a mapping: `<Nop>` does nothing. An `action` calls the action
   registry (4.3). An `rhs` emits its keys. With `remap: false` they go
   straight to `processKey` (built-in meaning). With `remap: true` each
   emitted key goes through `routeKey` (the full 4.1 ordering, so an
   argument such as the `k` in an rhs `fk` stays literal) with a depth
   counter; depth over 100, or more than 1,000 emitted keys for one typed
   key, stops the expansion with the error `E223 recursive mapping`.
6. `rhs` command-line segments: a `:` followed by text and `<CR>` inside
   an `rhs` runs that ex command directly (the same `executeEx` path as
   the ex row) without opening the row; this is how `:nohlsearch<CR>`
   works. A `:` at the very end of an `rhs` opens the ex row empty. Any
   other `:` placement is a compile error for that entry. `/` and `?` in
   an `rhs` are not special and open the search row as typed keys do.
7. Escape is an ordinary key for the buffer (so `<Esc><Esc>` can be
   mapped). If `<Esc>` is pressed with a buffer that matches nothing, the
   buffered keys are resolved as in step 4 and then `<Esc>` is handled.

The pending area of the status row shows the parser pending keys followed
by the mapper buffer in notation (for example `<Space>` or `k`).

Buffer resolution also happens immediately (as on a timeout) when:
a composition starts, a native text input arrives without a keydown, a
surface update not caused by Vim changes the selection or document, the
bridge loses focus, the keymap is replaced, or Vim is turned off.
`InputController` calls a new optional hook method
`VimInputHook.flushPendingInput()` at the start of `beginComposition` and
before handling a `beforeinput`/`input` text mutation. This keeps the
document order right when the IME is turned on after a pending key.

### 4.3 Action registry

`vim/actions.ts` holds the action ids. Vim built-ins (motions, operators,
text objects, registers, ex commands) are not duplicated as ids: a
noremap `rhs` already reaches them with their built-in meaning (section
10, divergence D1).

| Id | Modes | Effect |
|----|-------|--------|
| `vactr.evaluate` | n x i | `host.actions.evalAtCursor()` |
| `vactr.evaluateAll` | n x i | `host.actions.evalAll()` |
| `vactr.hush` | n x i | `host.actions.hush()` |
| `vactr.format` | n x i | `host.actions.format()` |
| `vactr.diagnostic.next` | n x | moves to the start of the first diagnostic after the cursor, wrapping; `no diagnostics` when there are none |
| `vactr.diagnostic.prev` | n x | the same, backwards |
| `vactr.completion` | n x i | opens the completion popup (the adapter sends a synthetic `Ctrl-Space` to the `'highest'` keymaps, the same matcher as `closePopupWithEscape`) |
| `hop.char1` | n x o | section 7 |
| `hop.char1Line` | n x o | section 7, current line only |
| `hop.word` | n x o | section 7, word starts |
| `hop.line` | n x o | section 7, first non-blank of each line |

In visual mode the diagnostic actions extend the selection. `VimActions`
(`types.ts`) gains `diagnosticNext()`, `diagnosticPrev()` and
`openCompletion()`. The adapter implements the diagnostic ones over
`DiagnosticsController.current()` (sorted by `from`) and passes the
resulting position back, so the jump goes through the normal selection
dispatch. There is no hover feature in vactr, so no hover action exists.

### 4.4 Insert-mode mappings (for example `kj`)

Insert-mode lhs keys are only seen when the keydown passed the
composition gate, so composition (Japanese IME) bypasses the mapping
layer completely and insert-mode IME is unchanged. When the buffer is
empty and a key starts no `i` lhs, `handleKey` returns exactly what the
merged session handling returns (most keys are not consumed and reach
native input as today).

When a keydown in insert or replace mode starts or extends a pending
buffer, the controller consumes it (`preventDefault`, so no native text
input happens) and holds it. This is the deferred-insert model (KQ3):

- the buffer completes a mapping: the mapping runs (`kj` -> `<Esc>` leaves
  insert mode with nothing inserted);
- the buffer resolves to plain keys (timeout, a non-matching key, or a
  forced flush from 4.2): each held printable key is inserted through a
  new `VimHost.typeText(text)`, implemented by a new
  `InputController.typeText(text)` that uses the same path as an
  `insertText` `beforeinput` (it applies `overwriteRange` in replace mode
  and dispatches with `userEvent: 'input.type'`). The insert session,
  dot repeat and macros therefore record it like typed text. A held
  non-printable key goes through the default handling of 4.4.1. The key
  that broke the match is then processed normally (for a printable key,
  the browser inserts it after the flushed text).

#### 4.4.1 Insert-mode `rhs` keys

An `rhs` key emitted in insert or replace mode goes through
`handleSessionKey` first (so `<Esc>`, replace-mode `<BS>` and arrow move
tokens behave as typed). If that does not consume it, a printable key is
inserted with `host.typeText`, and `<BS> <Del> <CR> <Tab> <Left> <Right>
<Up> <Down> <Home> <End>` go to a new `VimHost.defaultKey(key)`. The
adapter implements it by giving `KeyboardController.handle` a synthetic
`KeyboardEvent` with that key. This uses the existing delete, newline,
indent and movement behavior and its user events. Other keys in an
insert-mode `rhs` are a compile error for that entry.

### 4.5 Command-line (`c`) mappings

The ex/search row is a native `<input>` (`vim-status-view.tsx`). Its
keydown handler consults the compiled `c` map before its own handling:

- the lhs must be exactly one key; there is no buffering and no timeout;
- the `rhs` must be exactly one key, and must be one of `<Esc>` (cancel),
  `<CR>` (submit), `<Home>`, `<End>`, `<Left>`, `<Right>`, `<BS>`, `<Del>`
  (caret and edit operations on the input) or one printable character
  (inserted at the caret);
- `action` and `remap` are compile errors in `c`.

So `<C-g>` -> `<Esc>` cancels and `<C-a>` -> `<Home>` moves to the start.

### 4.6 Dot repeat and macros

- The redo record for `.` keeps recording keys at the parser level, after
  mapping (today's `lastSequence` capture), so `.` repeats the resulting
  command, as in Vim.
- Macro recording moves before the mapper: it records the typed keys.
  Replay feeds the key tokens through `routeKey` (the 4.1 ordering) with
  timers disabled; an
  ambiguous buffer is resolved by the next token, and at the end of the
  token list it is resolved as a timeout. Edit and move tokens are
  replayed as today.
- A hop jump used as an operator target is not recorded for `.` (the
  redo record is aborted); `.` repeats the previous change (KQ5).

## 5. Extended built-ins

These are new built-ins that the preset needs. They are active only when
a keymap is compiled (`CommandParser` option `extended: true`, also passed
to `normalizeKey`), so K0 behavior is unchanged (KQ6). They are pure core
features in `vim/` and are reachable both by typing and from an `rhs`.

| Keys | Meaning |
|------|---------|
| `gj`, `gk` | the same as `j`, `k`. The code pane has no soft wrap (`layout.ts` lays out one display row per document line), so display lines are document lines. If soft wrap is added later, these must change |
| `gJ` | join lines without inserting or removing spaces (count and visual forms as `J`) |
| `g;`, `g,` | go to the older / newer position in the change list. The list keeps the start position of each document change (any source), mapped through later changes like marks; a change on the same line as the newest entry replaces it; at most 100 entries; a new change resets the walk index. Charwise exclusive motion, not a jump. Errors `E662 At start of changelist` / `E663 At end of changelist`. Lives next to the mark table in `marks.ts` |
| `zz` | scroll so the cursor line is in the middle of the viewport (`host.scrollLines`); the cursor does not move |
| `<C-e>`, `<C-y>` | scroll the view down / up by `count` lines; if the cursor would leave the viewport it moves to the nearest visible line, keeping the desired column. Physical `Ctrl-y` stays redo (P2), so `<C-y>` is reached only from an `rhs` |
| `ys{motion or text object}{c}`, `yss{c}` | surround: wrap the range (for `yss`, the current line without its indent and trailing blanks) in the pair for `c` |
| `cs{old}{new}` | change the nearest surrounding `old` pair to `new` |
| `ds{c}` | delete the nearest surrounding `c` pair |

Surround (`vim/surround.ts`), written from the observed behavior of
vim-surround, not its code:

- Pair characters: `( ) b` -> `()`, `[ ] r` -> `[]`, `{ } B` -> `{}`,
  `< > a` -> `<>`, and the quotes `"`, `'`, `` ` ``. Any other character
  cancels with no change.
- Adding with an opening bracket (`(`, `[`, `{`, `<`) puts one space inside
  each side; a closing bracket or alias letter adds none. Quotes add none.
- `ds` and `cs` find the pair with the existing text objects (`a(`,
  `a"`, ...). Deleting or changing with an opening bracket as `old` also
  trims one run of blanks inside each side.
- A linewise range (for example `ysj`) is wrapped as one charwise span
  from the first non-blank of the first line to the end of the last line
  (vim-surround would put the pair on their own lines; divergence D3).
- Each surround command is one dispatch, one undo step and dot-repeatable
  through the normal redo record.

Under K0 these keys keep their merged meaning (invalid, consumed, no
change).

## 6. Next-key hints

### 6.1 Model

`vim/hints.ts` is pure. `buildHints(keymap, mode, prefix, extended, hop)`
returns `{ prefix, entries: { key, desc, group }[] }` for the keys that can
follow `prefix` in `mode` (`n`, `x` or `o`):

- every compiled mapping whose lhs starts with `prefix` and is longer
  contributes its next key. If the lhs ends there, the entry shows the
  mapping's `desc` (or its `rhs`/`action` in notation when there is no
  `desc`). If it continues, the entry is a group with `desc` `+N keys`;
- built-in commands for the prefix come from a static table in
  `hints.ts`: `g` -> `g gg first line`, `e end of previous word`, `~ toggle
  case`, `u lowercase`, `U uppercase`, and with `extended` also `j k J ; ,`;
  `z` (extended) -> `z center line`; `m` -> `a-z set mark`. A mapping with
  the same lhs overrides the built-in entry; a `<Nop>` mapping removes it;
- mappings whose action is `hop.*` are omitted when `hop.enabled` is
  false (7.5);
- entries are sorted: single printable keys in the order
  `a-z`, `A-Z`, digits, punctuation, then special keys; groups after
  plain entries with the same key class.

### 6.2 When the panel shows

- Only when `hints.enabled` is true, in `n`, `x` or `o` map mode, when no
  count or register prefix is pending, and not during replay, composition,
  an ex/search prompt or a hop session.
- A hint session starts when the combined pending keys (parser pending
  plus mapper buffer) equal a configured trigger for the mode. After
  `delayMs` with the keys still pending, the panel shows.
- While shown, each further key that keeps a valid prefix updates the
  panel at once (multi-level prefixes, no new delay).
- The panel follows the 4.2 resolution: for a mapper-buffered `m` (the
  preset maps `mm ms mr md ma mi`) it lists those mappings and the
  built-in `a-z set mark` entry. A next key that matches no mapping (for
  example `k`) resolves the buffer, becomes the mark argument, completes
  the command and closes the panel.
- It closes when the command or mapping completes, on `<Esc>`, on a key
  that makes the prefix invalid, on a mode change, on composition start,
  when the bridge loses focus, and when Vim is turned off.

### 6.3 Interaction with `timeoutlen`

While the panel is visible the mapper timer is suspended, so the user can
read the panel (which-key behavior). Before the panel shows (inside
`delayMs`), `timeoutlen` runs normally. Parser-level pending states (for
example a bare `g`) never time out, as today.

### 6.4 View

`code/vim-hints-view.tsx` renders `.vact-vim-hints` from the snapshot:

- absolutely positioned at the bottom left of `.vact-code`, 8px from the
  left edge and 8px above the status row; `pointer-events: none`; it has
  no focusable elements and never moves focus;
- a header with the prefix in notation, then a grid of `key  desc` rows
  (CSS grid, columns of about 24ch); at most 40% of the pane height; extra
  entries collapse into a final `+N more` row;
- solid flat background from the existing tokens, square corners, the
  monospace and spacing tokens, 8px gaps (`design-ui-style.md`);
- accessibility: the grid is `aria-hidden="true"`; a visually hidden
  child `.vact-vim-hints-summary` with `aria-live="polite"` holds a
  summary such as `g: 9 next keys`, updated when the prefix changes.

The snapshot gains `hints: { prefix: string; entries: HintEntry[] } | null`.

## 7. Hop jumps

### 7.1 Flow

1. A mapping runs a `hop.*` action. The controller starts a
   `HopSession` (`vim/hop.ts`) and keeps the parser state (so an operator
   is still pending in `o` mode). The mode label does not change; the
   status pending area shows `hop`.
2. `hop.char1` and `hop.char1Line` wait for one printable key. `<Esc>` or
   a non-printable key cancels.
3. Targets are computed, labels are assigned (7.3), the snapshot carries
   them, the renderer draws them (7.4) and the status bar announces
   `hop: N targets` (polite). No target: `hop: no match`, the session ends.
4. Typed keys narrow the labels. When one label is complete, the jump runs.
   `<Esc>`, or a key that matches no remaining label, cancels with no
   movement.

While a hop session is active the controller consumes every non-P
keydown and `allowsTextInput()` is false, so IME text is never inserted
and the existing discard rule applies.

### 7.2 Targets

The search area is the visible lines from `host.viewport()`, further
limited by a new `VimHost.isVisible(pos): boolean` (the adapter checks
`layout.coordsAtPos` against the view rectangle), so horizontally clipped
positions get no label.

- `hop.char1`: every grapheme on the visible lines equal to the typed
  character. Smartcase (KQ4): a typed lowercase ASCII letter also matches
  its upper case; anything else matches exactly.
- `hop.char1Line`: the same, on the cursor line only.
- `hop.word`: the start of every keyword or punctuation run (the `w`
  classes of `design-vim-mode.md` 4.4) on the visible lines.
- `hop.line`: the first non-blank of every visible non-empty line.

The cursor position itself is excluded. At most 4,096 targets are used
(nearest first).

### 7.3 Labels

`assignLabels(targets, cursor, keys)` is pure and deterministic:

- targets are ordered by line distance from the cursor line, then by
  column distance, then forward before backward;
- the label set starts as the single keys of `hop.keys` in order. While
  it has fewer labels than targets, the last label in the list is
  replaced by itself followed by each key of `hop.keys` (in order),
  appended at the end of the list. This keeps the set prefix-free and
  gives the nearest targets the shortest labels;
- labels are given to targets in order.

Typed keys are matched case-insensitively when `uppercaseLabels` is true
and exactly otherwise. After a partial label, only targets whose label
still matches keep a label, drawn with the remaining suffix.

### 7.4 Renderer overlay hook

`RenderFeedback` gains `hopLabels?: readonly { pos: number; text: string }[]`
and `hopRevision?: number`. The overlay cache key includes `hopRevision`.
In the overlay pass, after the cursor, each label is drawn at
`layout.coordsAtPos(pos)` as a filled quad covering the label width
(`text.length` cells of the font advance) in `palette.labelFill`, with the
text in `palette.labelText` through the existing `labelRun` and
`drawRun` used for binding labels. Glyph layout, the atlas, the text and
background layers are not touched. Labels are cleared when the session
ends. `mount.ts` passes the snapshot's hop labels with the cursor fields.
The expected `renderer.ts` diff is about 10 lines.

### 7.5 Jump semantics and the off switch

- Normal mode: the cursor moves to the target and the context mark is
  set (the jump list of `''`).
- Visual and visual-line mode: the visual head moves to the target, so
  the selection extends.
- Operator-pending mode (only when the user maps a `hop.*` action in
  `o`): the pending operator, count and register (a new
  `CommandParser.pendingOperator()`) are applied to a position target. A
  forward target is inclusive (like `f`), a backward one exclusive (like
  `F`). Not dot-repeatable (4.6).
- When `hop.enabled` is false, mappings whose action is `hop.*` are left
  out of the compiled maps, as if they were not written. Their lhs falls
  back to the built-in meaning (for example `gj` moves down a line), and
  hints omit them.

## 8. Settings, storage and import/export

### 8.1 Settings panel

The Vim status row gains a square text button `keymap`
(`.vact-vim-keymap-button`). It opens `.vact-vim-keymap`
(`code/vim-keymap-view.tsx`), a flat panel inside `.vact-code` above the
status row. It contains:

- a two-button segmented preset selector `Default` / `mise-darwin`
  (square buttons, `aria-pressed`; no native `<select>`). Choosing a
  preset replaces the editor text with that preset's full document;
- a monospace `<textarea>` with the JSON (flat, square, token colors);
- buttons `Apply`, `Import`, `Export`, `Reset`, `Close`;
- an issue list (`role="list"`) with each `KeymapIssue` path and message.

`Apply` compiles the text. If `keymap` is non-null, it saves the text
and applies it live (`VimController.setKeymap`), else it shows the errors
and saves nothing. `Reset` removes the stored document and applies K0.
`Escape` inside the panel or `Close` closes it and returns focus to the
bridge. Keys typed in the panel never reach the bridge. The panel works
while Vim is on; the stored document also applies when Vim is turned on
later.

The `Default` preset document is
`{ "version": 1, "leader": "\\", "timeoutlen": 1000, "maps": {}, "hints": { "enabled": false, "delayMs": 200, "triggers": {} }, "hop": { "enabled": false, "keys": "fdjhklsagqwertyuiopzxcvbnm", "uppercaseLabels": false } }`.
Saving it is a stored document, so the extended built-ins of section 5
turn on; `Reset` is the way back to exactly the merged behavior (KQ6).

### 8.2 Live apply

`VimController.setKeymap(keymap | null)` resolves the mapper buffer, ends
any hint or hop session, sets the parser and `normalizeKey` `extended`
option and installs the new maps. The document, selection, mode,
registers, marks and macros are kept.

### 8.3 Web storage

`code/vim-keymap-store.ts` defines
`KeymapStore { get(): Promise<string | null>; set(text): Promise<StoreResult>; reset(): Promise<StoreResult> }`.

- `LocalKeymapStore(storage)` uses key `vactr.vimKeymap` in the page
  storage (`pageStorage`), with the same per-viewer scope as
  `vactr.vimMode` (per browser profile and origin). Every access is in
  try/catch; text over 262,144 bytes is refused with `too-large`; a
  blocked storage keeps the document for the page load only and shows
  `keymap not saved: storage unavailable`.
- At startup the stored text goes through `compileKeymap`; a `null`
  keymap means K0 plus an error in the panel.

### 8.4 Tauri storage

`TauriKeymapStore(invoke)` calls three commands in a new
`editor/src-tauri/src/vim_keymap.rs`, written in the pattern of
`template.rs`:

- `vim_keymap_get() -> Result<Option<String>, String>`,
  `vim_keymap_set(text: String) -> Result<(), String>`,
  `vim_keymap_reset() -> Result<(), String>`;
- the file is `vim-keymap.json` in `app_config_dir()`, a fixed name with
  no path input; writes go through `vim-keymap.json.tmp` and a rename;
  reset ignores a missing file;
- `set` checks the text is at most 262,144 bytes, parses as a JSON
  object and has `"version": 1` (integer); otherwise it returns an error
  and writes nothing. `get` applies the same check and returns `None`
  for an invalid file. Full schema validation stays in TypeScript (one
  source of truth);
- the commands are registered in `lib.rs` next to the template commands.

The store is chosen with the existing `isTauri(win)` check in
`app/main.ts`, like `TauriTemplateStore`, and passed to the code mount.
In the Tauri app the file is the only store (no `localStorage` copy). The
document loads asynchronously; until it arrives the controller runs under
K0, then it applies live.

### 8.5 Import and export

- Web: `Import` opens a `.json` file picker (File System Access API when
  present, else `<input type=file accept=.json>`); `Export` saves
  `vactr-vim-keymap.json` (save picker or download). Imported text is put
  into the editor text area and is not applied until `Apply`.
- Tauri: the dialog `open`/`save` with a JSON filter plus
  `readTextFile`/`writeTextFile`, using the existing capabilities
  `dialog:allow-open`, `dialog:allow-save`, `fs:allow-read-text-file`,
  `fs:allow-write-text-file`. No capability change.
- Both reuse the plugin loading, picker and download helpers of
  `platform/files.ts` (exported as small helpers, no behavior change for
  `.vact` files). Imported text over 262,144 bytes is refused.

## 9. mise-darwin preset

### 9.1 Source and transcription rule

The preset reproduces
`/Users/taco/gits/tacogips/mise-darwin/dotfiles/.config/nvim/lua/taco/core/keymaps.lua`,
`core/options.lua` (leader) and `plugins/init.lua` (hop.nvim, mini.clue,
vim-surround). It ships as `editor/src/code/vim/presets/mise-darwin.json`
(a document of section 2, checked by a unit test that it compiles with
zero issues), and `presets/default.json` holds the `Default` document.

The design author's sandbox could not read the reference files, so the
table below comes from the issue text. The preset task must read the
three files and classify every mapping in them: (a) reproduced, (b) mapped
to the nearest vactr action from 9.3, or (c) listed in `unsupported` with
a reason. If the files cannot be read during implementation, the preset
ships with exactly this table, and the plan records the gap (KQ2).

### 9.2 Settings and mappings

Settings: `leader` `<Space>`; `timeoutlen` from `options.lua` if it sets
one, else `1000`; `hints.enabled` true, `delayMs` 200, triggers
`n: ["<Space>", "g", "m", "z"]`, `x: ["<Space>", "g"]`; `hop.enabled`
true, `keys` `fdjhklsagqwertyuiopzxcvbnm`, `uppercaseLabels` false.

| Mode | lhs | rhs / action |
|------|-----|--------------|
| i, x | `kj` | `<Esc>` |
| n | `k` | `gk` |
| n | `J` | `gJ` |
| n | `+` | `g;` |
| n | `mm` | `%` |
| n | `zc` | `zz` |
| n | `zj` | `<C-e>` |
| n | `zk` | `<C-y>` |
| n | `<Esc><Esc>` | `:nohlsearch<CR>` |
| n | `Q gQ gq g% gx g0 g8 ga g^ g_ g~ za zA zo` and the rest of the reference set | `<Nop>` |
| n, x | `<leader>y`, `<leader>Y`, `<leader>p`, `<leader>P` | `"+y`, `"+Y`, `"+p`, `"+P` (exact rhs from the reference) |
| i | `<C-d>`, `<C-h>`, `<C-f>`, `<C-b>` | `<Del>`, `<BS>`, `<Right>`, `<Left>` |
| c | `<C-g>`, `<C-a>` | `<Esc>`, `<Home>` |
| n | `ms`, `mr`, `md` | `ys`, `cs`, `ds` |
| n | `ma`, `mi` | text-object selection as in the reference (expected `va`, `vi`) |
| n | `gn`, `<C-g>` | `vactr.diagnostic.next` |
| n | `gp`, `<C-s>` | `vactr.diagnostic.prev` |
| n, x | `gj` | `hop.char1` |
| n, x | `gf` | `hop.char1Line` |

Every entry carries a `desc` for the hint panel.

Consequences the preset accepts, as in Neovim with the same maps: in
visual mode a single `k` waits up to `timeoutlen` (it is the prefix of
`kj`) before moving up, and in normal mode a single `<Esc>` waits up to
`timeoutlen` (prefix of `<Esc><Esc>`) before it clears the pending
command. Both are unit-tested.

### 9.3 Nearest actions and unsupported entries

| Reference feature | Preset handling |
|-------------------|-----------------|
| LSP format | `vactr.format` |
| QuickRun (run the buffer) | `vactr.evaluateAll` |
| LSP completion trigger | `vactr.completion` |
| LSP hover, LSP restart, file write/quit, Chilla, telescope pickers, window/buffer/tab commands | `unsupported` with a reason (write/quit stay the `:w`/`:q` no-op message of VQ2) |
| insert `<C-v>` paste from `+` | `unsupported`: `Ctrl-v` is reserved for the platform paste (P3); `Mod-v` pastes the clipboard (KQ7) |

## 10. Intentional divergences from Vim and the plugins

- D1. Action ids cover only features without Vim keys (`vactr.*`,
  `hop.*`). Vim motions, operators, text objects, registers and ex
  commands are reached with a noremap `rhs` (`"+y`, `:nohlsearch<CR>`).
- D2. `gj`/`gk` are plain line motions (no soft wrap).
- D3. Linewise surround wraps one charwise span; tags (`t`), function
  surrounds and visual `S` are not provided.
- D4. P keys can never be remapped (2.3); `<C-y>` scroll is only reachable
  from an `rhs`.
- D5. Insert-mode multi-key mappings defer the first key (Vim-like);
  they never see composition keys, so `kj` typed through an active IME is
  IME input.
- D6. The hint panel suspends `timeoutlen` while visible.
- D7. Hop smartcase, label order and the prefix-free label rule are this
  design's own; nothing is copied from hop.nvim, mini.clue, which-key or
  vim-surround. If permissive code is adapted after all, it is recorded
  in `THIRD_PARTY_NOTICES.md`.

## 11. Testing and verification

### 11.1 Unit tests (`editor/test/code/vim/`, pure core)

- `notation.test.ts`: every row of the section 3 table, case
  insensitivity, `<leader>` expansion and re-expansion, the literal `<`,
  rejected modifiers and unknown tokens, `formatNotation` round trips.
- `keymap-schema.test.ts`: version handling, the size limit, per-entry
  errors with paths (each 2.2 rule), last-wins duplicates, `v` alias,
  defaults, reserved keys per mode, `c` rules, `:` segment rules, the
  2,000-entry cap, warnings for unknown fields, both presets compile with
  zero issues.
- `mapper.test.ts` (fake timers through an injected timer host): noremap
  vs remap, `<Nop>`, prefix waiting and timeout, the longest-prefix
  fallback, `<Esc><Esc>`, leader, the recursion limit, argument
  characters bypassing maps, forced flushes, replay without timers.
  Re-entry regressions (4.1, 4.2 steps 4 and 5), with a controller stub
  that reports `awaitingArgument`: leftover keys after a built-in flush
  and remap-emitted keys go through `routeKey`, never mapper step 1, and
  an argument key never starts a buffer.
- `hints.test.ts`: entries and groups for multi-level prefixes, built-in
  table merge, override and `<Nop>` removal, hop omission when disabled,
  ordering.
- `hop.test.ts`: targets for each action (smartcase, current line,
  visible filter, cursor excluded, the 4,096 cap), label assignment for
  fewer and more targets than keys (prefix-free, nearest shortest),
  narrowing, `uppercaseLabels`.
- `extended.test.ts`: `gj gk gJ g; g, zz <C-e> <C-y>`, surround add,
  change and delete with each pair class and spacing rule, dot repeat and
  undo of surround, and the same keys under K0 leaving the document and
  mode unchanged.
- `keymap-controller.test.ts`: through `VimController` with the test
  host: kj escape (with a deferred `k` flushed on timeout and on a
  non-matching key, recorded in `.` and macros), `<leader>y` with the `+`
  register, `+`, `mm`, the z maps, diagnostics actions with a stub,
  hint snapshot open/update/close, hop jump in normal and visual mode,
  `d` with an `o`-mode hop map, hop disabled falling back, `setKeymap`
  live replace. Argument and second-key regressions with the
  `mise-darwin` preset, in `n` and (where it applies) `x` mode: `gg` and
  `zz` typed in a burst run without the timer firing (fake timers not
  advanced); `mk` sets mark `k` and the cursor does not move; `mgj` sets
  mark `g` and moves down one line with no hop session; `fk` and `tk` use
  `k` as the literal argument (no `k -> gk`); a test-only `remap: true`
  entry with rhs `fk` also uses `k` literally.
- Parity: `keymap-parity.test.ts` runs the existing controller key tables
  (a representative set from `controller.test.ts`, `parser.test.ts` and
  `keys.test.ts`) with no keymap and asserts identical documents, cursors,
  modes and snapshots; `normalizeKey` with `extended: false` returns
  `<Other>` for `Ctrl-h`.
- `purity.test.ts` covers the new `vim/` files (no DOM globals, no timers
  other than the injected host).

### 11.2 Integration tests

- `editor/test/canvas/vim-keymap-input.test.ts` through
  `InputController` with jsdom: insert-mode `kj` leaves insert with no
  text; `k` then a pause inserts `k`; `k` then a Japanese composition
  flushes `k` first and commits the composition once as today; normal-mode
  composition is still discarded with a keymap; `c`-mode `<C-g>`/`<C-a>`
  in the ex input; P keys still pass with a keymap that tries to map them
  (rejected at compile).
- `vim-keymap-mount.test.ts`: the panel opens from the status row,
  preset buttons fill the text, `Apply` with errors saves nothing and
  lists paths, `Apply` valid saves `vactr.vimKeymap` and applies live,
  `Reset` removes it, throwing storage is tolerated, the hint panel
  appears after the delay with the summary text, hop labels reach
  `render()`.
- `editor/src-tauri/src/vim_keymap.rs` unit tests: round trip in a temp
  dir, size and version refusal, invalid file read as `None`, reset of a
  missing file.
- Style: `test/style/ui-style.mjs` probes include the `keymap` button,
  the settings panel (segmented buttons, textarea, action buttons, issue
  list) and the hint panel: square, non-native, spaced.

### 11.3 End-to-end (Playwright)

`editor/test/e2e/vim-keymap.mjs` exports `runVimKeymapChecks(browser,
origin, name)`. `behavior.mjs` calls it after `runVimChecks`. Checks, in
Chromium and WebKit, with Vim enabled through `vactr.vimMode`:

- `vim-keymap-preset`: open the panel, choose `mise-darwin`, `Apply`;
  `vactr.vimKeymap` is stored and the issue list is empty;
- `vim-keymap-kj`: `i`, type `ab`, then `kj`; the text has `ab` and the
  mode is `NORMAL`;
- `vim-keymap-hints`: press `Space`, wait 300 ms; `.vact-vim-hints` is
  visible, lists the leader entries (`y`, `p`, ...) and the summary is
  non-empty; `Escape` closes it;
- `vim-keymap-hop`: on a seeded document, `gj`, a character, then the
  first label; the cursor is at that target (read through the surface
  API or `window.__vactrPerf`);
- `vim-keymap-hop-off`: apply the preset with `hop.enabled: false`; `gj`
  starts no hop session and moves the cursor down one line.

### 11.4 Gates

All in the foreground, logs under `tmp/vim-keymap/`. Heavy suites hold
the measurement lock (`mkdir
/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`, `rmdir` after,
wait and retry while it exists). The build always runs right before e2e.

1. `cd editor && npm run check`
2. `cd editor && ./node_modules/.bin/vitest run test/code/vim test/canvas/vim-keymap-input.test.ts test/canvas/vim-keymap-mount.test.ts`
3. `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`
   and `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml`
4. Locked: `cd editor && ./node_modules/.bin/vitest run` (full)
5. Locked: `cd editor && npm run test:style`
6. Locked: `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build && npm run e2e -- --browser all --profile behavior --run-id vim-keymap-001 --out ../tmp/vim-keymap/e2e/vim-keymap-001`
   (all `vim-*` and `vim-keymap-*` checks pass; add the
   `VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm` prefix
   and a note when the release wasm is absent)
7. Locked: `cd editor && npm run e2e:examples`

Verification records use exactly
`{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
Negative-control runs go only in `mutationEvidence`.

## 12. Rollout constraints

- K0 holds: with no stored document, every existing test passes
  unchanged.
- Files and budgets (every touched file stays under 1000 lines):
  `vim/notation.ts` (~150), `vim/keymap-schema.ts` (~350),
  `vim/mapper.ts` (~250), `vim/actions.ts` (~80), `vim/hints.ts` (~200),
  `vim/hop.ts` (~250), `vim/surround.ts` (~200), `vim/presets/*.json`;
  `controller.ts` grows by at most ~150 lines (it is 600); extended
  motions go in `motions.ts`/`execute.ts`, the change list in `marks.ts`,
  the `extended` grammar in `parser.ts`. If `controller.ts` would pass
  ~850 lines, the hop/hint session glue moves to `vim/sessions.ts`.
- Adapters: `code/vim-hints-view.tsx`, `code/vim-keymap-view.tsx`,
  `code/vim-keymap-store.ts`; small edits to `vim-mount.ts`, `mount.ts`,
  `input.ts` (`typeText`, `flushPendingInput` call sites),
  `vim-status-view.tsx` (button, `c` maps), `renderer.ts` (7.4),
  `code.css`, `platform/files.ts` (exported helpers), `app/main.ts`
  (store choice), `src-tauri/src/lib.rs`, new `src-tauri/src/vim_keymap.rs`.
- No new npm or cargo dependencies and no Tauri capability changes.
- Every plan declares `artifactRoots`: `target/`, `tree-sitter-vact.wasm`,
  `editor/node_modules`, `tmp/` and the Playwright outputs.
- Documentation: README `### Vim mode` gains the keymap document, the
  presets, hints and hop; `design-vim-mode.md` points here;
  `editor/test/e2e/README.md` lists the `vim-keymap-*` checks.

## 13. Implementation partition guidance

Plans use the prefix `impl-plans/active/vim-keymap-`. Suggested waves:

- Wave 0, in parallel: (a) `notation.ts` + `keymap-schema.ts` + types;
  (b) `src-tauri/src/vim_keymap.rs` + `lib.rs` + Rust tests.
- Wave 1, in parallel (after 0a): (a) `mapper.ts`; (b) `hints.ts`;
  (c) `hop.ts`; (d) extended built-ins and `surround.ts`; (e) presets.
- Wave 2: controller integration (mapper, actions, hint and hop sessions,
  snapshot, `setKeymap`, macro recording point) and the `input.ts` hooks.
- Wave 3, in parallel: (a) renderer hook and `mount.ts` wiring;
  (b) hint view, settings panel, CSS, style probes; (c) stores,
  import/export, `main.ts` wiring.
- Wave 4: integration tests, e2e, docs and the full gates.

## 14. Open questions

See `design-docs/user-qa/pending-vim-keymap-questions.md`:

- KQ1: hints and hop are off without a stored document
- KQ2: preset entries the design author could not read (m-maps, the full
  `<Nop>` set, `timeoutlen`)
- KQ3: deferred insert for insert-mode multi-key maps
- KQ4: hop smartcase
- KQ5: hop as an operator target and dot repeat
- KQ6: extended built-ins turn on with any stored document
- KQ7: insert `<C-v>` (reserved) in the preset

## References

- `design-docs/specs/design-vim-mode.md`: the baseline Vim mode
- `design-docs/specs/design-ui-style.md`: panel and button style rules
- `design-docs/specs/design-example-picker-and-persistence.md`: the
  `template.rs` command pattern and the Tauri store selection
- Vim `:help map.txt`, hop.nvim, mini.clue and vim-surround documentation:
  behavioral references only
