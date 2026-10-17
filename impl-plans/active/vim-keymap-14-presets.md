# Vim Keymap: Default and mise-darwin Presets Implementation Plan

**Status**: Completed (implementation; independent review pending)
**Plan ID**: VK-14 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 8.1 (Default document), 9.1-9.3; user-qa KQ2, KQ7
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`resolveJsonModule`), VK-01 (`compileKeymap` for the zero-issue test)
- **Parallel (wave 1)**: VK-10, VK-11, VK-12, VK-13, VK-15, VK-16
- **Consumers**: VK-20 (preset regression tests), VK-30 (panel preset buttons), VK-40 (e2e)
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

The user wants their Neovim setup reproduced as a selectable preset.
The reference files are the user's dotfiles:
`/Users/taco/gits/tacogips/mise-darwin/dotfiles/.config/nvim/lua/taco/core/keymaps.lua`,
`.../core/options.lua` (leader, possibly `timeoutlen`) and
`.../plugins/init.lua` (hop.nvim, mini.clue, vim-surround). The design
author could not read them (KQ2); this plan must try.

## Non-Goals

- No new built-ins or actions: anything the editor cannot do goes into
  `unsupported` with a reason.
- Do not edit `keymap-schema.ts` to make an entry pass; fix the entry.

## Tasks

1. Try to read the three reference files (read-only; never modify
   them). Record in the Progress Log whether each was readable.
2. If readable: classify every mapping as (a) reproduced, (b) nearest
   vactr action per design 9.3 (`vactr.format` for LSP format,
   `vactr.evaluateAll` for QuickRun, `vactr.completion` for completion
   trigger), or (c) an `unsupported` entry `{mode, lhs, reference, reason}`.
   Read `timeoutlen` from options.lua when set. Use the exact rhs for
   `<leader>Y`/`<leader>P` and the `ma`/`mi` targets, and the full `<Nop>`
   list.
3. If not readable: ship exactly the design 9.2 table (`timeoutlen` 1000,
   `ma` -> `va`, `mi` -> `vi`, `<leader>Y` -> `"+Y`, `<leader>P` -> `"+P`,
   the `<Nop>` list from the design) and record the gap.
4. Write the two JSON documents and the index module, plus tests.

## File-Level Changes

### editor/src/code/vim/presets/default.json (new)

Exactly the design 8.1 Default document (pretty-printed, 2-space
indent).

### editor/src/code/vim/presets/mise-darwin.json (new)

A version-1 document (design section 2) with: `leader` `<Space>`;
`timeoutlen` per task 2/3; `hints` `{enabled: true, delayMs: 200, triggers: {n: ["<Space>","g","m","z"], x: ["<Space>","g"]}}`;
`hop` `{enabled: true, keys: "fdjhklsagqwertyuiopzxcvbnm", uppercaseLabels: false}`;
`maps` per design 9.2 (at minimum): i and x `kj -> <Esc>`; n `k -> gk`,
`J -> gJ`, `+ -> g;`, `mm -> %`, `zc -> zz`, `zj -> <C-e>`, `zk -> <C-y>`,
`<Esc><Esc> -> :nohlsearch<CR>`, the `<Nop>` set (`Q gQ gq g% gx g0 g8 ga g^ g_ g~ za zA zo` and the rest),
n and x `<leader>y/Y/p/P` with the `+` register, i `<C-d> <Del>`,
`<C-h> <BS>`, `<C-f> <Right>`, `<C-b> <Left>`, c `<C-g> <Esc>`,
`<C-a> <Home>`, n `ms ys`, `mr cs`, `md ds`, `ma`/`mi` as classified,
n `gn` and `<C-g>` -> `vactr.diagnostic.next`, `gp` and `<C-s>` ->
`vactr.diagnostic.prev`, n and x `gj -> hop.char1`, `gf -> hop.char1Line`.
Every entry has a `desc`. `unsupported` lists at least: insert `<C-v>`
(reason: reserved platform paste, use Mod-v; KQ7), LSP hover, LSP
restart, file write/quit, Chilla, telescope pickers, window/buffer/tab
commands that appear in the reference.

Pitfall: JSON strings need `\\` for a backslash and `\"` inside rhs such
as `"\"+y"`.

### editor/src/code/vim/presets/index.ts (new, budget 30 lines)

```ts
export type KeymapPresetId = 'default' | 'mise-darwin';
export interface KeymapPreset { id: KeymapPresetId; label: string; text: string }
export const KEYMAP_PRESETS: readonly KeymapPreset[];   // [Default, mise-darwin]
```
`text` is `JSON.stringify(imported, null, 2)` of the imported JSON.

