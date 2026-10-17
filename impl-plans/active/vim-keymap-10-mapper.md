# Vim Keymap: KeyMapper (resolution, timeouts, remap) Implementation Plan

**Status**: Completed (implementation; independent review and serial integration pending)
**Plan ID**: VK-10 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 4.1, 4.2, 4.4, 4.6, 6.3
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (types: `MapMode`, `VimHost.timer` shape), VK-01 (`CompiledKeymap`, `RhsSegment`, `ActionId`)
- **Parallel (wave 1)**: VK-11, VK-12, VK-13, VK-14, VK-15, VK-16
- **Consumer**: VK-20 controller implements `MapperSink` and owns `routeKey`
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

The mapper sits between `normalizeKey` and the controller's built-in key
handling. It buffers typed keys per map mode, resolves them against the
compiled lhs set with Vim's timeout and longest-prefix rules, and emits
built-in keys, routed keys, actions and ex commands to a sink. The
controller (VK-20) owns `routeKey` (hop session -> `awaitingArgument` ->
mapper); the mapper only calls back through the sink. This plan is pure
and timer-injected.

## Non-Goals

- No controller, parser, hint or hop code. No `c`-mode handling (the
  status view does that with `CompiledKeymap.cmdline`).
- Do not decide argument bypass here; the controller never calls `feed`
  for argument keys.

## File-Level Changes

### editor/src/code/vim/mapper.ts (new, budget 260 lines)

```ts
export interface MapperSink {
  builtin(key: VimKey): void;          // noremap rhs keys and unmatched first keys -> controller processKey
  route(key: VimKey): void;            // leftover keys and remap rhs keys -> controller routeKey (full 4.1 ordering)
  action(id: ActionId): void;
  ex(command: string): void;           // rhs ':cmd<CR>' segment
  prompt(): void;                      // rhs trailing ':'
  error(message: string): void;        // 'E223 recursive mapping'
  changed(): void;                     // buffer or pending text changed (snapshot refresh)
}
export type FeedResult = 'consumed' | 'unmapped';
export class KeyMapper {
  constructor(keymap: CompiledKeymap, timer: (ms: number, cb: () => void) => () => void, sink: MapperSink);
  feed(key: VimKey, mode: 'n' | 'i' | 'x' | 'o'): FeedResult;
  flush(): void;                       // resolve the buffer as on a timeout
  pending(): string;                   // formatNotation(buffer)
  buffer(): readonly VimKey[];
  mode(): 'n' | 'i' | 'x' | 'o' | null;
  suspendTimeout(on: boolean): void;   // hint panel visible
  setReplay(on: boolean): void;        // macro replay: no timers
}
```

Resolution (design 4.2):

1. `feed`: if the buffer is non-empty and `mode` differs from the buffer
   mode, `flush()` first. If the buffer is empty and `key` starts no lhs
   in `mode`, return `'unmapped'` without calling the sink (the
   controller then calls `processKey` itself and returns its result; this
   keeps insert-mode native input untouched).
2. Push the key. Exact match and not a prefix of a longer lhs -> run it.
   Proper prefix of some lhs -> keep waiting, (re)start the timer with
   `timeoutlen` unless suspended or in replay, call `sink.changed()`,
   return `'consumed'`. No match -> resolve (step 4).
3. Timer fire or `flush()` -> resolve.
4. Resolve: copy and clear the buffer and cancel the timer **before**
   emitting anything. Find the longest lhs equal to a prefix of the
   copied keys; if found, run it and `sink.route` each remaining key in
   order; else `sink.builtin(first)` then `sink.route` each remaining key.
   Leftover keys must go to `sink.route`, never back into internal step 1
   directly (DR-K1: the controller's `routeKey` checks hop and
   `awaitingArgument` first).
5. Run a mapping: `nop` -> nothing; `action` -> `sink.action(id)`;
   `keys`: for each segment, `ex` -> `sink.ex`, `prompt` -> `sink.prompt`,
   `keys` -> each key to `sink.builtin` when `remap` is false, or to
   `sink.route` when `remap` is true.
6. Recursion: keep a depth counter incremented around a remap emission
   and a per-top-level-feed emitted-key counter. Depth > 100 or > 1,000
   emitted keys -> `sink.error('E223 recursive mapping')`, drop the rest
   of the expansion, clear the buffer. Reset the emitted counter when a
   top-level (depth 0) `feed` starts.
7. `<Esc>` is an ordinary key (so `<Esc><Esc>` maps).
8. `suspendTimeout(true)` cancels a running timer; `false` restarts it
   for a non-empty buffer. `setReplay(true)` cancels timers and never
   starts new ones.

Use a trie or a sorted lhs list per mode built once in the constructor;
lookups must be O(lhs length) per key, not a scan of all mappings.

