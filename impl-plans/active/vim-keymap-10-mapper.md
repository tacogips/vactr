# Vim Keymap: KeyMapper (resolution, timeouts, remap) Implementation Plan

**Status**: Ready
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

- [ ] Pinned API exactly; file within budget; purity passes
- [ ] All listed cases pass, including the DR-K1 re-entry case
- [ ] Progress Log entry added

## Progress Log

(none yet)
