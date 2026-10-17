# Vim Keymap: Controller Integration (routeKey, mapper, hints, hop, extended) Implementation Plan

**Status**: Completed (implementation; independent review and serial integration pending)
**Plan ID**: VK-20 (wave 2)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 1, 4.1-4.4, 4.6, 5, 6.2, 6.3, 7.1, 7.5, 8.2, 11.1
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00, VK-01, VK-10 (mapper), VK-11 (hints), VK-12 (hop), VK-13 (parser/extended/surround/change list), VK-14 (presets for regressions)
- **Next**: VK-30 mount wiring (wave 3)
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full; this wave runs the full `npm run check`)

## Intent and Context

`editor/src/code/vim/controller.ts` (`VimController`, 600 lines) is the
merged Vim mode's mode machine. This plan connects the wave-1 pieces:
the `routeKey` ordering (hop session -> `awaitingArgument` -> mapper),
the mapper sink, action dispatch, the hint session and its timers, the
hop session and jumps, execution of the extended built-ins and surround,
the macro recording point, `setKeymap` live apply, forced flushes and the
snapshot fields. With no keymap (K0) the controller must behave exactly
as today.

To keep `controller.ts` under its budget, put the keymap glue (mapper
sink, hint timer/session, hop session state) in a new
`vim/keymap-runtime.ts` class that the controller owns; the controller
keeps only short call sites.

## Non-Goals

- No DOM, storage or mount changes (VK-30). No change to `parser.ts`,
  `keys.ts`, `mapper.ts`, `hints.ts`, `hop.ts`, `surround.ts`,
  `extended.ts` (report defects in their plans' terms in the Progress
  Log; do not edit them).
- No new Vim features beyond the design.

## File-Level Changes

### editor/src/code/vim/keymap-runtime.ts (new, budget 300 lines)

A class used only by `VimController`, e.g.

```ts
export interface RuntimeHost { /* controller callbacks: processKey, routeKey, runAction, executeEx, openPrompt, message, refresh, mapMode, parserPending, hasCountOrRegister, isReplaying, promptOpen, extended */ }
export class KeymapRuntime {
  constructor(keymap: CompiledKeymap, host: VimHost, cb: RuntimeHost);
  feed(key: VimKey): FeedResult;                 // KeyMapper.feed with the current map mode
  flush(): void; pending(): string; dispose(): void;
  afterKey(): void;                              // hint session update after any key/flush
  hints(): HintView | null;
  closeHints(): void;
  startHop(kind: HopKind, source: HopSource): HopStep;
  hopActive(): boolean; feedHop(key: VimKey): HopStep; hopView(): HopView | null; endHop(): void;
}
```
The exact internal shape is yours; the controller-facing behavior below
is the contract.

### editor/src/code/vim/controller.ts (budget: at most 850 lines; run `wc -l`)

1. **State.** `private runtime: KeymapRuntime | null = null`,
   `private extended = false`, `private changes = new ChangeList()`.
   `onUpdate` records every `docChanged` update into `changes`.
2. **`setKeymap(keymap: CompiledKeymap | null): void`.** Flush the mapper
   (resolve as timeout), end hint and hop sessions, dispose the old
   runtime, set `this.extended = keymap !== null`,
   `this.parser.setExtended(this.extended)`, create the new runtime when
   non-null, `refresh()`. Document, selection, mode, registers, marks and
   macros are kept.
3. **`handleKey`.** Keep the composition guard and P check first. Call
   `normalizeKey(input, this.extended)`. When `runtime` is null, call the
   existing body (now `processKey(key, input)`) exactly as today. Else:
   record the typed key into the macro (non-insert modes only, same
   condition as today) and call `routeKey(key, input)`; return true when
   consumed, else the `processKey` result.