Pitfalls: re-entrancy (sink callbacks call `feed` again through
`routeKey`); never hold stale buffer state across a sink call; never
throw.

## Invariants

- Pure; timers only via the injected function (purity test).
- `'unmapped'` is returned only when nothing was buffered and the sink was
  not called.

## Test Cases (`editor/test/code/vim/mapper.test.ts`, fake timers, a recording sink whose `route` calls back into `feed` with mode `n` unless a stub `awaitingArgument` flag says to call `builtin`)

- n map `k -> gk` noremap: feed `k` -> builtin `g`,`k`; consumed
- `kj -> <Esc>` in i: feed `k` -> consumed, nothing emitted; feed `j` -> builtin `<Esc>`
- `kj` in i: feed `k`, advance 1000 ms -> builtin `k`; feed `k` then `a` -> builtin `k`, route `a`
- empty buffer, `x` not an lhs start -> `'unmapped'`, sink untouched
- `<Esc><Esc> -> :nohlsearch<CR>`: two Esc -> `ex('nohlsearch')`; one Esc + 1000 ms -> builtin `<Esc>`
- longest prefix: maps `a -> x`, `abc -> y`; feed `a`,`b`,`d` -> builtin `x`, route `b`, route `d`
- remap: `a -> b` (remap), `b -> c` -> builtin `c`; `a -> a` remap -> error E223 once, no hang
- `<Nop>`: `Q` -> nothing emitted, consumed
- action: `gj -> hop.char1` -> `action('hop.char1')`
- trailing `:` -> `prompt()`
- suspendTimeout: `<Space>` prefix, suspend, advance 5000 ms -> still buffered; unsuspend + 1000 ms -> resolved
- setReplay: prefix buffered, advance time -> no resolution until `flush()`
- DR-K1 re-entry: `g` prefix (maps `gj`), feed `g`,`g` -> builtin `g`, then route `g` (the test sink, flagged as awaiting an argument, sends it to builtin) -> builtin sequence `g`,`g` with no timer pending
- mode change with a buffer: buffer `k` in i, feed in n -> flush first

## Verification (from editor/)

1. Scoped typecheck including `src/code/vim/mapper.ts` and the test -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim/mapper.test.ts test/code/vim/purity.test.ts` -> exit 0, failureCount 0, at least 16 cases

## Completion Criteria

- [x] Pinned API exactly; file within budget; purity passes
- [x] All listed cases pass, including the DR-K1 re-entry case
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-10 implementation

**Tasks Completed**: Added the pure KeyMapper trie and timer lifecycle, resolution/replay/recursion behavior, remap and leftover `sink.route` re-entry, and 24 focused mapper/purity tests. DR-K1 regressions cover normal `gg`, `zz`, `mk`, `mgj`, a remapped `mgj` sequence, and visual-mode `gg`. Mapper is 205 lines (260-line budget); test file is 230 lines.

**Source Hashes**:
- `editor/src/code/vim/mapper.ts`: `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986`
- `editor/test/code/vim/mapper.test.ts`: `ef266bb64e8905a9fed480b9ce4e1156c853a7ec`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-1/final-typecheck.log", notes: "Scoped to mapper.ts and mapper.test.ts; exitStatus=0."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/mapper.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 24, testsPassed: 24, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-1/final-vitest.log", notes: "2 files passed; includes all 23 mapper cases and the purity suite."}`
- `{command: "git diff --check -- editor/src/code/vim/mapper.ts editor/test/code/vim/mapper.test.ts impl-plans/active/vim-keymap-10-mapper.md", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "inline (command output)", notes: "No whitespace errors."}`

**Superseded Attempts**:
- Initial focused vitest run: exit 1, 10 harness expectation failures / 24 tests (`tmp/vim-keymap/VK-10/attempt-1/vitest.log`). Corrected route fallback expectations and parser-state fixture behavior.
- Retry 2 focused vitest run: exit 1, 2 harness assertion failures / 24 tests (`tmp/vim-keymap/VK-10/attempt-1/retry-2-vitest.log`). Corrected the prompt counter getter and accounted for routed re-entry.

**Evidence**: Per-edit intentions and complete verification logs are under `tmp/vim-keymap/VK-10/attempt-1/`. Scoped tsconfig is `tmp/vim-keymap/VK-10/tsconfig.json`.

**Drift**: Shared worktree contains changes owned by concurrent plans, including mapper dependencies in `types.ts`, `keymap-schema.ts`, `notation.ts`, and `actions.ts`; preserved as read-only inputs. No other paths were edited.

**Downstream**: Independent test-integrity/adversarial review and combined-tree integration are pending. VK-20 owns controller integration and confirms the route callback contract against the production parser.

### Session: 2026-10-17 VK-10 test-integrity repair

