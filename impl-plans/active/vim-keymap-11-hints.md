# Vim Keymap: Hint Tree Implementation Plan

**Status**: Ready
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

- [ ] Pinned API; file within budget; purity passes
- [ ] At least 10 passing cases covering groups, overrides, `<Nop>` removal, ordering, triggers
- [ ] Progress Log entry added

## Progress Log

(none yet)