4. **`routeKey(key, input?)`** (private): hop active -> `feedHop`; else
   `parser.awaitingArgument()` -> `processKey(key)` (built-in, unmapped);
   else `runtime.feed(key)`: `'unmapped'` -> `processKey(key, input)`.
   After each key, `runtime.afterKey()` and `refresh()`. Leftover and
   remap keys from the mapper sink call `routeKey` (DR-K1); noremap keys
   call `processKey`. `processKey` must not push the macro token when the
   runtime exists (it was recorded before routing).
5. **Insert-mode consumption (design 4.4).** In insert/replace mode a key
   that the mapper buffers is consumed (`preventDefault` by returning
   true). Built-in delivery of a held printable key uses
   `host.typeText(key)`; a held or rhs non-printable key goes through
   `handleSessionKey` first, then `host.defaultKey(key)`. `<Esc>` keeps
   the existing exit path.
6. **Mapper sink.** `action(id)`: `vactr.evaluate/evaluateAll/hush/format`
   -> `host.actions.*`; `vactr.completion` -> `host.actions.openCompletion()`;
   `vactr.diagnostic.next/prev` -> `host.actions.diagnosticNext/Prev(cursor)`,
   null -> message `no diagnostics`, else move (normal: selection +
   `scrollCaret`; visual: extend the visual head like a motion);
   `hop.*` -> start a hop session (item 8). `ex(command)` -> the existing
   `executeEx(command)`. `prompt()` -> open the `:` prompt like the
   `prompt` action. `error(msg)` -> error message.
7. **Hint session (design 6.2, 6.3).** Only when `keymap.hints.enabled`,
   map mode n/x/o, no count/register prefix, not replaying, no prompt, no
   hop. When the combined pending keys (parser pending keys + mapper
   buffer) equal a trigger (`isTrigger`), start `host.timer(delayMs)`;
   on fire, if the same keys are still pending, set
   `snapshot.hints = buildHints(...)` and `mapper.suspendTimeout(true)`.
   While shown, every key that leaves a non-empty pending prefix with
   entries updates `hints` at once; otherwise close (`hints = null`,
   `suspendTimeout(false)`). Close on completion, `<Esc>`, invalid key,
   mode change, composition start (`flushPendingInput`), `blur()`, and
   `setKeymap`/`dispose`.
8. **Hop session (design 7.1, 7.5).** Start with a `HopSource` built from
   the document, `cursor()`, `host.viewport()` and `host.isVisible`.
   While active: `allowsTextInput()` returns false; the status pending
   area shows the step prompt (`hop` / `hop: N targets`); the snapshot
   carries `hop: HopView | null`; `announce` the `hop: N targets` text
   once (polite). On `jump`: normal -> set the context mark, dispatch the
   selection, `scrollCaret`; visual/visual-line -> set `visual.head` and
   dispatch the visual selection; operator-pending ->
   `parser.pendingOperator()`, build a `ParsedCommand` with target
   `{kind:'position', pos, inclusive: pos > cursor}`, run it through
   `executeTarget` (handle the `position` target like the search/mark
   path with `motionRange`), and abort the redo record (not
   dot-repeatable). On `cancel`: message when non-null; restore the mode
   (operator-pending -> parser reset, normal). Always clear `hop` from the
   snapshot when the session ends.
9. **Extended execution.** In `executeAction`: `join-raw` -> `joinRaw`
   (one dispatch `vim.join`, leave visual like `join`); `scroll-center`
   -> `host.scrollLines(scrollCenterDelta(...))`; `scroll-line-down/up`
   -> `scrollLineResult` then `scrollLines` and, when it returns a
   cursor, dispatch the selection keeping `desiredColumn`;
   `surround-add` -> resolve the range from the command target exactly as
   `executeTarget` does for operators (motion via `motionRange`, text
   object via `selectTextObject`, `line` via `lineTargetRange`), then
   `surroundAdd`; `surround-change/delete` -> `surroundChange/Delete`.
   Each edit is one dispatch with `isolateHistory('full')` and userEvent
   `vim.surround` / `vim.join`; null outcome -> abort the redo record.
   `execute` must route `surround-*` commands to `executeAction` even
   though they carry a target. Motions `g;`/`g,` in `motion()`:
   `changes.step(-1|1, count)`; error -> message and fail like a failed
   motion; result is charwise exclusive, not a jump.