## Invariants

- Both presets compile with zero issues (no errors, no warnings).
- No source outside `presets/` changes.

## Test Cases (`editor/test/code/vim/presets.test.ts`)

- each preset: `compileKeymap(text).issues` is `[]` and `keymap` non-null
- Default: maps empty, hints and hop disabled, leader `\`
- mise-darwin: leader `' '`; `maps.i` and `maps.x` contain `kj` -> keys `<Esc>`; `maps.n` contains `k`, `J`, `+`, `mm`, `zc`, `zj`, `zk`, `<Esc><Esc>` (ex `nohlsearch`), `gn`/`gp`/`<C-g>`/`<C-s>` actions, `gj`/`gf` hop actions, `ms`/`mr`/`md`; `cmdline` maps `<C-g>` -> `<Esc>`, `<C-a>` -> `<Home>`; triggers n `[[' '],['g'],['m'],['z']]`; every mapping has a desc
- the JSON contains an `unsupported` array with at least 4 entries, one for `<C-v>`

## Verification (from editor/)

1. Scoped typecheck including `src/code/vim/presets/index.ts` and the test -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim/presets.test.ts` -> exit 0, failureCount 0, at least 6 cases
3. Record in the Progress Log which reference files were readable (task 1) and, if any were not, that the design table was used (KQ2 gap).

## Completion Criteria

- [x] Both JSON presets and index.ts exist; zero compile issues
- [x] Classification recorded; all three reference files were readable
- [x] Preset tests pass (7/7)
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-14 implementation

**Tasks Completed**: Added both bundled preset documents, the typed preset index, and seven preset tests. The source dotfiles were readable at `/Users/taco/gits/tacogips/mise-darwin/dotfiles/.config/nvim/lua/taco/core/keymaps.lua`, `core/options.lua`, and `plugins/init.lua`. `options.lua` contains no `timeoutlen`, so the preset uses 1000 ms. No KQ2 readability gap applies.

**Classification**: Reproduced keymaps include `kj`, `k`, `J`, `+`, `mm`, `zc/zj/zk`, `<Esc><Esc>`, the complete explicit `<Nop>` sets from the reference, `+` register maps, insert navigation/deletion, command-line controls, surround (`ms/mr/md`), text-object targets (`ma/mi`), diagnostic navigation and hop (`gj/gf`). The QuickRun mapping `<leader>R` uses the nearest `vactr.evaluateAll` action. The reference's unsupported hover, restart, file operations, path/Chilla actions, LSP navigation/code actions/rename, Ctrl-v paste, and insert Ctrl-a completion mapping are recorded in `unsupported` with reasons. The reference has no explicit format mapping; format-on-save is configured separately. Window/buffer/tab categories and additional Telescope picker categories are recorded as absent in the inspected keymaps file, not as invented active mappings. The plan contract requests `<leader>Y` in both n and x; x therefore includes that extra plan-required entry while the reference itself maps `Y` only in n.

**Source Hashes**:
- `editor/src/code/vim/presets/default.json`: `7619700d1480c07345bbe754ef6e6f5eb813d243`
- `editor/src/code/vim/presets/mise-darwin.json`: `a3f897a94853041b2a1abccbc468d646902f7250`
- `editor/src/code/vim/presets/index.ts`: `68b59dde96845e79da0c61e60d50db6aa3098436` (10 lines; under 30)
- `editor/test/code/vim/presets.test.ts`: `63e98a54416e8dd3c59c980a0bd2b1961c44eac1` (72 lines)

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-14/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-14/attempt-2/typecheck.log", notes: "Scoped to presets/index.ts, the imported schema/action/notation modules and presets.test.ts; final source after the leader-qualified QuickRun correction."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/presets.test.ts", exitStatus: 0, testsRun: 7, testsPassed: 7, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-14/attempt-2/vitest.log", notes: "One file, seven cases; final source after the leader-qualified QuickRun correction."}`

**Drift**: Shared worktree includes concurrent changes in other plan paths; VK-14 did not edit those paths. The first typecheck setup attempt used a wrong relative config path and zsh's reserved `status` variable, so TypeScript did not run in that attempt; the corrected command above passed. The initial successful source verification preceded the QuickRun lhs correction and is superseded by attempt-2.

**Blockers**: None for VK-14. Formal review and consumer wiring remain downstream.
