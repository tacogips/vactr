# Vim Keymap: Notation, Schema Compiler and Action Table Implementation Plan

**Status**: Ready
**Plan ID**: VK-01 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 2, 3, 4.2 (step 6), 4.3, 4.4.1, 4.5, 7.5, 8.1
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-00, VK-02, VK-03, VK-04
- **Next**: VK-10 mapper, VK-11 hints, VK-14 presets, VK-16 views consume `CompiledKeymap`, `KeymapIssue`, `formatNotation`
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

Users write a version-1 JSON keymap document (design section 2). This
plan delivers the pure compiler that turns that text into a
`CompiledKeymap` plus path-qualified issues, the Vim-notation parser, and
the action id table. It never throws, and the editor must start with any
input. It runs in parallel with VK-00, so it must not import the new
VK-00 types; it defines its own `MapMode` locally as
`'n' | 'i' | 'x' | 'o' | 'c'` (identical to the VK-00 alias; VK-20 may
unify later) and uses the existing `VimKey` from `types.ts`.

## Non-Goals

- No mapper resolution, timers, hints or hop logic.
- No preset files (VK-14). No storage (VK-03).
- Do not edit `types.ts`, `keys.ts`, `parser.ts` or any existing test.

## File-Level Changes

### editor/src/code/vim/notation.ts (new, budget 180 lines)

```ts
export function parseNotation(text: string, leader: VimKey | null): { keys: VimKey[] } | { error: string };
export function formatKey(key: VimKey): string;
export function formatNotation(keys: readonly VimKey[]): string;
```

