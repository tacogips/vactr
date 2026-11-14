# Vim Keymap: Notation, Schema Compiler and Action Table Implementation Plan

**Status**: Completed
**Plan ID**: VK-01 (wave 0)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 2, 3, 4.2 (step 6), 4.3, 4.4.1, 4.5, 7.5, 8.1
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Parallel (wave 0)**: VK-00, VK-02, VK-03, VK-04
- **Next**: VK-10 mapper, VK-11 hints, VK-14 presets, VK-16 views consume `CompiledKeymap`, `KeymapIssue`, `formatNotation`
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full)

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

- [x] Exports match the pinned signatures exactly
- [x] All three files within budget; purity test passes
- [x] At least 62 new passing test cases
- [x] Verification records logged; Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-01 implementation
**Tasks Completed**: Notation parser/formatter, action ID table, schema compiler, and focused tests.
**Source hashes**: notation.ts `124c1b29a8d356956b7f8fc30f3e5aaa895bc0e1`; actions.ts `aa05e1e83dc2ea5d6efd69fa94eef9866ec77559`; keymap-schema.ts `c95643dd240786d811737c0a0c24340d882cc8a7`; notation.test.ts `202c61c4fd91ed2a39261251313aab354b511c38`; keymap-schema.test.ts `284a23c0750aef3e37e699ff6c4fffeb3687fa9a`; actions.test.ts `9b78935d5c8ac1fcbc1c39d26976c96c49e693c6`.
**Notes**: The initial scoped typecheck exposed a test-only union narrowing error (`tmp/vim-keymap/VK-01/attempt-1/typecheck.log`, exit 1); the assertion was narrowed by target kind and the exact required typecheck then passed at `tmp/vim-keymap/VK-01/attempt-2/typecheck.log`. The action-mode assertions were tightened to match the accepted design table and exercise an incompatible mode. Adversarial finding VK01-ADV-1 was repaired by segmenting plain runs once, remembering a failed closing-angle search, and appending segments iteratively to avoid argument-stack limits. Attempt-3 long-input tests exposed the spread-call limit (Vitest exit 1); the correction passed all attempt-4 checks. The earlier mapper re-entry review finding concerns mapper/controller routing and regression tests allocated to downstream VK-10/VK-13/VK-20; no mapper behavior is in VK-01 writePaths.
**Verification**:
- {"command":"cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-01/tsconfig.json","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-2/typecheck.log","notes":"Scoped VK-01 sources and tests."}
- {"command":"cd editor && ./node_modules/.bin/vitest run test/code/vim/notation.test.ts test/code/vim/keymap-schema.test.ts test/code/vim/actions.test.ts test/code/vim/purity.test.ts","exitStatus":0,"testsRun":123,"testsPassed":123,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-2/vitest.log","notes":"Four files passed; 123 total passing tests, including purity and more than 62 new cases."}


### Session: 2026-10-17 VK-01 adversarial repair
**Tasks Completed**: Repaired linear-time notation parsing; added 200,000-character plain and unmatched-angle tests, mixed long-run token parsing, and a capped long-rhs compile test that verifies invalid-entry skipping and sibling retention; corrected hash labels.
**Verification**:
- {"command":"cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-01/tsconfig.json","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-4/typecheck.log","notes":"Scoped VK-01 sources and tests."}
- {"command":"cd editor && ./node_modules/.bin/vitest run test/code/vim/notation.test.ts test/code/vim/keymap-schema.test.ts test/code/vim/actions.test.ts test/code/vim/purity.test.ts","exitStatus":0,"testsRun":127,"testsPassed":127,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-4/vitest.log","notes":"Four files passed, including purity and all long-input regressions."}
- {"command":"node --experimental-strip-types --no-warnings --input-type=module -e \"const {parseNotation}=await import('./src/code/vim/notation.ts'); for (const n of [2000,8000,32000,250000]) { const t=performance.now(); const r=parseNotation('a'.repeat(n), null); console.log('plain',n,Math.round(performance.now()-t)+'ms',r.keys?.length); } for (const n of [250000]) { const t=performance.now(); const r=parseNotation('<'.repeat(n), null); console.log('unmatched-angle',n,Math.round(performance.now()-t)+'ms',r.keys?.length); }\"","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-4/timing.log","notes":"Plain inputs: 2K 0ms, 8K 1ms, 32K 2ms, 250K 15ms. Unmatched angle run: 250K 2ms."}
**Prior attempt**: attempt-3 Vitest initially exited 1 because `keys.push(...segments)` exceeded the argument stack for 200,000 keys; fixed with iterative appends. Full output is preserved in `tmp/vim-keymap/VK-01/attempt-3/vitest.log`.

### Session: 2026-10-17 VK-01 current-source verification
**Tasks Completed**: Rechecked VK01-ADV-1's linear-time parser correction on the current shared source tree; no implementation-source edits were needed in this session.
**Source hashes (git blob)**: notation.ts `124c1b29a8d356956b7f8fc30f3e5aaa895bc0e1`; actions.ts `aa05e1e83dc2ea5d6efd69fa94eef9866ec77559`; keymap-schema.ts `c95643dd240786d811737c0a0c24340d882cc8a7`; notation.test.ts `202c61c4fd91ed2a39261251313aab354b511c38`; keymap-schema.test.ts `284a23c0750aef3e37e699ff6c4fffeb3687fa9a`; actions.test.ts `9b78935d5c8ac1fcbc1c39d26976c96c49e693c6`.
**Notes**: An initial attempt to append the typecheck exit used zsh's read-only `status` variable; the typecheck was rerun with `rc`, and the complete attempt-5 log records exit 0. Current-source parser timing: 250,000 plain characters in 16 ms and 250,000 unmatched `<` characters in 2 ms. Formal independent review and downstream mapper/integration work remain assigned to later workflow plans.
**Verification**:
- {"command":"cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-01/tsconfig.json","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-5/typecheck.log","notes":"Scoped VK-01 sources and tests on the current shared tree."}
- {"command":"cd editor && ./node_modules/.bin/vitest run test/code/vim/notation.test.ts test/code/vim/keymap-schema.test.ts test/code/vim/actions.test.ts test/code/vim/purity.test.ts","exitStatus":0,"testsRun":127,"testsPassed":127,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-5/vitest.log","notes":"Four files passed, including purity and all long-input regressions."}
- {"command":"cd editor && node --experimental-strip-types --no-warnings --input-type=module -e \"const {parseNotation}=await import('./src/code/vim/notation.ts'); for (const n of [2000,8000,32000,250000]) { const t=performance.now(); const r=parseNotation('a'.repeat(n), null); console.log('plain',n,Math.round(performance.now()-t)+'ms',r.keys?.length); } const n=250000; const t=performance.now(); const r=parseNotation('<'.repeat(n), null); console.log('unmatched-angle',n,Math.round(performance.now()-t)+'ms',r.keys?.length);\"","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/vim-keymap/VK-01/attempt-5/timing.log","notes":"250K plain parsed in 16 ms; 250K unmatched-angle parsed in 2 ms."}
