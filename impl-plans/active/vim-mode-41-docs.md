# Vim Mode: README Documentation Implementation Plan

**Status**: Completed
**Plan ID**: VIM-41 (wave 4)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 2, 3.3, 3.5, 3.7, 4.8, 4.13, 4.14, 6, 9; design-docs/user-qa/pending-vim-mode-questions.md
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Related Plans

- **Depends On**: VIM-30 (the behavior is integrated, so the text matches the shipped keys)
- **Parallel (wave 4)**: VIM-40

## Intent and Context

The design (section 9) requires README.md's `## Editor` section to
document Vim mode. The doc must cover:

- the toggle and the saved setting;
- the modes and the cursor shapes;
- the supported keys;
- the ex commands and the vactr action mapping;
- the live-coding shortcuts in every mode;
- the IME behavior (a primary Japanese use case);
- the clipboard registers;
- touch use;
- the excluded features.

Write it in the README's plain, compact style (see the existing
`## Editor` bullets). The common execution rules in
`vim-mode-00-contract.md` apply.

## Non-Goals

- Do not edit `README.jd.md`. The project rule is English-only output,
  and the Japanese README is maintained separately.
- No design-doc or plan-index edits; the serial finalization owns those.
- No source or test changes.

## File-Level Changes

### `README.md`

1. In `## Editor`, in the "It provides:" list, add one bullet after the
   completion-popup bullet:
   `- an optional Vim mode (off by default; the vim button in the transport bar), described below;`
2. After the paragraph that ends "...is handled by the editor." (the end of
   the editor feature list, before the wasm build snippet), add a
   `### Vim mode` subsection with these short parts:
   - **Turning it on.** The `vim` toggle in the transport bar. The setting
     is saved per browser in `localStorage` (`vactr.vimMode`), and the
     Tauri app keeps its own.
   - **Modes.** Normal, insert, replace (`R`), visual (`v`), visual line
     (`V`) and operator-pending. The status row shows the mode, the
     pending keys, `recording @q` and the active search. Cursors are a
     block in normal and visual, a bar in insert, and an underline in
     replace and operator-pending.
   - **Keys.** One compact list or table of the motions, operators, text
     objects, registers (`"a-z`, `A-Z` append, `0`, `_`, `+`/`*` system
     clipboard), counts, `.`, marks and macros. Take the sets from design
     section 2.1; do not invent extra keys.
   - **Ex commands.** `:{n}`, `:$`, `:s`/`:%s`/`:'<,'>s` with `g`/`i`,
     and `:noh`. `:w :q :wq :x :q!` only show a message. `:eval`,
     `:evalall`/`:run`, `:hush` and `:format` run the matching vactr
     actions.
   - **Live-coding shortcuts.**
     - `Mod-Enter`, `Mod-Shift-Enter`, `Mod-.` and `Shift-Alt-f` work in
       every mode. Mod is Cmd on macOS and iPadOS, and Ctrl elsewhere.
     - `Ctrl-Space` and the completion popup keys work in every mode.
     - The first Escape closes the popup, and the next one leaves insert
       mode. `Ctrl-[` is Escape.
     - The browser's own `Ctrl-r`/`Ctrl-f`/`Ctrl-d`/`Ctrl-u`/`Ctrl-b`
       shortcuts are replaced by the Vim keys while the code pane has
       focus and Vim is not in insert mode.
   - **IME (Japanese input).**
     - Composition in insert mode works exactly as without Vim.
     - In normal, visual and operator-pending modes, IME input never
       inserts text; it is ignored, and the status row says so.
     - The browser cannot turn the OS IME off, so switch the IME to direct
       input for normal-mode commands.
     - Escape during a conversion cancels the conversion first.
     - In replace mode, an IME commit inserts rather than overwrites.
   - **Clipboard.** `"+`/`"*` use the browser clipboard API. When it is
     unavailable or denied, an internal copy is used, with a message.
   - **Touch.** Vim mode is for hardware keyboards. Tap the mode label to
     enter or leave insert mode with the on-screen keyboard, or turn Vim
     off with the toggle.
   - **Not included.** Visual block (`Ctrl-v`), search highlighting,
     numbered registers and the other items in
     `design-docs/user-qa/pending-vim-mode-questions.md` (link it).

Keep the subsection under about 70 lines.

## Acceptance Criteria

- [x] README.md has the new bullet and the `### Vim mode` subsection with
      all ten parts.
- [x] Every key and command named exists in design section 2.1 or 4.13.
- [x] No other product file is changed; `README.jd.md` is untouched.

## Verification

1. `grep -n '### Vim mode' README.md` must print exactly one line.
2. `grep -c 'vactr.vimMode\|IME\|:evalall\|Ctrl-\[' README.md` must
   print a count of 4 or more.
3. `git diff --stat -- README.md README.jd.md` must show only
   README.md changed.

## Progress Log

### Session: 2026-10-17

**Tasks Completed**: Added the optional Vim bullet and the compact Vim mode
subsection to `README.md`, covering toggle/persistence, modes/cursors, keys,
ex commands, live-coding shortcuts, Japanese IME, clipboard, touch and
excluded features.

**Verification**: `grep -n '### Vim mode' README.md` returned one line
(`tmp/vim-mode/VIM-41/final2-section.log`); the required content grep
returned 8 (`final2-content.log`); the scoped diff stat showed only
`README.md` (`final2-diff-stat.log`). The section spans 46 lines
(`final2-section-length.log`), and `git diff --check -- README.md
impl-plans/active/vim-mode-41-docs.md` passed (`final2-diff-check.log`).
The scoped status lists only `README.md` and this plan
(`final2-scoped-status.log`).

**Notes**: The motions, operators, text-object pairs, shortcut commands,
registers, marks, macros and Ex commands were checked against design
sections 2.1 and 4.13. Other shared-tree files visible in
`readme-status-final-02.log` belong to other fanout plans; this plan edited only
`README.md` and its own progress record. `README.jd.md` is untouched. Formal
review and serial finalization remain downstream workflow steps.