Rules (design section 3 table): printable characters map to themselves
(grapheme by grapheme, use `Intl.Segmenter` like `keys.ts:normalizeKey`);
`<Space>` -> `' '`; `<lt>` `<Bar>` `<Bslash>` -> `<` `|` `\`;
`<CR>`/`<Enter>`/`<Return>` -> `<CR>`; `<Esc>` and `<C-[>` -> `<Esc>`;
`<BS> <Del> <Tab> <Left> <Right> <Up> <Down> <Home> <End>` -> the same
names as `keys.ts` `namedKeys`; `<C-{letter}>` -> `<C-{lowercase}>`;
`<leader>` -> the `leader` argument (error `<leader> needs a leader` when
null). Names inside `<...>` are case-insensitive. Any other `<...>`
(`<M-x>`, `<A-x>`, `<D-x>`, `<S-x>`, `<F1>`, `<C-1>`, `<Nop>` inside a
sequence) is an error naming the token. A `<` that does not start a valid
`<name>` token (no closing `>`, or contains a space) is the literal `<`.
`formatKey(' ')` is `<Space>`, `formatKey('<')` is `<lt>`; other keys
print as stored. `formatNotation` joins `formatKey`.

### editor/src/code/vim/actions.ts (new, budget 60 lines)

```ts
export type ActionId = 'vactr.evaluate' | 'vactr.evaluateAll' | 'vactr.hush' | 'vactr.format'
  | 'vactr.diagnostic.next' | 'vactr.diagnostic.prev' | 'vactr.completion'
  | 'hop.char1' | 'hop.char1Line' | 'hop.word' | 'hop.line';
export const ACTION_MODES: Readonly<Record<ActionId, readonly ('n' | 'i' | 'x' | 'o')[]>>;
export function isActionId(value: string): value is ActionId;
export const isHopAction = (id: ActionId): boolean => id.startsWith('hop.');
```

Modes exactly as the design 4.3 table (`vactr.evaluate/evaluateAll/hush/format/completion`: n x i;
diagnostics: n x; `hop.*`: n x o).

### editor/src/code/vim/keymap-schema.ts (new, budget 450 lines)

```ts
export const KEYMAP_MAX_BYTES = 262_144;
export const KEYMAP_MAX_ENTRIES = 2_000;
export const DEFAULT_HOP_KEYS = 'fdjhklsagqwertyuiopzxcvbnm';
export type MapMode = 'n' | 'i' | 'x' | 'o' | 'c';
export type RhsSegment = { kind: 'keys'; keys: readonly VimKey[] } | { kind: 'ex'; command: string } | { kind: 'prompt' };
export type MapTarget = { kind: 'keys'; segments: readonly RhsSegment[]; remap: boolean } | { kind: 'nop' } | { kind: 'action'; id: ActionId };
export interface CompiledMapping { mode: 'n' | 'i' | 'x' | 'o'; lhs: readonly VimKey[]; target: MapTarget; desc: string | null; path: string }
export interface CompiledKeymap {
  leader: VimKey; timeoutlen: number;
  maps: Readonly<Record<'n' | 'i' | 'x' | 'o', readonly CompiledMapping[]>>;
  cmdline: ReadonlyMap<VimKey, VimKey>;
  hints: { enabled: boolean; delayMs: number; triggers: Readonly<Record<'n' | 'x' | 'o', readonly (readonly VimKey[])[]>> };
  hop: { enabled: boolean; keys: string; uppercaseLabels: boolean };
}
export interface KeymapIssue { path: string; severity: 'error' | 'warning'; message: string }
export function compileKeymap(text: string): { keymap: CompiledKeymap | null; issues: KeymapIssue[] };
```

Compilation rules (design 2.1-2.3, 4.2 step 6, 4.4.1, 4.5, 7.5):

- Size over `KEYMAP_MAX_BYTES` (UTF-8, use `new TextEncoder().encode(text).length`),
  invalid JSON, non-object root, `version` missing or not the integer 1
  -> `keymap: null` with exactly one error at path `$` or `version`.
- Defaults: `leader` `\`, `timeoutlen` 1000 (50..10000), hints disabled,
  `delayMs` 200 (0..5000), no triggers, hop disabled, `DEFAULT_HOP_KEYS`,
  `uppercaseLabels` false. An invalid scalar keeps its default and adds an
  error at its path.
- `leader` must parse (with leader null) to exactly one key.
- `maps.v` entries are appended to `x` in document order. Unknown mode
  keys or unknown fields: warning, ignored. Entries beyond 2,000 in total:
  skipped with one error.
- Entry: `lhs` 1..16 keys (`<leader>` expanded); exactly one of `rhs`
  (string) and `action`; `remap` boolean only with key-sequence `rhs`;
  `desc` string <= 80 chars without control characters. `rhs` exactly
  `<Nop>` -> `{kind:'nop'}`; else 1..256 keys.
- Reserved first lhs key (design 2.3): in n/i/x/o `<C-a> <C-z> <C-y> <C-c> <C-x> <C-v>`;
  in c only `<C-c> <C-x> <C-v>`. Error text
  `reserved for a platform or live-coding shortcut`.
- `:` segments (n/x/o only): after parsing rhs keys, split them: a `:`
  key followed by keys up to the first `<CR>` becomes `{kind:'ex', command}`
  (the command is the joined printable keys; a non-printable key inside
  is an error); a `:` as the very last key becomes `{kind:'prompt'}`; a
  `:` followed by keys with no `<CR>` is an error. In mode `i`, `:` is
  literal text and never splits.
- Mode `i` rhs keys must be printable or one of
  `<Esc> <BS> <Del> <CR> <Tab> <Left> <Right> <Up> <Down> <Home> <End>`.
- Mode `c`: lhs exactly one key; rhs exactly one key from
  `<Esc> <CR> <Home> <End> <Left> <Right> <BS> <Del>` or one printable;
  `action`/`remap` are errors; stored in `cmdline`.
- Action entries: unknown id or id not allowed in the mode -> error.
- Last-wins: a later entry with the same mode and lhs replaces the
  earlier one (keep document order of the survivor at its last position).
- Hop off (design 7.5): when `hop.enabled` is false, drop every mapping
  whose target is a `hop.*` action, silently (no issue).
- `hints.triggers`: keys n/x/o (`v` -> x), each an array (<= 32) of
  notation strings; each parses to >= 1 key.
- `hop.keys`: 2..64 printable ASCII, no spaces, no duplicates
  (case-insensitive when `uppercaseLabels` is true); invalid -> default
  with an error.
- `unsupported`: must be an array of objects, otherwise a warning; never
  compiled.

Pitfalls: never throw (wrap `JSON.parse`); paths use the form
`maps.n[3].rhs`; do not mutate the parsed JSON; keep each issue message
human readable and include the offending value where short.

## Invariants

- Pure module: no DOM, no timers (purity test).
- `compileKeymap` is deterministic for the same input.

## Test Cases

`editor/test/code/vim/notation.test.ts` (at least 25 cases):
- `'kj'` -> `['k','j']`; `'<Space>y'` -> `[' ','y']`; `'<esc><ESC>'` -> `['<Esc>','<Esc>']`
- `'<C-H>'` -> `['<C-h>']`; `'<C-[>'` -> `['<Esc>']`; `'<lt>'` -> `['<']`; `'a<b'` -> `['a','<','b']`
- `'<leader>y'` with leader `' '` -> `[' ','y']`; with null -> error
- `'<M-x>'`, `'<F1>'`, `'<S-Tab>'` -> error containing the token
- `formatNotation([' ','y','<'])` -> `'<Space>y<lt>'`; round-trip of every table row

`editor/test/code/vim/keymap-schema.test.ts` (at least 35 cases):
- `'{'` -> keymap null, one error; `'{"version":2}'` -> null; 262,145-byte text -> null
- `'{"version":1}'` -> defaults exactly as listed, no issues
- `rhs` and `action` both present -> error at `maps.n[0]`, entry skipped, other entries kept
- `remap` with `action` -> error; `<Nop>` -> `{kind:'nop'}`
- `{"lhs":"<C-a>","rhs":"x"}` in n -> reserved error; `{"lhs":"<C-a>","rhs":"<Home>"}` in c -> allowed (c reserves only `<C-c> <C-x> <C-v>`); `<C-v>` in c -> reserved error
- `":nohlsearch<CR>"` in n -> `[{kind:'ex', command:'nohlsearch'}]`; `"dd:"` -> keys then prompt; `":foo"` -> error; `":x<CR>"` in i -> plain keys
- i-mode rhs `<F1>` -> error (notation) and `<C-x>` -> error (not allowed in i)
- c-mode two-key lhs -> error; c rhs `<Home>` -> cmdline map entry
- duplicates: two `kj` entries -> last wins; `v` entries land in `x`
- hop disabled: `gj -> hop.char1` absent from `maps.n`; enabled: present
- 2,001 entries -> 2,000 compiled plus one error
- unknown top-level field -> warning; invalid `timeoutlen: 5` -> default 1000 plus error
- `hop.keys: "aa"` -> default plus error; `uppercaseLabels: true` with `"aA"` -> error

`editor/test/code/vim/actions.test.ts`: every id has the design modes; `isActionId('nope')` false.

## Verification (from editor/)

1. Scoped typecheck including the three new source files and three new tests -> exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/notation.test.ts test/code/vim/keymap-schema.test.ts test/code/vim/actions.test.ts test/code/vim/purity.test.ts` -> exit 0, failureCount 0, at least 62 tests.

## Completion Criteria

- [ ] Exports match the pinned signatures exactly
- [ ] All three files within budget; purity test passes
- [ ] At least 62 new passing test cases
- [ ] Verification records logged; Progress Log entry added

## Progress Log

(none yet)
