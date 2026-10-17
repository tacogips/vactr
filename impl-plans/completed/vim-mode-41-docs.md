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

### Session: 2026-10-17 VIM-41 current-source confirmation

**Tasks Completed**: No product edits were needed in this execution: the VIM-41
README bullet and subsection are already present in the shared branch. Rechecked
the current README and confirmed the ten documentation parts and their keys and
commands against design sections 2.1, 3.3, 3.5, 3.7, 4.8, 4.13 and 4.14.

**Verification**: `grep -n '### Vim mode' README.md` printed exactly one line
at 235 (`tmp/vim-mode/VIM-41/step6-final-20261017-01/grep-section.log`);
the required content grep returned 8 (`grep-content.log`). The scoped diff
stat and whitespace check both exited 0 (`diff-stat.log`, `diff-check.log`).
The README is already at HEAD, so the current diff stat is empty; the prior
implementation evidence above records the original README-only change.
`README.jd.md` remains untouched.

**Notes**: This run updated only this plan's Progress Log. Existing shared-tree
changes owned by other plans were left intact. Formal review and serial
finalization remain downstream workflow steps.

### Session: 2026-10-17 VIM-41 count-bearing documentation verification

**Tasks Completed**: Added a plan-local read-only Node assertion script for the
README documentation contract. It checks the single heading, the Editor
bullet, all nine ordered subsection labels, each extracted key/command token
against design sections 2.1 and 4.13, required content, subsection length and
the unchanged `README.jd.md` state.

**Verification**: `node tmp/vim-mode/VIM-41/step6-countcheck-20261017-02/doc-check.mjs`
passed with `testsRun=158`, `testsPassed=158`, `failureCount=0`, including 140
design-token assertions (`doc-check.log`, exit 0). `grep -n '### Vim mode'
README.md` returned the single heading at line 235 (`grep-section.log`, exit 0).
`git diff --stat -- README.md README.jd.md` was empty (`diff-stat.log`, exit 0),
and `git diff --check -- impl-plans/active/vim-mode-41-docs.md` passed before
this entry (`plan-diff-check-preappend.log`, exit 0). The README files remain
unchanged from `HEAD`; this session changes only this plan's Progress Log and
adds evidence under `tmp/vim-mode/VIM-41/step6-countcheck-20261017-02/`.


### Session: 2026-10-17 VIM-41 node:test evidence-runner repair

**Tasks Completed**: Added a Node `node:test` suite under
`tmp/vim-mode/VIM-41/step6-countcheck-20261017-03/` that retains the existing
README checks as individually named tests. It verifies the heading, Editor
bullet, all nine ordered subsection parts, each extracted design token against
sections 2.1 and 4.13, a minimum token-count guard, the required README tokens,
section length, and `README.jd.md` readability and unchanged status.

**Verification**: `node --test
tmp/vim-mode/VIM-41/step6-countcheck-20261017-03/doc-check.test.mjs` passed
with 159 tests, 159 passed, and 0 failed (exit 0;
`doc-check-test.log`), including 140 design-token tests.
`node --check tmp/vim-mode/VIM-41/step6-countcheck-20261017-03/doc-check.test.mjs`
passed (exit 0; `node-check.log`). `grep -n '### Vim mode' README.md`
returned exactly one heading at line 235 (exit 0; `grep-section.log`).
`git diff --stat -- README.md README.jd.md` was empty (exit 0;
`diff-stat.log`), and `git diff --quiet HEAD -- README.md README.jd.md`
confirmed both README files are byte-identical to `HEAD` (exit 0;
`readme-identity.log`).

**Notes**: The Progress Log is the only tracked file changed for VIM-41 in this
repair; test code and logs are plan-local evidence. The initial failed runner
transcription attempts are preserved as `doc-check-test-attempt-01.log` and
`doc-check-test-attempt-02.log` and are not gating verification. Independent
Step 6 test-integrity and Step 7 adversarial decisions remain downstream.
