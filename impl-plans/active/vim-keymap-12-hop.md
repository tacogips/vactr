# Vim Keymap: Hop Targets, Labels and Session Implementation Plan

**Status**: Completed (implementation; serial integration pending)
**Plan ID**: VK-12 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 7.1-7.3, 7.5
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`HopView`, `HopLabel`, `VimViewport`)
- **Parallel (wave 1)**: VK-10, VK-11, VK-13, VK-14, VK-15, VK-16
- **Consumer**: VK-20 starts sessions from `hop.*` actions and applies jumps; VK-04/VK-30 draw `HopView.labels`
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

Hop (easymotion) jumps: after `hop.char1` the user types one character,
every visible occurrence gets a label from `hop.keys`, and typing a label
jumps there. This plan delivers the pure engine: target collection, the
prefix-free label assignment, and a small state machine. Applying the jump
(normal, visual, operator) is VK-20.

## Non-Goals

- No controller, renderer or DOM code. No dot-repeat handling.
- Do not copy hop.nvim code or its label algorithm; implement the design
  rule below.

## File-Level Changes

### editor/src/code/vim/hop.ts (new, budget 260 lines)

```ts
export type HopKind = 'char1' | 'char1Line' | 'word' | 'line';
export const HOP_TARGET_LIMIT = 4096;
export interface HopSource { doc: Text; cursor: number; viewport: VimViewport; isVisible(pos: number): boolean }
export interface HopSettings { keys: string; uppercaseLabels: boolean }
export function hopTargets(source: HopSource, kind: HopKind, char: string | null): number[];
export function assignLabels(count: number, keys: string): string[];
export type HopStep = { kind: 'pending'; view: HopView | null; prompt: string } | { kind: 'jump'; pos: number } | { kind: 'cancel'; message: string | null };
export class HopSession {
  constructor(kind: HopKind, settings: HopSettings, source: HopSource);
  start(): HopStep;
  feed(key: VimKey): HopStep;
}
```

Targets (design 7.2):
- Visible lines are `viewport.topLine` .. `topLine + rows - 1`, clamped to
  the document; `char1Line` uses only the cursor line.
- `char1`/`char1Line`: every grapheme start whose grapheme equals the
  typed character; smartcase: a typed lowercase ASCII letter also matches
  its uppercase; otherwise exact.
- `word`: the start of every run of class 1 or 2 from
  `words.ts:classAt`/`charClass` (blank = class 0) on visible lines.
- `line`: `words.ts:firstNonBlank` of each visible non-empty line.
- Exclude the cursor position and positions where `isVisible(pos)` is
  false. Order: by line distance from the cursor line, then column
  distance (grapheme columns via `graphemeColumn`), then forward before
  backward. Truncate to `HOP_TARGET_LIMIT`.

Labels (design 7.3): start with the single keys of `keys` in order. While
`labels.length < count`, remove the **last** label `L` and append
`L + k` for each key `k` of `keys` in order. Return the first `count`
labels. Pitfall: this must be prefix-free; test it for 1, 26, 27, 100 and
700 targets.

Session:
- `start()`: for `char1`/`char1Line` return
  `{kind:'pending', view:null, prompt:'hop'}` (waiting for the character);
  for `word`/`line` compute targets immediately.
- With targets computed: none -> `{kind:'cancel', message:'hop: no match'}`;
  else `pending` with a view whose labels are the assigned labels
  (uppercased for display when `uppercaseLabels`), `revision` starting at
  1 and incremented on every view change, and `prompt` `hop: N targets`.
- `feed(key)` while waiting for the character: `<Esc>` or a key that is
  not a single printable grapheme -> cancel (message null).
- `feed(key)` while labeling: append to the typed label prefix (compare
  case-insensitively when `uppercaseLabels`, exactly otherwise). Exactly
  one complete label -> `{kind:'jump', pos}`; still a prefix of some labels
  -> pending view showing only matching targets with their remaining
  suffix; no match or `<Esc>` -> cancel (message null).

## Invariants

- Pure, deterministic; no timers.

## Test Cases (`editor/test/code/vim/hop.test.ts`)

