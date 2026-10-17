# Pending: Vim mode questions (2026-10-16, session 374, branch wf/vim-mode)

These questions came up while designing the optional Vim mode for the code
pane (`design-docs/specs/design-vim-mode.md`). Each has a recommendation.
Implementation follows the recommendation until the question is answered.

The following are already decided and are not asked here: Vim mode is
off by default; the core is pure and DOM-free over `@codemirror/state`
with no EditorView; live-coding shortcuts work in every mode; Escape
closes the completion popup before it leaves insert mode; normal mode
never inserts IME text; renderer changes are limited to the cursor shape.

## VQ1. Visual block mode (`Ctrl-v`)

Visual block needs multi-range selections and column-rectangle operators,
and the renderer and input bridge handle only the main selection.

- Recommendation: defer it. `Ctrl-v` stays unbound in Vim mode, so the
  platform default applies (paste on Windows/Linux).
- Alternative: implement block selection as several selection ranges.
  This needs renderer and bridge work for multiple ranges.

## VQ2. `:w` / `:q` mapping

The document is synced live, so there is nothing to write or quit.

- Recommendation: `:w :q :wq :x :q!` do nothing and show the message
  `vactr syncs the document live; use :eval, :evalall or :hush`.
  Explicit commands are `:eval` (`Mod-Enter`), `:evalall` / `:run`
  (`Mod-Shift-Enter`), `:hush` (`Mod-.`) and `:format` (`Shift-Alt-f`).
  This avoids an unexpected audible change from habitual `:w`.
- Alternative: `:w` runs `evalAll`, as some live-coding setups map
  "save" to "evaluate".

## VQ3. Search-match highlighting (`hlsearch`)

Drawing all matches needs a new `search` annotation kind in
`renderer.ts`. A sibling branch (`wf/code-render`) is editing that file,
and the scope limits renderer changes to the cursor shape.

- Recommendation: defer it until `wf/code-render` is merged. `:noh` is
  accepted and clears the active-search indicator in the status row.
- Alternative: add the annotation kind now and accept the merge risk.

## VQ4. Excluded Vim features

Excluded from this scope: the numbered registers `1-9`, the small-delete
register `-`, the read-only registers `. % :`, insert-mode
`Ctrl-r`/`Ctrl-o`/`Ctrl-w`/`Ctrl-d`/`Ctrl-t`, `gv`, `gi`, the tag objects
`it`/`at`, `:g`, `:normal`, user mappings and vimrc, ex history,
incremental search, the `c` substitute flag, string- and comment-aware
bracket objects, and `Ctrl-a`/`Ctrl-x` increment.

- Recommendation: keep them excluded and add them later when users ask.
  The parser rejects them with a status message instead of guessing.

## VQ5. IME keys in normal mode

With the OS IME on, keys arrive as `keyCode 229` composition, and the
browser cannot switch the IME off.

- Recommendation: discard all IME input in non-insert modes (no command
  and no text). The bridge refocuses to end the platform composition, and
  the status row shows `IME input ignored in NORMAL mode`. The README
  documents this.
- Alternative: read `event.code` (the physical key) from 229 keydowns as
  Vim commands. This is layout-dependent and does not work on every
  engine.

## VQ6. Search dialect

- Recommendation: JavaScript `RegExp` source with the `u` flag. A
  trailing `\c` makes the search case-insensitive. A pattern that does not
  compile is searched literally, with a note. `:s` replacement accepts
  Vim's `&`, `\0`-`\9` and `\&`.
- Alternative: translate Vim "magic" syntax (`\(`, `\|`, `\<`, `\>`) to
  JS. This is closer to Vim but harder to get right.

## VQ7. Desired-column unit for `j`/`k`

- Recommendation: count graphemes from the line start, with a tab as one
  column. This is simple and deterministic.
- Alternative: display columns from the layout advances, with tab stops
  and wide CJK cells. This matches Vim's screen columns more closely, but
  it couples the core to the layout.

## VQ8. Clipboard registers in the Tauri webview

- Recommendation: use the browser async Clipboard API in both tiers,
  called inside the keydown gesture. Keep an internal mirror, and when the
  API is missing or rejects, use the mirror and show a message. Add no
  Tauri plugin.
- Alternative: add `tauri-plugin-clipboard-manager` for native clipboard
  access in the Tauri tier. This means a new dependency and capability.

## VQ9. IME commit in replace mode

The composition range is fixed when the composition starts, so the
length of the overwrite is not known in advance.

- Recommendation: an IME commit in replace mode inserts. Typed text
  without IME overwrites. The README documents this.
- Alternative: after the commit, delete as many following graphemes as
  were committed.