10. **Macros and replay (design 4.6).** With a runtime, replay key tokens
    go through `routeKey` with `runtime` in replay mode (no timers);
    after the last token call `runtime.flush()`. Dot repeat is unchanged
    (parser-level `lastSequence`).
11. **Flushes.** Implement `flushPendingInput()` (resolve the mapper
    buffer, close hints). Add public `blur(): void` (same plus close
    hints) for VK-30. External surface updates (not `selfDispatch`) that
    change the selection or document schedule `host.timer(0, flush)`;
    never flush synchronously inside `onUpdate` (CodeSurface forbids
    re-entrant dispatch, and a flush may call `typeText`).
12. **Snapshot.** `pending` = parser pending + `runtime.pending()` (or the
    hop prompt while hopping); `hints` and `hop` fields set as above;
    `refresh()` after timer-driven changes.
13. **dispose.** Dispose the runtime (cancel timers) before the existing
    cleanup.

Pitfalls: the macro-stop `q` (recording active, parser idle, mapper
buffer empty) must not be pushed into the macro before routing, matching
today's order in `handleKey`; re-entrancy between sink callbacks and `routeKey`; timers
firing after `dispose` or `setKeymap` (cancel them); keeping K0 byte-for-
byte equivalent (all new code paths guarded by `runtime !== null`);
`allowsTextInput` during hop; not double-recording macro keys.

## Invariants

- K0: every existing test under `editor/test/code/vim/` and
  `editor/test/canvas/vim-*.test.ts` passes unchanged.
- `controller.ts` <= 850 lines; `keymap-runtime.ts` <= 300; purity passes.

## Test Cases