**Finding Repaired**: `VK10-TI-1` (`comm-007156`). Replaced the direct-recursion emitted-limit fixture with a depth-2 fan-out (`a -> b*256`, `b -> cccc`). The test asserts E223 once, empty buffer, positive truncated builtin output under 1024, and a second E223 after a new top-level feed. That second-feed assertion verifies `recursionFailed` reset only; `VK10-TI-2` below separately verifies emitted-key counter reset. The direct `a -> a` depth-limit test and all DR-K1 cases remain unchanged.

**Source Hashes**:
- `editor/src/code/vim/mapper.ts`: `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986` (unchanged after mutation restore)
- `editor/test/code/vim/mapper.test.ts`: `e69c974e83eaf1472ce277323cc28f1777f7a46a`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-2/typecheck.log", notes: "Scoped mapper and test typecheck on repaired test source."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/mapper.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 24, testsPassed: 24, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-2/vitest.log", notes: "2 files passed on repaired test source."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/mapper.test.ts -t 'stops a fan-out remap expansion after 1,000 emitted keys'", exitStatus: 0, testsRun: 1, testsPassed: 1, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-2/fanout-positive.log", notes: "New cap regression passes with production guard present."}`

**Mutation Evidence**: Temporarily removed the emitted-key comparison from `mapper.ts`; the new test failed (1 failed, 22 skipped) because no E223 was produced, demonstrating cap sensitivity. Full log: `tmp/vim-keymap/VK-10/attempt-2/mutation-cap-removed.log`. Restored from `mapper.pre-mutation.ts`; current mapper hash exactly matches the pre-mutation `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986`. Mutation source copy and intent are under `tmp/vim-keymap/VK-10/attempt-2/`.

### Session: 2026-10-17 VK-10 Step 6 rerun

**Tasks Completed**: Rechecked the existing `VK10-TI-1` repair and the DR-K1 re-entry coverage on the current shared-tree source. No implementation source changes were needed. Updated the plan status to distinguish completed implementation from pending independent review and serial integration.

**Source Hashes**:
- `editor/src/code/vim/mapper.ts`: `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986`
- `editor/test/code/vim/mapper.test.ts`: `e69c974e83eaf1472ce277323cc28f1777f7a46a`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-3/retry/typecheck.log", notes: "Scoped typecheck includes mapper.ts and mapper.test.ts."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/mapper.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 24, testsPassed: 24, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-3/vitest.log", notes: "Current source passed all mapper and purity tests."}`

**Evidence**: The initial typecheck log wrapper used zsh's reserved `status` variable and did not record a command exit; the successful exact rerun and complete log are preserved under `tmp/vim-keymap/VK-10/attempt-3/retry/`.

**Downstream**: Independent review of the repaired finding and combined-tree integration/finalization remain pending; they are not VK-10 implementation gates.

### Session: 2026-10-17 VK-10 test-integrity repair VK10-TI-2

**Finding Repaired**: `VK10-TI-2` (`comm-007177`). Added `resets the emitted-key budget at each top-level feed` beside the fan-out guard test. With `a -> b.repeat(200)` and `b -> cc`, each top-level `a` emits 600 remap keys and yields 400 `c` builtins. Two feeds produce 800 `c` builtins without E223, proving the budget resets between feeds. The prior fan-out test's second-feed assertion is now explicitly described as checking only the `recursionFailed` reset.

**Source Hashes**:
- `editor/src/code/vim/mapper.ts`: `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986` (restored byte-identically after negative control)
- `editor/test/code/vim/mapper.test.ts`: `ee4c3f15a06c8d8883acd8f9e3c49cc6bdea69f6`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-10/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-4/typecheck.log", notes: "Scoped typecheck on restored mapper and updated mapper tests."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/mapper.test.ts test/code/vim/purity.test.ts", exitStatus: 0, testsRun: 25, testsPassed: 25, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-4/vitest.log", notes: "All existing mapper/purity cases and VK10-TI-2 pass on restored source."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim/mapper.test.ts -t 'resets the emitted-key budget at each top-level feed'", exitStatus: 0, testsRun: 1, testsPassed: 1, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-10/attempt-4/reset-positive.log", notes: "New regression passes with the production reset present."}`

**Mutation Evidence**: Saved exact pre-mutation source at `tmp/vim-keymap/VK-10/attempt-4/mapper.pre-mutation.ts`, then removed only `this.emittedKeys = 0;` while retaining `this.recursionFailed = false;`. The new test failed as expected: one failed, 23 skipped, exitStatus=1, with E223 on the second top-level feed (`tmp/vim-keymap/VK-10/attempt-4/mutation-reset-removed.log`). Restored mapper.ts from the saved copy; `git hash-object` returned `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986`.
