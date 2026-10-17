# Vim Keymap: Documentation Implementation Plan

**Status**: Completed
**Plan ID**: VK-41 (wave 4)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 2, 5-9, 12 (documentation)
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-30
- **Parallel (wave 4)**: VK-40
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

Users must be able to find and use the keymap, presets, hints and hop.
The README `### Vim mode` section (`README.md`, around the `## Editor`
section) describes the merged Vim mode; extend it in the same concise
bullet style. The e2e README lists the Vim checks; add the new ones.

## Non-Goals

- No source or test changes. No change to design documents (the design
  status flip to Implemented is serial finalization after all gates pass).
- Do not edit `impl-plans/README.md` (serial reconciliation).

## File-Level Changes

### README.md (shared)

In `### Vim mode` add bullets:
- **Keymap.** The `keymap` button in the Vim status row opens a JSON
  editor; Default and mise-darwin presets; Apply validates per entry and
  applies live; Import/Export a `.json` file; Reset returns to the
  built-in behavior. Stored in `localStorage` `vactr.vimKeymap` on the
  web and in `vim-keymap.json` in the app config directory in the Tauri
  app. Without a stored keymap Vim mode behaves exactly as before.
- **Document format.** Version 1 fields: `leader`, `timeoutlen`, `maps`
  (`n i x v o c`, `lhs`, `rhs` or `action`, `remap`, `desc`, `<Nop>`),
  `hints`, `hop`, `unsupported`; the action ids list; the reserved
  `Ctrl` chords; a short example (`kj` to Escape).
- **Extra built-ins with a keymap.** `gj gk` (plain line motions, no soft
  wrap), `gJ`, `g; g,`, `zz`, `<C-e>`/`<C-y>` (`<C-y>` only from a
  mapping because `Ctrl-y` is redo), `ys yss cs ds`.
- **Hints.** Bottom-left panel after a trigger prefix (`hints.triggers`,
  `delayMs`); it does not take focus; the mapping timeout pauses while it
  is shown.
- **Hop.** `hop.char1`, `hop.char1Line`, `hop.word`, `hop.line`; labels from
  `hop.keys`; smartcase; visual extends; operator targets with an `o`
  mapping (not dot-repeatable); `hop.enabled: false` makes `gj`/`gf` fall
  back to the built-ins.
- **IME.** Insert-mode mappings never see IME composition; a held key is
  inserted before a composition starts.
- Link `design-docs/specs/design-vim-keymap-hints-hop.md` and
  `design-docs/user-qa/pending-vim-keymap-questions.md`.

### editor/test/e2e/README.md (shared)

Add a short section for `vim-keymap.mjs` listing the checks
`vim-keymap-preset`, `vim-keymap-kj`, `vim-keymap-hints`,
`vim-keymap-hop`, `vim-keymap-hop-off`, that they run inside the behavior
profile after the Vim checks, and the gate command from VK-40 gate 6
(with the rebuild-before-e2e rule).

## Verification (from the worktree root)

1. `rg -n 'vactr.vimKeymap|hop.char1|vim-keymap-hints|design-vim-keymap-hints-hop' README.md editor/test/e2e/README.md` -> each term found (record the output, exit 0)
2. `rg -n '[^\x00-\x7F]' README.md editor/test/e2e/README.md` on the added lines -> no emojis introduced (record; pre-existing non-ASCII elsewhere is fine)

## Completion Criteria

- [x] README Vim section covers keymap, format, extra built-ins, hints, hop, IME, links
- [x] e2e README lists the five checks and the gate command
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 (session-401 continuation)

**Tasks Completed**: README and e2e README documentation; VK-41 completion record.

**Verification**:
- `rg -n 'vactr.vimKeymap|hop.char1|vim-keymap-hints|design-vim-keymap-hints-hop' README.md editor/test/e2e/README.md` -> exit 0; each required term found. Log: `tmp/vim-keymap/VK-41/s401-attempt-2/terms.log`.
- `rg -n '[^\x00-\x7F]' README.md editor/test/e2e/README.md` -> exit 1 with no matches, confirming no non-ASCII characters (including emojis) in either README. Log: `tmp/vim-keymap/VK-41/s401-attempt-2/nonascii.log`.

**Notes**: The first logging wrapper did not run the checks because zsh rejected assignment to its read-only `status` variable; both commands were rerun with a safe exit-code variable and the complete logs above. Source and test files were not changed.

### Session: 2026-10-17 (Step 6 verification rerun)

**Tasks Completed**: Reverified the VK-41 documentation contract and added positive-count behavior evidence to resolve the carried implementation-materially-unverified gate.

**Verification**:
- `cd editor && ./node_modules/.bin/vitest run test/code/vim test/code/vim-keymap-store.test.ts` -> exit 0; 34 files and 1,171 tests passed. Log: `tmp/vim-keymap/VK-41/s402-attempt-1/focused-vitest.log`.
- `rg -n 'vactr.vimKeymap|hop.char1|vim-keymap-hints|design-vim-keymap-hints-hop' README.md editor/test/e2e/README.md` -> exit 0; all required terms found. Log: `tmp/vim-keymap/VK-41/s402-attempt-1/terms.log`.
- `rg -n '[^\x00-\x7F]' README.md editor/test/e2e/README.md` -> exit 1 with no matches. Log: `tmp/vim-keymap/VK-41/s402-attempt-1/nonascii.log`.
- `git diff --check -- README.md editor/test/e2e/README.md impl-plans/active/vim-keymap-41-docs.md` -> exit 0. Log: `tmp/vim-keymap/VK-41/s402-attempt-1/diff-check.log`.

**Notes**: VK-41 is documentation-only and no implementation source was modified. The focused Vim suite exercises the documented keymap behavior on the current tree; build/e2e outputs are owned by VK-40 and are not required by this plan.

### Session: 2026-10-17 (Step 8 documentation refresh and archive)

**Tasks Completed**: Corrected the README hop-off sentence (`gj` falls back to the built-in line motion; `gf` does nothing because vactr has no built-in `gf`). Archived all 16 VK plans to `impl-plans/completed/` after the session-401 integration review accepted the combined tree.

**Verification**: The README term, non-ASCII and `git diff --check` checks were rerun after the edit. Logs: `tmp/vim-keymap/step8/`.
