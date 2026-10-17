# Vim Keymap: Documentation Implementation Plan

**Status**: Ready
**Plan ID**: VK-41 (wave 4)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 2, 5-9, 12 (documentation)
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-30
- **Parallel (wave 4)**: VK-40
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

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

- [ ] README Vim section covers keymap, format, extra built-ins, hints, hop, IME, links
- [ ] e2e README lists the five checks and the gate command
- [ ] Progress Log entry added

## Progress Log

(none yet)
