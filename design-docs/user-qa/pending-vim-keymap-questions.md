# Pending: Vim keymap, hints and hop questions (2026-10-17, session 395, branch wf/vim-keymap)

These questions came up while designing the JSON keymap, the next-key
hints and hop jumps for the Vim mode
(`design-docs/specs/design-vim-keymap-hints-hop.md`). Each has a
recommendation. Implementation follows the recommendation until the
question is answered.

Already decided and not asked here: Vim mode itself stays off by default;
P keys (live-coding, undo and clipboard chords) are never remapped; no
code is copied from hop.nvim, mini.clue, which-key or vim-surround; the
renderer change is limited to the hop label overlay.

## KQ1. Hints and hop with no stored keymap

- Recommendation: off. With no stored document the controller runs
  exactly the merged Vim mode (design section 1, K0). Hints and hop turn
  on only when a stored document enables them, for example the
  `mise-darwin` preset.
- Alternative: turn hints on by default with built-in triggers `g` and
  `z`. This changes the default behavior.

## KQ2. Preset entries the design author could not read

The reference files under `/Users/taco/gits/tacogips/mise-darwin/` were
not readable from the design sandbox. The design table (section 9.2)
comes from the issue text. Unknown details: the exact targets of the
`ma`/`mi` maps (expected `va`/`vi`), the full `<Nop>` list beyond the one
in the issue, the exact `<leader>Y`/`<leader>P` right-hand sides, and
whether `options.lua` sets `timeoutlen`.

- Recommendation: the preset implementation task reads the three files
  and classifies every mapping (reproduced, nearest action, or
  `unsupported` with a reason). If the files are not readable then, the
  preset ships with the design table and `timeoutlen` 1000, and the gap is
  recorded in the plan.
- Alternative: the user confirms these entries before the preset task.

## KQ3. Insert-mode multi-key maps (`kj`)

- Recommendation: deferred insert, as in Vim. The first key (`k`) is
  held and shown in the status row; it is inserted when the buffer stops
  matching or after `timeoutlen`. Keys typed through an active IME never
  reach the mapping layer.
- Alternative: insert `k` at once and delete it when `j` completes the
  map. This shows the key at once but adds a delete to the undo and dot
  records.

## KQ4. Hop character matching

- Recommendation: smartcase. A typed lowercase ASCII letter matches both
  cases; any other character matches exactly.
- Alternative: always exact, or always case-insensitive.

## KQ5. Hop as an operator target

- Recommendation: supported when the user maps a `hop.*` action in `o`
  mode (the preset does not, matching the reference modes `n` and `x`).
  A forward target is inclusive and a backward target exclusive. The
  command is not dot-repeatable.
- Alternative: no operator support (document as unsupported).

## KQ6. When the extended built-ins turn on

`gj gk gJ g; g, zz <C-e> <C-y>` and `ys cs ds` are not in the merged Vim
mode.

- Recommendation: they turn on whenever a keymap document is stored,
  including the `Default` preset. `Reset` returns to exactly the merged
  behavior.
- Alternative: always on (changes the no-config behavior), or a separate
  setting (one more option to explain).

## KQ7. Insert `<C-v>` paste from `+`

`Ctrl-v` is in the platform pass-through set (P3), so it cannot be
mapped.

- Recommendation: list it as `unsupported` in the preset; `Mod-v` pastes
  the system clipboard already.
- Alternative: allow mapping `Ctrl-v` in insert mode on Apple platforms
  only, where `Ctrl-v` is not the platform paste.