`editor/test/code/vim/keymap-controller.test.ts` (createTestHost, fake timers, `compileKeymap(KEYMAP_PRESETS[1].text)` unless stated):
- insert `abkj` (keys `i a b k j`) -> doc gains `ab`, mode normal, nothing extra typed
- insert `k` then advance timeoutlen -> `k` inserted via `typed`; `k` then `x` -> `kx`; dot repeat replays the insert including `k`
- `<Space>y` then `iw` -> `+` register clipboard write with the word
- `+` after two edits -> first press lands on the newest change, second press on the older change (design 357, VK-13 lines 90-95); `mm` on `(a)` -> jumps to the match
- `zc`/`zj`/`zk` -> `scrolls` records center delta, +1, -1
- `<Esc><Esc>` with an active search -> snapshot.search null; single `<Esc>` + timeout -> pending cleared
- `gn`/`<C-g>` with `diagnostics: [5, 20]` -> cursor 5 then 20; no diagnostics -> message `no diagnostics`
- hint: `<Space>` then advance 200 ms -> `snapshot.hints` with the leader entries; `y` -> hints null after completion; `<Esc>` closes; hints visible + 5000 ms -> buffer still pending (timeout suspended)
- hop: `gj` then `o` -> `snapshot.hop` labels; typing the first label moves the cursor; visual `v gj o <label>` extends the selection; `<Esc>` cancels and clears `hop`
- o-mode hop: custom keymap with o `gj -> hop.char1`: `d gj x <label>` deletes through the target inclusively; `.` does not repeat it
- hop disabled (preset with `hop.enabled:false`): `gj` moves down one line (extended `gj`), no hop session
- **DR-K1 regressions**: `gg` in a burst (no timer advance) -> cursor at line 1 with no pending timer; `zz` likewise scrolls; `mk` sets mark `k` with no cursor move (verify with `` `k ``); `mgj` sets mark `g` and moves down one line, no hop; `fk` and `tk` treat `k` literally; a test keymap with n `X -> fk` (`remap: true`) treats `k` literally
- `setKeymap(null)` after the preset -> `gj` invalid again (merged behavior), `kj` in insert types `kj`
- macro: `qa gj o <label> q` then `@a` replays the hop deterministically

`editor/test/code/vim/keymap-parity.test.ts`: a table of at least 30 representative rows copied from the existing controller/parser tests (input doc, keys, expected doc, cursor, mode; include `gj`, `gJ`, `zz`, `ys`, `Ctrl-h`, `Ctrl-e` rows whose merged expectation is "no change"). Each row runs on (A) a controller that never received a keymap and (B) a controller that received `setKeymap(<compiled mise-darwin>)` then `setKeymap(null)`; both must equal the row's expected values and each other, including snapshot `pending` and `message`. The existing unchanged test files are the primary K0 evidence; this table guards the `setKeymap(null)` return path.

`editor/test/code/vim/extended-controller.test.ts`: `gJ`, `3<C-e>` with the cursor leaving the viewport, `ysiw(`, `ds(`, `cs"'`, `.` repeating a surround, `u` undoing a surround in one step; the same keys with no keymap leave doc and mode unchanged.

## Verification (from editor/)

1. `npm run check` -> exit 0
2. `./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts` -> exit 0, failureCount 0, at least 45 new cases
3. `wc -l src/code/vim/controller.ts src/code/vim/keymap-runtime.ts` -> at most 850 and 300 (record the output)

## Completion Criteria

- [x] `setKeymap`, `blur`, `flushPendingInput`, `routeKey` behavior as specified
- [x] All listed cases pass, including every DR-K1 regression and the parity table
- [x] Line budgets met; purity passes
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-20 implementation attempt 1

**Tasks completed**: Controller/runtime integration, 33-row K0 parity table,
extended controller coverage, DR-K1 routing regressions, hints, hop, macros,
flush/blur lifecycle, and plan-owned progress record.

**Implementation**: Added `keymap-runtime.ts` (mapper sink, timeout/hint
lifecycle and hop session) and connected it to `VimController`. `routeKey`
now orders hop session, parser argument bypass, then mapper; leftover and remap
keys use the same route. Added live `setKeymap`, K0 reset, insert key delivery,
diagnostic/hop actions, extended join/scroll/surround execution, change-list
recording, replay, asynchronous external-update flush, and snapshot views.

**Verification**:
- `cd editor && npm run check`: exit 0; `tmp/vim-keymap/VK-20/attempt-1/npm-check-final-5.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts`: exit 1; 1,229 passed, 1 failed. The new `+` older-change regression expected offset 0 but `ChangeList.step(-1, 1)` returned the newest position at offset 3. Log: `tmp/vim-keymap/VK-20/attempt-1/focused-exact-blocked-final.log`.
- Earlier complete selected suite run: exit 0, 1,227/1,227 passed before adding the final `+` regression case; `tmp/vim-keymap/VK-20/attempt-1/focused-exact-final-2.log`.
- New controller/parity/extended suites passed 59/59 before the final cases; `tmp/vim-keymap/VK-20/attempt-1/vk20-positive-final-4.log`.
- `wc -l`: controller 836 (<=850), runtime 135 (<=300); `tmp/vim-keymap/VK-20/attempt-1/line-budget-final.log`.
- `git diff --check` on VK-20 paths: exit 0.

### Session: 2026-10-17 VK-20 review repair

**Tasks completed**: Corrected the `+` controller regression to assert the
accepted ChangeList behavior: after two edits, the first `+` (mise-darwin
`g;`) lands on the newest recorded change at offset 3, and the second reaches
the older change at offset 0. Renamed the case to describe walking newest to
older. The prior VK-13/`marks.ts` ownership blocker was a test expectation
mistake; production `marks.ts` remains unchanged (blob
`7df7653c4ffc294cfa79cd95f61bb2c90801c3d7`).

**Rationale**: `design-docs/specs/design-vim-keymap-hints-hop.md:357` specifies
that a new change resets the walk index to after newest, and
`impl-plans/active/vim-keymap-13-extended-builtins.md:90-95` specifies that
`g;` moves older from that position. The probe
`tmp/vim-keymap/reconcile/attempt-5/changelist-probe.log` observes first step
3, second step 0, then E662. The previous focused failure log remains preserved
at `tmp/vim-keymap/VK-20/attempt-1/focused-exact-blocked-final.log`.

**Verification**:
- `cd editor && npm run check`: exit 0; `tmp/vim-keymap/VK-20/attempt-2/npm-check.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts`: exit 0, 1,230/1,230 passed; `tmp/vim-keymap/VK-20/attempt-2/focused-vitest.log`.
- `cd editor && wc -l src/code/vim/controller.ts src/code/vim/keymap-runtime.ts`: exit 0, 836/135 lines; within 850/300 limits.
- `git hash-object editor/src/code/vim/marks.ts`: exit 0, `7df7653c4ffc294cfa79cd95f61bb2c90801c3d7` (unchanged accepted VK-13 source).
- `git diff --check -- <VK-20 paths>`: exit 0.

Independent test-integrity and adversarial reviews, then serial integration,
remain downstream; no approval is claimed here.

### Session: 2026-10-17 VK-20 insert-mismatch review repair

**Finding**: `VK20-insert-mismatch-key-dropped` (high, adversarial review
`comm-007201`). The runtime callback discarded the boolean returned by the
re-entrant `routeKey`; `handleKey` then consumed the original prefix-mismatch
event, so its key was neither inserted natively nor delivered by the controller.

**Correction**: The callback now checks the route result and, only when it is
false and the controller remains in insert or replace mode, delivers the key
through a shared printable `host.typeText` / non-printable `host.defaultKey`
helper. `deliverMappedInsertKey` shares that helper after its existing single
`handleSessionKey` call. The route order and top-level unmapped-key behavior
remain unchanged. The test input helper now translates `<BS>` to the actual
`Backspace` keyboard event.

**Regression coverage**: Added immediate, no-timer tests for `kx|`, `like|`,
`k\n|` after `<CR>`, and `a|` after `k<BS>`. Existing `kj` consumption,
timeout behavior, DR-K1 routing cases, `+` newest-then-older assertions, and
parity cases remain in the focused suite.

**Verification**:
- `cd editor && npm run check`: exitStatus 0; log `tmp/vim-keymap/VK-20/attempt-3/retry-1-final-npm-check.log`.
- `cd editor && ./node_modules/.bin/vitest run test/code/vim test/canvas/vim-input.test.ts test/canvas/vim-mount.test.ts test/canvas/input-vim-hook.test.ts`: exitStatus 0, 1,234/1,234 passed; log `tmp/vim-keymap/VK-20/attempt-3/retry-1-focused-vitest.log`.
- `wc -l editor/src/code/vim/controller.ts editor/src/code/vim/keymap-runtime.ts`: exitStatus 0, 843/135 lines; log `tmp/vim-keymap/VK-20/attempt-3/structural-checks.log`.
- Protected blobs: `marks.ts` remains `7df7653c4ffc294cfa79cd95f61bb2c90801c3d7`; `mapper.ts` remains `b6bda6bd39ced1d0840bbc7e49f9a6aac7d11986` (VK-10 accepted blob); same structural-check log.
- `git diff --check` on VK-20 paths: exitStatus 0; same structural-check log.

The first attempt-3 Vitest run exposed that the test helper was passing the literal
string `<BS>` instead of a browser Backspace key; that run failed 1/1,234 and is
preserved at `tmp/vim-keymap/VK-20/attempt-3/focused-vitest.log`. The helper was
corrected, and the subsequent full focused suite passed. An initial npm check
wrapper also used zsh's readonly `status` variable; the successful final-source
rerun is recorded above. Independent test-integrity and adversarial review are
still pending; this is not a review acceptance claim.