- `assignLabels(3, 'abc')` -> `['a','b','c']`; `(4, 'abc')` -> `['a','b','ca','cb']`; prefix-free and unique for 27, 100, 700 with the default keys
- char1 on `foo bar|\nfoo` (viewport rows 10) typed `o` -> targets ordered nearest first, cursor excluded
- smartcase: `F` and `f` in text, typed `f` -> both; typed `F` -> only `F`
- char1Line ignores other lines; `isVisible` false positions excluded; viewport limits lines
- word targets on `ab  cd.ef` -> starts of `ab`, `cd`, `.`, `ef` (minus the cursor)
- line targets skip empty lines and use first non-blank
- session: char1 -> pending view null; feed `o` -> view with labels; feed first label -> jump to the nearest target
- multi-char: 30 targets with keys `ab...` -> feeding the first char of a two-char label narrows the view (revision increments) and the second char jumps
- `uppercaseLabels`: labels display uppercase, typing lowercase matches
- `<Esc>` while labeling -> cancel; no targets -> cancel with `hop: no match`

## Verification (from editor/)

1. Scoped typecheck including `src/code/vim/hop.ts` and the test -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim/hop.test.ts test/code/vim/purity.test.ts` -> exit 0, failureCount 0, at least 14 cases

## Completion Criteria

- [x] Pinned API; file within budget; purity passes
- [x] At least 14 passing cases, including prefix-freeness and the 4,096 cap
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 implementation

**Tasks completed**: VK-12 pure hop engine and unit coverage.

**Files**:
- `editor/src/code/vim/hop.ts` (157 lines; blob `333b4c53d2d712c3def6c25360d0adc985d20ef1`): pinned exports, visible target collection, smartcase, ordering, 4096 cap, prefix-free labels and hop session.
- `editor/test/code/vim/hop.test.ts` (88 lines; blob `32b8e117530ac18f7b36b4d300e27aaa93c1cb24`): 22 hop cases including label sizes 1/26/27/100/700, session flow and cap.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-12/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-12/attempt-2/scoped-typecheck.log", notes: "Scoped to hop.ts and hop.test.ts; exit=0."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/hop.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 24, testsPassed: 24, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-12/attempt-2/vitest.log", notes: "Two files passed; 22 hop assertions plus 2 purity assertions."}`

**Prior attempt**: `tmp/vim-keymap/VK-12/attempt-1/vitest.log` exited 1 (6/24 assertions). The failures were incorrect expected positions/order in newly authored fixtures and prompted a non-printable-grapheme guard; attempt 2 passed on final source.

**Notes**: Dependency VK-00 was admitted by `acceptedPlanIds`. No controller, renderer, or DOM integration was added; those seams remain assigned to VK-20/VK-04/VK-30. Formal review and serial combined-tree verification remain downstream.

### Session: 2026-10-17 current-tree verification rerun

**Tasks completed**: Re-ran VK-12's scoped typecheck and hop/purity tests on the current shared tree; implementation source was unchanged.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-12/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-12/attempt-3/scoped-typecheck.log", notes: "Current-tree scoped typecheck; exit=0."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/hop.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 24, testsPassed: 24, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-12/attempt-3/vitest.log", notes: "Two files passed; exit=0."}`

**Notes**: The DR-K1 mapper re-entry finding remains assigned to VK-10/VK-13/VK-20 and combined integration review; it is outside VK-12's pure hop engine contract. Controller and renderer integration remain assigned to downstream plans.

### Session: 2026-10-17 (owner fix after session-398 VK-30 blocker)
**Notes**: `hopTargets` ordering recomputed `graphemeColumn` inside the sort comparator, which is quadratic on long lines (the 5,000-character cap case took 1.3 s alone and timed out under aggregate load). Line and grapheme column are now recorded once per target during the scan and the sort uses precomputed keys; ordering is unchanged. Cap case 1 ms; `hop.test.ts` 23/23; VK-30 aggregate `vitest run test/canvas test/code/vim test/code/vim-keymap-store.test.ts test/platform` 1537/1537.
