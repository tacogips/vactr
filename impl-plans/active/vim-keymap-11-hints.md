# Vim Keymap: Hint Tree Implementation Plan

**Status**: Completed (implementation; independent review and serial finalization pending)
**Plan ID**: VK-11 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 6.1, 6.2
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`HintView`, `HintEntry`), VK-01 (`CompiledKeymap`, `formatKey`, `formatNotation`)
- **Parallel (wave 1)**: VK-10, VK-12, VK-13, VK-14, VK-15, VK-16
- **Consumer**: VK-20 (hint session in the controller), VK-16 (view renders `HintView`)
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

A which-key style panel lists the keys that can follow the current
pending prefix. This plan builds that list as pure data from the compiled
maps plus a static built-in table. Timing and showing are the
controller's job (VK-20); rendering is VK-16.

## Non-Goals

- No timers, no DOM, no controller changes.
- No hop filtering logic here: VK-01 already drops `hop.*` mappings when
  hop is disabled, so they are absent from `CompiledKeymap.maps`.

## File-Level Changes

### editor/src/code/vim/hints.ts (new, budget 200 lines)

```ts
export function buildHints(keymap: CompiledKeymap, mode: 'n' | 'x' | 'o', prefix: readonly VimKey[], extended: boolean): HintView;
export function isTrigger(keymap: CompiledKeymap, mode: 'n' | 'x' | 'o', pending: readonly VimKey[]): boolean;
```

- Mapping entries: for every mapping in `keymap.maps[mode]` whose lhs
  starts with `prefix` and is longer: next key `k = lhs[prefix.length]`.
  If `lhs.length === prefix.length + 1` it is a leaf with `desc` = the
  mapping desc, else `formatNotation` of the rhs keys or the action id
  (`<Nop>` targets are not listed). If longer, a group entry
  `{ key, desc: '+N keys', group: true }` where N counts mappings under
  `prefix + k`. A leaf and a group for the same key: show the leaf desc
  and set `group: true`.
- Built-in table (design 6.1): prefix `g`: `g first line`, `e end of previous word`,
  `~ toggle case`, `u lowercase`, `U uppercase`, and when `extended`:
  `j line down`, `k line up`, `J join without spaces`, `; older change`,
  `, newer change`; prefix `z` (extended only): `z center line`; prefix
  `m` (mode n only): key `a-z`, desc `set mark`. In mode `x` the `g`
  table is the same; mode `o` gets only `g` entries `g e j k`.
- A mapping whose lhs equals `prefix + k` overrides the built-in `k`
  entry; a `<Nop>` mapping for `prefix + k` removes the built-in entry.
  The `a-z` range entry stays (single mapped letters such as `ms` show as
  their own entries next to it).
- Order: keys sorted by class `a-z`, `A-Z`, `0-9`, punctuation, then
  special keys (`<...>` and `<Space>`); within a class by code point;
  the `a-z` range entry sorts at `a`.
- `prefix` string is `formatNotation(prefix)`; `summary` is
  `${prefix}: ${entries.length} next keys`.
- `isTrigger`: true when `pending` equals (key-wise) one of
  `keymap.hints.triggers[mode]` and `keymap.hints.enabled`.

## Invariants

- Pure and deterministic.

## Test Cases (`editor/test/code/vim/hints.test.ts`)

Build keymaps with `compileKeymap` from VK-01.
- n maps `<leader>y "+y` (desc `yank to clipboard`), `<leader>p`, leader `<Space>`: prefix `[' ']` -> entries `p`, `y` with descs; summary `<Space>: 2 next keys`
- multi-level: `<leader>ga` and `<leader>gb` -> prefix `[' ']` has group `g` `+2 keys`; prefix `[' ','g']` has `a`, `b`
- prefix `g`, extended false, no maps -> `e g u U ~` built-ins (sorted `e g u U ~`), no `j k`
- extended true adds `j k J ; ,`; a map `gj -> hop.char1` (hop enabled) overrides `j` desc; `g~ -> <Nop>` removes `~`
- prefix `m` in n with `mm`,`ms` maps -> `a-z set mark`, `m`, `s` entries
- `z` with extended false and no maps -> empty entries
- `isTrigger` with triggers n `[<Space>, g]`: `[' ']` true, `['g']` true, `['z']` false; hints disabled -> false

## Verification (from editor/)

1. Scoped typecheck including `src/code/vim/hints.ts` and the test -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim/hints.test.ts test/code/vim/purity.test.ts` -> exit 0, failureCount 0, at least 10 cases

## Completion Criteria

- [x] Pinned API; file within budget; purity passes
- [x] At least 10 passing cases covering groups, overrides, `<Nop>` removal, ordering, triggers
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-11 implementation

**Tasks Completed**: Added pure hint generation and trigger matching from compiled maps plus the design's built-in table. Mapping descriptions, action-id fallbacks, descendant groups, exact-map overrides, `<Nop>` removal, mode-specific built-ins, deterministic key-class ordering, formatted prefix/summary, and trigger equality are covered. Hop-off mappings are already dropped by VK-01 compilation and therefore do not enter the hint list.

**Source Hashes and Budgets**:
- `editor/src/code/vim/hints.ts`: `1c2fb0ae0afa1d74475e2ee0dcb9ab3fc0f98b29` (112 lines; budget 200)
- `editor/test/code/vim/hints.test.ts`: `ffb829824581a96db6011091dde1dfb54fe9051f` (110 lines)
- `impl-plans/active/vim-keymap-11-hints.md`: refreshed after this entry

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-11/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-11/attempt-3/typecheck.log", notes: "Scoped to VK-11 implementation and test files plus their imported modules."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/hints.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 16, testsPassed: 16, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-11/attempt-3/vitest.log", notes: "15 hint cases and the Vim purity test passed, including n/x built-in parity."}`

**Prior Verification**: The first scoped typecheck found two test-call argument errors, corrected before the passing rerun. The first focused Vitest run passed 14/15 because the hop-off test expected the wrong class ordering; the test expectation was corrected to match the specified lowercase, uppercase, digit, punctuation and special-key ordering. A further x-mode parity assertion was added, and the final run passed 16/16. Earlier logs remain in `tmp/vim-keymap/VK-11/attempt-1/` and `attempt-2/`.

**Evidence**: Per-edit intentions are recorded in `tmp/vim-keymap/VK-11/attempt-1/source-edit-intent.md`. The scoped config is `tmp/vim-keymap/VK-11/tsconfig.json`. No files outside this plan's write paths were edited. Independent review and serial integration verification remain downstream.
